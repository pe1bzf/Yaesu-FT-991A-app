//! Radio-service: de "baas" van de radio.
//!
//! - Houdt een [`RadioState`] bij (frequenties, mode, filter).
//! - Voert [`Action`]s uit in de juiste volgorde en leest daarna terug wat de
//!   radio werkelijk heeft ingesteld.
//! - Verwerkt meldingen die de radio zelf stuurt (`AI1;`, bijv. bij draaien aan
//!   de knop) en meldt elke wijziging aan een luisteraar.
//!
//! Een UI praat alleen in [`Action`] (erin) en [`RadioState`] (eruit), beide als
//! JSON. Zo kan dezelfde UI lokaal via Tauri of later via de Raspberry Pi over
//! een WebSocket werken.

use std::sync::{Arc, Mutex};

use ft991a_cat::{
    bandwidth_hz, check_frequency, width_options, Command, Mode, Response, Vfo, WidthOption,
    IF_SHIFT_MAX_HZ, IF_SHIFT_STEP_HZ, RADIO_ID,
};
use ft991a_transport::{Reader, Transport, TransportError, TransportEvent, Writer};
use serde::{Deserialize, Serialize};

/// Alles wat de UI over de radio moet weten.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RadioState {
    pub connected: bool,
    pub freq_a: u32,
    pub freq_b: u32,
    pub mode: Option<Mode>,
    /// `None` in FM, AM en C4FM: daar zijn geen filters in te stellen.
    pub filter: Option<FilterState>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FilterState {
    /// `SH`-index 00–21.
    pub width_index: u8,
    /// Wat die index in deze mode betekent, als de tabel hem kent.
    pub width_hz: Option<u32>,
    pub narrow: bool,
    pub if_shift_hz: i16,
    /// Alle keuzes voor de bandbreedte in deze mode/narrow-stand.
    pub width_options: Vec<WidthOption>,
}

impl RadioState {
    fn new() -> RadioState {
        RadioState {
            connected: true,
            freq_a: 0,
            freq_b: 0,
            mode: None,
            filter: None,
        }
    }

    /// Verwerkt een antwoord of melding van de radio.
    fn apply(&mut self, response: &Response) {
        match *response {
            Response::Frequency(Vfo::A, hz) => self.freq_a = hz,
            Response::Frequency(Vfo::B, hz) => self.freq_b = hz,
            Response::Info(vfo, line) => {
                match vfo {
                    Vfo::A => self.freq_a = line.frequency_hz,
                    Vfo::B => self.freq_b = line.frequency_hz,
                }
                if vfo == Vfo::A {
                    self.set_mode(line.mode);
                }
            }
            Response::Mode(mode) => self.set_mode(mode),
            Response::Width(index) => {
                if let Some(f) = &mut self.filter {
                    f.width_index = index;
                }
            }
            Response::Narrow(on) => {
                if let Some(f) = &mut self.filter {
                    f.narrow = on;
                }
            }
            Response::IfShift(hz) => {
                if let Some(f) = &mut self.filter {
                    f.if_shift_hz = hz;
                }
            }
            _ => {}
        }
        self.derive_filter_fields();
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = Some(mode);
        match (mode.filter_group(), &self.filter) {
            (None, _) => self.filter = None,
            (Some(_), Some(_)) => {}
            (Some(_), None) => {
                self.filter = Some(FilterState {
                    width_index: 0,
                    width_hz: None,
                    narrow: false,
                    if_shift_hz: 0,
                    width_options: Vec::new(),
                })
            }
        }
    }

    fn derive_filter_fields(&mut self) {
        let Some(mode) = self.mode else { return };
        let Some(group) = mode.filter_group() else {
            return;
        };
        if let Some(f) = &mut self.filter {
            f.width_options = width_options(mode, f.narrow);
            f.width_hz = bandwidth_hz(group, f.narrow, f.width_index);
        }
    }
}

/// Wat de UI kan vragen. In JSON: `{"type": "set_mode", "mode": "USB"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    SetFrequency {
        vfo: Vfo,
        hz: u32,
    },
    /// Frequentie omhoog of omlaag, bijv. `delta_hz: -1000` voor 1 kHz omlaag.
    StepFrequency {
        vfo: Vfo,
        delta_hz: i64,
    },
    SetMode {
        mode: Mode,
    },
    SetWidth {
        index: u8,
    },
    SetNarrow {
        on: bool,
    },
    /// Wordt afgerond op 20 Hz en begrensd op ±1200 Hz.
    SetIfShift {
        hz: i32,
    },
    /// Alles opnieuw uitlezen.
    Refresh,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RadioError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Cat(#[from] ft991a_cat::CatError),
    #[error("dit is geen FT-991A (ID {0}, verwacht {RADIO_ID})")]
    WrongRadio(String),
    #[error("in {} zijn geen filters in te stellen", .0.label())]
    NoFilter(Mode),
    #[error("bandbreedte-index {0} bestaat niet in deze mode")]
    UnknownWidth(u8),
    #[error("onverwacht antwoord van de radio: {0:?}")]
    Unexpected(Response),
}

