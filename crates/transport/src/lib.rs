//! Transport: praat met de radio over een byte-stroom.
//!
//! - Een eigen leesthread knipt binnenkomende bytes op `;` tot losse berichten.
//! - Er is altijd hooguit één vraag "in de lucht"; het antwoord wordt herkend aan
//!   de eerste twee letters, zodat antwoorden nooit verwisseld raken.
//! - Berichten waar niemand op wacht (Auto Information na `AI1;`) gaan als
//!   [`TransportEvent::Frame`] naar boven.
//!
//! De byte-stroom kan een echte seriële poort zijn ([`serial`]) of de simulator.

use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ft991a_cat::{split_frames, CatError, Command, Response};

#[cfg(feature = "serial")]
pub mod serial;

pub type Reader = Box<dyn Read + Send>;
pub type Writer = Box<dyn Write + Send>;

/// Hoe lang we op een antwoord wachten.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("de radio antwoordde niet op {0}")]
    Timeout(String),
    #[error("de radio weigerde {0} (niet toegestaan in deze mode of toestand)")]
    Rejected(String),
    #[error("de verbinding met de radio is verbroken")]
    Closed,
    #[error("fout op de poort: {0}")]
    Io(String),
    #[error(transparent)]
    Cat(#[from] CatError),
}

/// Wat de leesthread naar boven meldt.
#[derive(Debug, Clone, PartialEq)]
pub enum TransportEvent {
    /// Een bericht waar geen vraag bij hoort, al gelezen.
    Frame(Response),
    /// De poort is dicht of weggevallen (bijv. USB-kabel eruit).
    Closed(String),
}

pub type EventHandler = Box<dyn Fn(TransportEvent) + Send + Sync>;

struct Pending {
    prefix: &'static str,
    reply: mpsc::Sender<String>,
}

pub struct Transport {
    /// De schrijfkant. Wie hem vasthoudt mag als enige een vraag stellen.
    writer: Mutex<Writer>,
    pending: Arc<Mutex<Option<Pending>>>,
    alive: Arc<AtomicBool>,
    timeout: Duration,
}

