//! Een gesimuleerde FT-991A.
//!
//! Begrijpt dezelfde CAT-commando's als de echte radio, houdt een toestand bij
//! en antwoordt via een byte-stroom die er voor de app net zo uitziet als een
//! seriële poort. Zo werken de UI, de radio-service en de tests zonder radio.
//!
//! Gedrag dat niet in de Yaesu-manual staat (welke commando's in FM met `?;`
//! geweigerd worden, wat er met de bandbreedte gebeurt bij een modewissel) is
//! een redelijke aanname en moet met de echte radio vergeleken worden.

use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ft991a_cat::{
    bandwidth_hz, check_frequency, split_frames, Command, InfoLine, MemoryMode, Mode,
    RepeaterShift, Response, ToneMode, Vfo, RADIO_ID,
};

/// De toestand van de nep-radio.
#[derive(Debug, Clone, PartialEq)]
pub struct SimState {
    pub freq_a: u32,
    pub freq_b: u32,
    pub mode: Mode,
    pub width: u8,
    pub narrow: bool,
    pub if_shift: i16,
    pub auto_info: bool,
}

impl Default for SimState {
    fn default() -> Self {
        SimState {
            freq_a: 14_250_000,
            freq_b: 7_074_000,
            mode: Mode::Usb,
            width: 0,
            narrow: false,
            if_shift: 0,
            auto_info: false,
        }
    }
}

struct Inner {
    state: SimState,
    /// Uitgang van elke open verbinding, voor AI-meldingen.
    outputs: Vec<Sender<Vec<u8>>>,
}

/// Handvat naar de nep-radio. Goedkoop te klonen; alle klonen delen één radio.
#[derive(Clone)]
pub struct SimRadio {
    inner: Arc<Mutex<Inner>>,
}

impl Default for SimRadio {
    fn default() -> Self {
        Self::new()
    }
}

impl SimRadio {
    pub fn new() -> SimRadio {
        SimRadio {
            inner: Arc::new(Mutex::new(Inner {
                state: SimState::default(),
                outputs: Vec::new(),
            })),
        }
    }

    pub fn state(&self) -> SimState {
        self.inner.lock().unwrap().state.clone()
    }

    /// Opent een "seriële poort" naar de nep-radio: (lezen, schrijven).
    pub fn connect(&self) -> (Box<dyn Read + Send>, Box<dyn Write + Send>) {
        let (tx, rx) = mpsc::channel();
        self.inner.lock().unwrap().outputs.push(tx.clone());
        (
            Box::new(SimReader {
                rx,
                leftover: VecDeque::new(),
            }),
            Box::new(SimWriter {
                radio: self.clone(),
                out: tx,
                buffer: String::new(),
            }),
        )
    }

    /// Doet alsof iemand aan de VFO-A-knop draait. Met `AI1;` meldt de radio
    /// de nieuwe frequentie zelf, net als de echte.
    pub fn turn_dial(&self, delta_hz: i64) {
        let mut inner = self.inner.lock().unwrap();
        let wanted = inner.state.freq_a as i64 + delta_hz;
        if let Ok(hz) = check_frequency(wanted.max(0) as u64) {
            inner.state.freq_a = hz;
            if inner.state.auto_info {
                let msg = Response::Frequency(Vfo::A, hz).encode().into_bytes();
                inner.outputs.retain(|out| out.send(msg.clone()).is_ok());
            }
        }
    }