pub type Listener = Box<dyn Fn(&RadioState) + Send + Sync>;

struct Shared {
    state: Mutex<RadioState>,
    listener: Listener,
}

impl Shared {
    fn update(&self, response: &Response) -> RadioState {
        let mut state = self.state.lock().unwrap();
        state.apply(response);
        state.clone()
    }

    fn notify(&self) {
        let snapshot = self.state.lock().unwrap().clone();
        (self.listener)(&snapshot);
    }

    fn on_event(&self, event: TransportEvent) {
        match event {
            TransportEvent::Frame(response) => {
                self.update(&response);
            }
            TransportEvent::Closed(_) => self.state.lock().unwrap().connected = false,
        }
        self.notify();
    }
}

pub struct Radio {
    transport: Transport,
    shared: Arc<Shared>,
}

impl Radio {
    /// Verbindt via een open poort (echt of simulator): controleert dat het een
    /// FT-991A is, zet Auto Information aan en leest alles één keer uit.
    /// `listener` hoort elke wijziging, ook die van draaien aan de knop.
    pub fn connect(
        reader: Reader,
        writer: Writer,
        listener: Listener,
    ) -> Result<Radio, RadioError> {
        let shared = Arc::new(Shared {
            state: Mutex::new(RadioState::new()),
            listener,
        });
        let events = shared.clone();
        let transport = Transport::new(reader, writer, Box::new(move |e| events.on_event(e)));
        let radio = Radio { transport, shared };

        match radio.transport.query(Command::ReadId)? {
            Response::Id(id) if id == RADIO_ID => {}
            Response::Id(id) => return Err(RadioError::WrongRadio(id)),
            other => return Err(RadioError::Unexpected(other)),
        }
        radio.transport.send(Command::SetAutoInfo(true))?;
        radio.refresh()?;
        radio.shared.notify();
        Ok(radio)
    }

    pub fn state(&self) -> RadioState {
        self.shared.state.lock().unwrap().clone()
    }

    /// Voert een actie uit en geeft de nieuwe toestand terug.
    pub fn apply(&self, action: Action) -> Result<RadioState, RadioError> {
        let result = self.run(action);
        // Ook bij een fout: de UI moet weten wat de radio nu werkelijk doet.
        self.shared.notify();
        result.map(|_| self.state())
    }

    fn run(&self, action: Action) -> Result<(), RadioError> {
        match action {
            Action::SetFrequency { vfo, hz } => self.set_frequency(vfo, hz),
            Action::StepFrequency { vfo, delta_hz } => {
                let current = match vfo {
                    Vfo::A => self.state().freq_a,
                    Vfo::B => self.state().freq_b,
                } as i64;
                let target = check_frequency((current + delta_hz).max(0) as u64)?;
                self.set_frequency(vfo, target)
            }
            Action::SetMode { mode } => {
                self.transport.send(Command::SetMode(mode))?;
                self.read(Command::ReadMode)?;
                // De betekenis van de bandbreedte hangt van de mode af.
                self.refresh_filter()
            }
            Action::SetWidth { index } => {
                let (mode, narrow) = self.filter_context()?;
                let group = mode.filter_group().expect("filter_context checkt dit");
                if bandwidth_hz(group, narrow, index).is_none() {
                    return Err(RadioError::UnknownWidth(index));
                }
                self.transport.send(Command::SetWidth(index))?;
                self.read(Command::ReadWidth)
            }
            Action::SetNarrow { on } => {
                self.filter_context()?;
                self.transport.send(Command::SetNarrow(on))?;
                self.read(Command::ReadNarrow)?;
                // Andere tabel, dus de index kan door de radio aangepast zijn.
                self.read(Command::ReadWidth)
            }
            Action::SetIfShift { hz } => {
                self.filter_context()?;
                self.transport
                    .send(Command::SetIfShift(round_if_shift(hz)))?;
                self.read(Command::ReadIfShift)
            }
            Action::Refresh => self.refresh(),
        }
    }

    fn set_frequency(&self, vfo: Vfo, hz: u32) -> Result<(), RadioError> {
        self.transport.send(Command::SetFrequency(vfo, hz))?;
        self.read(Command::ReadFrequency(vfo))
    }

    fn refresh(&self) -> Result<(), RadioError> {
        self.read(Command::ReadFrequency(Vfo::A))?;
        self.read(Command::ReadFrequency(Vfo::B))?;
        self.read(Command::ReadMode)?;
        self.refresh_filter()
    }

    fn refresh_filter(&self) -> Result<(), RadioError> {
        if self.state().filter.is_some() {
            self.read(Command::ReadNarrow)?;
            self.read(Command::ReadWidth)?;
            self.read(Command::ReadIfShift)?;
        }
        Ok(())
    }

    /// Mode en narrow-stand, of een nette fout als de mode geen filters heeft.
    fn filter_context(&self) -> Result<(Mode, bool), RadioError> {
        let state = self.state();
        match (state.mode, state.filter) {
            (Some(mode), Some(f)) => Ok((mode, f.narrow)),
            (Some(mode), None) => Err(RadioError::NoFilter(mode)),
            (None, _) => Err(RadioError::Unexpected(Response::Other(
                "mode onbekend".into(),
            ))),
        }
    }