impl Transport {
    /// Start de leesthread. `on_event` wordt vanuit die thread aangeroepen.
    pub fn new(reader: Reader, writer: Writer, on_event: EventHandler) -> Transport {
        let pending: Arc<Mutex<Option<Pending>>> = Arc::new(Mutex::new(None));
        let alive = Arc::new(AtomicBool::new(true));
        {
            let pending = pending.clone();
            let alive = alive.clone();
            thread::Builder::new()
                .name("ft991a-reader".into())
                .spawn(move || read_loop(reader, pending, alive, on_event))
                .expect("leesthread starten");
        }
        Transport {
            writer: Mutex::new(writer),
            pending,
            alive,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn is_open(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Stuurt een commando zonder op antwoord te wachten (voor Set-commando's).
    pub fn send(&self, cmd: Command) -> Result<(), TransportError> {
        let text = cmd.encode()?;
        let mut writer = self.writer.lock().unwrap();
        self.write(&mut writer, &text)
    }

    /// Stuurt een vraag en wacht op het bijbehorende antwoord.
    pub fn query(&self, cmd: Command) -> Result<Response, TransportError> {
        let text = cmd.encode()?;
        let prefix = cmd
            .answer_prefix()
            .expect("query alleen voor commando's met een antwoord");

        let mut writer = self.writer.lock().unwrap();
        let (tx, rx) = mpsc::channel();
        *self.pending.lock().unwrap() = Some(Pending { prefix, reply: tx });

        let result =
            self.write(&mut writer, &text)
                .and_then(|_| match rx.recv_timeout(self.timeout) {
                    Ok(frame) => match Response::parse(&frame)? {
                        Response::Rejected => Err(TransportError::Rejected(text.clone())),
                        r => Ok(r),
                    },
                    Err(RecvTimeoutError::Timeout) => Err(TransportError::Timeout(text.clone())),
                    Err(RecvTimeoutError::Disconnected) => Err(TransportError::Closed),
                });
        self.pending.lock().unwrap().take();
        result
    }

    fn write(&self, writer: &mut Writer, text: &str) -> Result<(), TransportError> {
        if !self.is_open() {
            return Err(TransportError::Closed);
        }
        writer
            .write_all(text.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|e| TransportError::Io(e.to_string()))
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        // De leesthread ziet dit bij zijn volgende time-out en stopt.
        self.alive.store(false, Ordering::SeqCst);
    }
}

fn read_loop(
    mut reader: Reader,
    pending: Arc<Mutex<Option<Pending>>>,
    alive: Arc<AtomicBool>,
    on_event: EventHandler,
) {
    let mut buf = [0u8; 256];
    let mut text = String::new();
    let reason = loop {
        if !alive.load(Ordering::SeqCst) {
            return; // netjes gesloten door de app zelf
        }
        match reader.read(&mut buf) {
            Ok(0) => break "poort gesloten".to_string(),
            Ok(n) => {
                text.push_str(&String::from_utf8_lossy(&buf[..n]));
                for frame in split_frames(&mut text) {
                    dispatch(frame, &pending, &on_event);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
                ) => {}
            Err(e) => break e.to_string(),
        }
    };
    alive.store(false, Ordering::SeqCst);
    // Wie nog op een antwoord wacht krijgt meteen "verbroken".
    pending.lock().unwrap().take();
    on_event(TransportEvent::Closed(reason));
}

fn dispatch(frame: String, pending: &Mutex<Option<Pending>>, on_event: &EventHandler) {
    {
        let mut slot = pending.lock().unwrap();
        let matches = slot
            .as_ref()
            .is_some_and(|p| frame == "?" || frame.starts_with(p.prefix));
        if matches {
            let p = slot.take().unwrap();
            let _ = p.reply.send(frame);
            return;
        }
    }
    // Onleesbare berichten slaan we over in plaats van de verbinding te breken.
    if let Ok(response) = Response::parse(&frame) {
        on_event(TransportEvent::Frame(response));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ft991a_cat::{Mode, Vfo};
    use std::sync::mpsc::Receiver;

    /// Een minimale nep-poort: wat de test in `to_app` stopt komt bij de
    /// transportlaag binnen; wat de transportlaag schrijft komt in `from_app`.
    struct ChannelReader(Receiver<Vec<u8>>);
    impl Read for ChannelReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.recv_timeout(Duration::from_millis(20)) {
                Ok(bytes) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                Err(RecvTimeoutError::Timeout) => Err(ErrorKind::TimedOut.into()),
                Err(RecvTimeoutError::Disconnected) => Ok(0),
            }
        }
    }
    struct ChannelWriter(mpsc::Sender<String>);
    impl Write for ChannelWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _ = self.0.send(String::from_utf8_lossy(buf).into_owned());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct Rig {
        transport: Arc<Transport>,
        to_app: mpsc::Sender<Vec<u8>>,
        from_app: Receiver<String>,
        events: Receiver<TransportEvent>,
    }

    fn rig() -> Rig {
        let (to_app, app_in) = mpsc::channel();
        let (app_out, from_app) = mpsc::channel();
        let (ev_tx, events) = mpsc::channel();
        let ev_tx = Mutex::new(ev_tx);
        let transport = Transport::new(
            Box::new(ChannelReader(app_in)),
            Box::new(ChannelWriter(app_out)),
            Box::new(move |e| {
                let _ = ev_tx.lock().unwrap().send(e);
            }),
        )
        .with_timeout(Duration::from_millis(200));
        Rig {
            transport: Arc::new(transport),
            to_app,
            from_app,
            events,
        }
    }

    #[test]
    fn query_gets_its_own_answer_and_passes_others_on() {
        let rig = rig();
        let t = rig.transport.clone();
        let handle = thread::spawn(move || t.query(Command::ReadMode));
        assert_eq!(rig.from_app.recv().unwrap(), "MD0;");
        // Eerst komt er een AI-melding binnen, in twee stukken, dan het antwoord.
        rig.to_app.send(b"FA0142".to_vec()).unwrap();
        rig.to_app.send(b"50000;MD02;".to_vec()).unwrap();
        assert_eq!(handle.join().unwrap(), Ok(Response::Mode(Mode::Usb)));
        assert_eq!(
            rig.events.recv_timeout(Duration::from_secs(1)).unwrap(),
            TransportEvent::Frame(Response::Frequency(Vfo::A, 14_250_000))
        );
    }

    #[test]
    fn question_mark_is_a_rejection() {
        let rig = rig();
        let t = rig.transport.clone();
        let handle = thread::spawn(move || t.query(Command::ReadWidth));
        rig.from_app.recv().unwrap();
        rig.to_app.send(b"?;".to_vec()).unwrap();
        assert!(matches!(
            handle.join().unwrap(),
            Err(TransportError::Rejected(_))
        ));
    }

    #[test]
    fn silence_is_a_timeout() {
        let rig = rig();
        assert!(matches!(
            rig.transport.query(Command::ReadId),
            Err(TransportError::Timeout(_))
        ));
    }

    #[test]
    fn closed_port_is_reported() {
        let rig = rig();
        drop(rig.to_app);
        assert!(matches!(
            rig.events.recv_timeout(Duration::from_secs(1)).unwrap(),
            TransportEvent::Closed(_)
        ));
        assert_eq!(
            rig.transport.send(Command::ReadId),
            Err(TransportError::Closed)
        );
    }
}
