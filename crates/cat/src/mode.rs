use serde::{Deserialize, Serialize};

use crate::FilterGroup;

/// Bedrijfsmode zoals `MD0x;` en de IF-statusregel die kennen.
///
/// In JSON (naar de UI) heten ze zoals op het display van de radio,
/// bijvoorbeeld `"USB"`, `"CW-R"` en `"DATA-FM"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum Mode {
    Lsb,
    Usb,
    Cw,
    Fm,
    Am,
    RttyLsb,
    CwR,
    DataLsb,
    RttyUsb,
    DataFm,
    FmN,
    DataUsb,
    AmN,
    C4fm,
}

impl Mode {
    pub const ALL: [Mode; 14] = [
        Mode::Lsb,
        Mode::Usb,
        Mode::Cw,
        Mode::Fm,
        Mode::Am,
        Mode::RttyLsb,
        Mode::CwR,
        Mode::DataLsb,
        Mode::RttyUsb,
        Mode::DataFm,
        Mode::FmN,
        Mode::DataUsb,
        Mode::AmN,
        Mode::C4fm,
    ];

    /// Het ene teken dat de radio voor deze mode gebruikt.
    pub fn code(self) -> char {
        match self {
            Mode::Lsb => '1',
            Mode::Usb => '2',
            Mode::Cw => '3',
            Mode::Fm => '4',
            Mode::Am => '5',
            Mode::RttyLsb => '6',
            Mode::CwR => '7',
            Mode::DataLsb => '8',
            Mode::RttyUsb => '9',
            Mode::DataFm => 'A',
            Mode::FmN => 'B',
            Mode::DataUsb => 'C',
            Mode::AmN => 'D',
            Mode::C4fm => 'E',
        }
    }

    pub fn from_code(c: char) -> Option<Mode> {
        let c = c.to_ascii_uppercase();
        Mode::ALL.into_iter().find(|m| m.code() == c)
    }

    /// Naam zoals op het display.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Lsb => "LSB",
            Mode::Usb => "USB",
            Mode::Cw => "CW",
            Mode::Fm => "FM",
            Mode::Am => "AM",
            Mode::RttyLsb => "RTTY-LSB",
            Mode::CwR => "CW-R",
            Mode::DataLsb => "DATA-LSB",
            Mode::RttyUsb => "RTTY-USB",
            Mode::DataFm => "DATA-FM",
            Mode::FmN => "FM-N",
            Mode::DataUsb => "DATA-USB",
            Mode::AmN => "AM-N",
            Mode::C4fm => "C4FM",
        }
    }

    /// Welke bandbreedtetabel bij deze mode hoort. `None` voor FM, AM en C4FM:
    /// daar is de bandbreedte vast en kies je smal via de mode (FM-N, AM-N).
    pub fn filter_group(self) -> Option<FilterGroup> {
        match self {
            Mode::Lsb | Mode::Usb => Some(FilterGroup::Ssb),
            Mode::Cw | Mode::CwR => Some(FilterGroup::Cw),
            Mode::RttyLsb | Mode::RttyUsb | Mode::DataLsb | Mode::DataUsb => {
                Some(FilterGroup::Data)
            }
            Mode::Fm | Mode::Am | Mode::DataFm | Mode::FmN | Mode::AmN | Mode::C4fm => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for m in Mode::ALL {
            assert_eq!(Mode::from_code(m.code()), Some(m));
        }
        assert_eq!(Mode::from_code('a'), Some(Mode::DataFm));
        assert_eq!(Mode::from_code('F'), None);
    }

    #[test]
    fn json_uses_display_names() {
        for m in Mode::ALL {
            let json = serde_json::to_string(&m).unwrap();
            assert_eq!(json, format!("\"{}\"", m.label()));
        }
    }
}
