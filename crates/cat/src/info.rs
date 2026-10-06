use serde::{Deserialize, Serialize};

use crate::{CatError, Mode};

/// Waar de radio op staat: VFO, geheugen, QMB, enz. (positie 23 van `IF`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    Vfo,
    Memory,
    MemoryTune,
    Qmb,
    QmbMemoryTune,
    Pms,
    Home,
}

impl MemoryMode {
    const CODES: [MemoryMode; 7] = [
        MemoryMode::Vfo,
        MemoryMode::Memory,
        MemoryMode::MemoryTune,
        MemoryMode::Qmb,
        MemoryMode::QmbMemoryTune,
        MemoryMode::Pms,
        MemoryMode::Home,
    ];
    fn from_digit(d: u8) -> Option<Self> {
        Self::CODES.get(d as usize).copied()
    }
    fn digit(self) -> u8 {
        Self::CODES.iter().position(|m| *m == self).unwrap() as u8
    }
}

/// Toonmode zoals `CT0x;` (positie 24 van `IF`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneMode {
    Off,
    CtcssEncDec,
    CtcssEnc,
    DcsEncDec,
    DcsEnc,
}

impl ToneMode {
    const CODES: [ToneMode; 5] = [
        ToneMode::Off,
        ToneMode::CtcssEncDec,
        ToneMode::CtcssEnc,
        ToneMode::DcsEncDec,
        ToneMode::DcsEnc,
    ];
    fn from_digit(d: u8) -> Option<Self> {
        Self::CODES.get(d as usize).copied()
    }
    fn digit(self) -> u8 {
        Self::CODES.iter().position(|m| *m == self).unwrap() as u8
    }
}

/// Repeatershift zoals `OS0x;` (positie 27 van `IF`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeaterShift {
    Simplex,
    Plus,
    Minus,
}

impl RepeaterShift {
    const CODES: [RepeaterShift; 3] = [
        RepeaterShift::Simplex,
        RepeaterShift::Plus,
        RepeaterShift::Minus,
    ];
    fn from_digit(d: u8) -> Option<Self> {
        Self::CODES.get(d as usize).copied()
    }
    fn digit(self) -> u8 {
        Self::CODES.iter().position(|m| *m == self).unwrap() as u8
    }
}

/// De statusregel die `IF;` (VFO-A) en `OI;` (VFO-B) teruggeven.
///
/// Opbouw na de twee letters: kanaal (3), frequentie (9), clarifier (teken + 4),
/// RX-clar (1), TX-clar (1), mode (1), VFO/geheugen (1), toonmode (1), `00`, shift (1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoLine {
    pub memory_channel: u16,
    pub frequency_hz: u32,
    pub clarifier_hz: i16,
    pub rx_clarifier: bool,
    pub tx_clarifier: bool,
    pub mode: Mode,
    pub memory_mode: MemoryMode,
    pub tone_mode: ToneMode,
    pub shift: RepeaterShift,
}

/// Lengte van alles na de twee letters.
const BODY_LEN: usize = 25;

impl InfoLine {
    /// Leest het deel ná `IF` of `OI`.
    pub fn parse(body: &str) -> Result<InfoLine, CatError> {
        let bad = || CatError::Unparsable(body.to_string());
        if body.len() != BODY_LEN || !body.is_ascii() {
            return Err(bad());
        }
        let num = |range: std::ops::Range<usize>| body[range].parse::<u32>().map_err(|_| bad());
        let digit = |pos: usize| {
            body.as_bytes()[pos]
                .is_ascii_digit()
                .then(|| body.as_bytes()[pos] - b'0')
                .ok_or_else(bad)
        };
        let flag = |pos: usize| match digit(pos)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(bad()),
        };

        let sign = match &body[12..13] {
            "+" => 1,
            "-" => -1,
            _ => return Err(bad()),
        };
        if &body[22..24] != "00" {
            return Err(bad());
        }

        Ok(InfoLine {
            memory_channel: num(0..3)? as u16,
            frequency_hz: num(3..12)?,
            clarifier_hz: sign * num(13..17)? as i16,
            rx_clarifier: flag(17)?,
            tx_clarifier: flag(18)?,
            mode: Mode::from_code(body.as_bytes()[19] as char).ok_or_else(bad)?,
            memory_mode: MemoryMode::from_digit(digit(20)?).ok_or_else(bad)?,
            tone_mode: ToneMode::from_digit(digit(21)?).ok_or_else(bad)?,
            shift: RepeaterShift::from_digit(digit(24)?).ok_or_else(bad)?,
        })
    }

    /// Schrijft het deel ná `IF`/`OI` (gebruikt door de simulator).
    pub fn encode(&self) -> String {
        format!(
            "{:03}{:09}{}{:04}{}{}{}{}{}00{}",
            self.memory_channel,
            self.frequency_hz,
            if self.clarifier_hz < 0 { '-' } else { '+' },
            self.clarifier_hz.unsigned_abs(),
            self.rx_clarifier as u8,
            self.tx_clarifier as u8,
            self.mode.code(),
            self.memory_mode.digit(),
            self.tone_mode.digit(),
            self.shift.digit(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_line() {
        // 70 cm repeater: 438,650 MHz, FM, CTCSS-toon, min-shift.
        let line = InfoLine::parse("001438650000+000000402002").unwrap();
        assert_eq!(line.memory_channel, 1);
        assert_eq!(line.frequency_hz, 438_650_000);
        assert_eq!(line.clarifier_hz, 0);
        assert_eq!(line.mode, Mode::Fm);
        assert_eq!(line.memory_mode, MemoryMode::Vfo);
        assert_eq!(line.tone_mode, ToneMode::CtcssEnc);
        assert_eq!(line.shift, RepeaterShift::Minus);
    }

    #[test]
    fn round_trips() {
        let line = InfoLine {
            memory_channel: 117,
            frequency_hz: 14_250_000,
            clarifier_hz: -120,
            rx_clarifier: true,
            tx_clarifier: false,
            mode: Mode::DataUsb,
            memory_mode: MemoryMode::Memory,
            tone_mode: ToneMode::Off,
            shift: RepeaterShift::Simplex,
        };
        let body = line.encode();
        assert_eq!(body, "117014250000-012010C10000");
        assert_eq!(InfoLine::parse(&body), Ok(line));
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(InfoLine::parse("001438650000+00000040200").is_err());
    }
}