    fn read(&self, cmd: Command) -> Result<(), RadioError> {
        let response = self.transport.query(cmd)?;
        self.shared.update(&response);
        Ok(())
    }
}

/// Rondt af op 20 Hz en begrenst op ±1200 Hz.
fn round_if_shift(hz: i32) -> i16 {
    let step = IF_SHIFT_STEP_HZ as i32;
    let max = IF_SHIFT_MAX_HZ as i32;
    let rounded = ((hz as f64 / step as f64).round() as i32) * step;
    rounded.clamp(-max, max) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use ft991a_sim::SimRadio;
    use std::sync::mpsc;
    use std::time::Duration;

    fn connect(sim: &SimRadio) -> (Radio, mpsc::Receiver<RadioState>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let (r, w) = sim.connect();
        let radio = Radio::connect(
            r,
            w,
            Box::new(move |s| {
                let _ = tx.lock().unwrap().send(s.clone());
            }),
        )
        .unwrap();
        (radio, rx)
    }

    #[test]
    fn connect_reads_everything() {
        let sim = SimRadio::new();
        let (radio, _) = connect(&sim);
        let s = radio.state();
        assert!(s.connected);
        assert_eq!(s.freq_a, 14_250_000);
        assert_eq!(s.freq_b, 7_074_000);
        assert_eq!(s.mode, Some(Mode::Usb));
        let f = s.filter.unwrap();
        assert_eq!(f.width_hz, Some(2400));
        assert_eq!(f.width_options.len(), 14);
        assert!(sim.state().auto_info);
    }

    #[test]
    fn set_and_step_frequency() {
        let sim = SimRadio::new();
        let (radio, _) = connect(&sim);
        radio
            .apply(Action::SetFrequency {
                vfo: Vfo::B,
                hz: 145_500_000,
            })
            .unwrap();
        let s = radio
            .apply(Action::StepFrequency {
                vfo: Vfo::A,
                delta_hz: -1_000,
            })
            .unwrap();
        assert_eq!(s.freq_a, 14_249_000);
        assert_eq!(s.freq_b, 145_500_000);
        assert_eq!(sim.state().freq_b, 145_500_000);
        assert!(radio
            .apply(Action::SetFrequency {
                vfo: Vfo::A,
                hz: 500_000_000
            })
            .is_err());
    }

    #[test]
    fn mode_change_updates_filter_table() {
        let sim = SimRadio::new();
        let (radio, _) = connect(&sim);
        let s = radio.apply(Action::SetMode { mode: Mode::Cw }).unwrap();
        assert_eq!(s.filter.unwrap().width_hz, Some(2400));
        let s = radio.apply(Action::SetNarrow { on: true }).unwrap();
        let f = s.filter.unwrap();
        assert!(f.narrow);
        assert_eq!(f.width_hz, Some(500));
        let s = radio.apply(Action::SetWidth { index: 1 }).unwrap();
        assert_eq!(s.filter.unwrap().width_hz, Some(50));

        let s = radio.apply(Action::SetMode { mode: Mode::Fm }).unwrap();
        assert!(s.filter.is_none());
        assert_eq!(
            radio.apply(Action::SetWidth { index: 1 }),
            Err(RadioError::NoFilter(Mode::Fm))
        );
    }

    #[test]
    fn if_shift_is_rounded_and_clamped() {
        let sim = SimRadio::new();
        let (radio, _) = connect(&sim);
        let s = radio.apply(Action::SetIfShift { hz: 109 }).unwrap();
        assert_eq!(s.filter.unwrap().if_shift_hz, 100);
        let s = radio.apply(Action::SetIfShift { hz: -5000 }).unwrap();
        assert_eq!(s.filter.unwrap().if_shift_hz, -1200);
        assert_eq!(sim.state().if_shift, -1200);
    }

    #[test]
    fn invalid_width_is_refused_before_sending() {
        let sim = SimRadio::new();
        let (radio, _) = connect(&sim);
        assert_eq!(
            radio.apply(Action::SetWidth { index: 3 }),
            Err(RadioError::UnknownWidth(3))
        );
    }

    #[test]
    fn turning_the_dial_reaches_the_listener() {
        let sim = SimRadio::new();
        let (radio, rx) = connect(&sim);
        while rx.try_recv().is_ok() {}
        sim.turn_dial(500);
        let s = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(s.freq_a, 14_250_500);
        assert_eq!(radio.state().freq_a, 14_250_500);
    }

    #[test]
    fn actions_are_plain_json() {
        let a: Action = serde_json::from_str(r#"{"type":"set_mode","mode":"DATA-USB"}"#).unwrap();
        assert_eq!(
            a,
            Action::SetMode {
                mode: Mode::DataUsb
            }
        );
        let a: Action =
            serde_json::from_str(r#"{"type":"step_frequency","vfo":"A","delta_hz":-100}"#).unwrap();
        assert_eq!(
            a,
            Action::StepFrequency {
                vfo: Vfo::A,
                delta_hz: -100
            }
        );
    }
}
