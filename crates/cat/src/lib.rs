//! CAT-laag voor de Yaesu FT-991A.
//!
//! Deze crate weet alleen hoe commando's eruitzien: hij bouwt de tekst die naar
//! de radio gaat (`FA014250000;`) en leest de tekst die terugkomt. Er zit geen
//! seriële poort of toestand in; dat doen `ft991a-transport` en `ft991a-radio`.
//!
//! Bron: Yaesu *FT-991A CAT Operation Reference Manual*, samengevat in
//! `cat/ft991a-cat-overzicht.md` in de projectbestanden.

mod command;
mod filter;
mod info;
mod mode;
mod response;

pub use command::Command;
pub use filter::{bandwidth_hz, width_options, FilterGroup, WidthOption};
pub use info::{InfoLine, MemoryMode, RepeaterShift, ToneMode};
pub use mode::Mode;
pub use response::Response;

use serde::{Deserialize, Serialize};

/// Laagste en hoogste frequentie die `FA`/`FB` accepteren (Hz).
pub const FREQ_MIN_HZ: u32 = 30_000;
pub const FREQ_MAX_HZ: u32 = 470_000_000;

/// IF-shift: −1200 … +1200 Hz in stappen van 20 Hz.
pub const IF_SHIFT_MAX_HZ: i16 = 1200;
pub const IF_SHIFT_STEP_HZ: i16 = 20;

/// Antwoord op `ID;` voor een FT-991A.
pub const RADIO_ID: &str = "0670";

/// Elk CAT-bericht eindigt met dit teken.
pub const TERMINATOR: char = ';';

/// Welke VFO (A = hoofd, B = sub).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Vfo {
    A,
    B,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatError {
    #[error("frequentie {0} Hz ligt buiten 30 kHz – 470 MHz")]
    FrequencyOutOfRange(u64),
    #[error("IF-shift {0} Hz ligt buiten −1200 … +1200 Hz")]
    IfShiftOutOfRange(i32),
    #[error("bandbreedte-index {0} ligt buiten 00–21")]
    WidthOutOfRange(u8),
    #[error("onbekend of onleesbaar CAT-bericht: {0:?}")]
    Unparsable(String),
}

/// Controleert een frequentie en geeft hem terug als hij binnen bereik ligt.
pub fn check_frequency(hz: u64) -> Result<u32, CatError> {
    if (FREQ_MIN_HZ as u64..=FREQ_MAX_HZ as u64).contains(&hz) {
        Ok(hz as u32)
    } else {
        Err(CatError::FrequencyOutOfRange(hz))
    }
}

/// Knipt een stroom tekens op `;` in losse berichten.
///
/// Geeft de complete berichten terug (zonder `;`, in hoofdletters) en laat een
/// onvolledig stuk in `buffer` staan tot de rest binnen is.
pub fn split_frames(buffer: &mut String) -> Vec<String> {
    let mut frames = Vec::new();
    while let Some(pos) = buffer.find(TERMINATOR) {
        let frame: String = buffer[..pos].trim().to_ascii_uppercase();
        buffer.drain(..=pos);
        if !frame.is_empty() {
            frames.push(frame);
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_stream_on_semicolon() {
        let mut buf = String::from("FA014250000;md02;\r\nIF0");
        assert_eq!(split_frames(&mut buf), vec!["FA014250000", "MD02"]);
        assert_eq!(buf.trim(), "IF0");
    }

    #[test]
    fn frequency_bounds() {
        assert!(check_frequency(29_999).is_err());
        assert_eq!(check_frequency(30_000), Ok(30_000));
        assert_eq!(check_frequency(470_000_000), Ok(470_000_000));
        assert!(check_frequency(470_000_001).is_err());
    }
}