    /// Verwerkt één commando (zonder `;`) en geeft het antwoord, of `None` als
    /// een Set-commando gelukt is (de echte radio zegt dan ook niets).
    pub fn handle(&self, frame: &str) -> Option<Response> {
        let mut inner = self.inner.lock().unwrap();
        let s = &mut inner.state;
        let Ok(cmd) = Command::parse(frame) else {
            return Some(Response::Rejected);
        };
        let has_filter = s.mode.filter_group().is_some();
        match cmd {
            Command::ReadId => Some(Response::Id(RADIO_ID.into())),
            Command::ReadAutoInfo => Some(Response::AutoInfo(s.auto_info)),
            Command::SetAutoInfo(on) => {
                s.auto_info = on;
                None
            }
            Command::ReadFrequency(Vfo::A) => Some(Response::Frequency(Vfo::A, s.freq_a)),
            Command::ReadFrequency(Vfo::B) => Some(Response::Frequency(Vfo::B, s.freq_b)),
            Command::SetFrequency(Vfo::A, hz) => {
                s.freq_a = hz;
                None
            }
            Command::SetFrequency(Vfo::B, hz) => {
                s.freq_b = hz;
                None
            }
            Command::ReadMode => Some(Response::Mode(s.mode)),
            Command::SetMode(mode) => {
                s.mode = mode;
                s.fix_width();
                None
            }
            Command::ReadWidth => Some(Response::Width(s.width)),
            Command::ReadNarrow => Some(Response::Narrow(s.narrow)),
            Command::ReadIfShift => Some(Response::IfShift(s.if_shift)),
            // Filters bestaan niet in FM/AM/C4FM: de radio weigert ze.
            Command::SetWidth(_) | Command::SetNarrow(_) | Command::SetIfShift(_)
                if !has_filter =>
            {
                Some(Response::Rejected)
            }
            Command::SetWidth(index) => {
                let group = s.mode.filter_group().unwrap();
                if bandwidth_hz(group, s.narrow, index).is_none() {
                    return Some(Response::Rejected);
                }
                s.width = index;
                None
            }
            Command::SetNarrow(on) => {
                s.narrow = on;
                s.fix_width();
                None
            }
            Command::SetIfShift(hz) => {
                s.if_shift = hz;
                None
            }
            Command::ReadInfo(vfo) => {
                let freq = if vfo == Vfo::A { s.freq_a } else { s.freq_b };
                Some(Response::Info(
                    vfo,
                    InfoLine {
                        memory_channel: 1,
                        frequency_hz: freq,
                        clarifier_hz: 0,
                        rx_clarifier: false,
                        tx_clarifier: false,
                        mode: s.mode,
                        memory_mode: MemoryMode::Vfo,
                        tone_mode: ToneMode::Off,
                        shift: RepeaterShift::Simplex,
                    },
                ))
            }
        }
    }
}

impl SimState {
    /// Na een wissel van mode of narrow: een index die in de nieuwe tabel niet
    /// bestaat valt terug op 00 (standaardbreedte).
    fn fix_width(&mut self) {
        if let Some(group) = self.mode.filter_group() {
            if bandwidth_hz(group, self.narrow, self.width).is_none() {
                self.width = 0;
            }
        }
    }
}

struct SimWriter {
    radio: SimRadio,
    out: Sender<Vec<u8>>,
    buffer: String,
}

impl Write for SimWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.push_str(&String::from_utf8_lossy(buf));
        for frame in split_frames(&mut self.buffer) {
            if let Some(answer) = self.radio.handle(&frame) {
                let _ = self.out.send(answer.encode().into_bytes());
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct SimReader {
    rx: Receiver<Vec<u8>>,
    leftover: VecDeque<u8>,
}

impl Read for SimReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.leftover.is_empty() {
            // Korte time-out, net als een seriële poort, zodat de leesthread
            // kan stoppen als de app de verbinding sluit.
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(bytes) => self.leftover.extend(bytes),
                Err(RecvTimeoutError::Timeout) => return Err(ErrorKind::TimedOut.into()),
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
        let n = buf.len().min(self.leftover.len());
        for (dst, src) in buf.iter_mut().zip(self.leftover.drain(..n)) {
            *dst = src;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(sim: &SimRadio, cmd: &str) -> String {
        sim.handle(cmd.trim_end_matches(';'))
            .map(|r| r.encode())
            .unwrap_or_default()
    }

    #[test]
    fn answers_like_the_radio() {
        let sim = SimRadio::new();
        assert_eq!(ask(&sim, "ID;"), "ID0670;");
        assert_eq!(ask(&sim, "FA145500000;"), "");
        assert_eq!(ask(&sim, "FA;"), "FA145500000;");
        assert_eq!(ask(&sim, "MD04;"), "");
        assert_eq!(ask(&sim, "IF;"), "IF001145500000+000000400000;");
    }

    #[test]
    fn rejects_filters_in_fm_and_bad_input() {
        let sim = SimRadio::new();
        ask(&sim, "MD04;");
        assert_eq!(ask(&sim, "SH005;"), "?;");
        assert_eq!(ask(&sim, "IS0+0100;"), "?;");
        assert_eq!(ask(&sim, "FA1425;"), "?;");
        assert_eq!(ask(&sim, "ZZ;"), "?;");
    }

    #[test]
    fn width_follows_the_table_for_the_mode() {
        let sim = SimRadio::new();
        assert_eq!(ask(&sim, "SH021;"), "");
        // 21 (3200 Hz) bestaat niet bij narrow: terug naar 00.
        ask(&sim, "NA01;");
        assert_eq!(ask(&sim, "SH0;"), "SH000;");
        // Bij narrow bestaat index 03 wel (600 Hz).
        assert_eq!(ask(&sim, "SH003;"), "");
    }

    #[test]
    fn stream_round_trip_and_auto_info() {
        let sim = SimRadio::new();
        let (mut r, mut w) = sim.connect();
        w.write_all(b"AI1;FA;").unwrap();
        let mut buf = [0u8; 64];
        let n = r.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"FA014250000;");
        sim.turn_dial(1_000);
        let n = r.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"FA014251000;");
    }
}
