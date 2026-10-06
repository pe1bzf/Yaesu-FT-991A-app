use crate::{check_frequency, CatError, Mode, Vfo, IF_SHIFT_MAX_HZ, TERMINATOR};

/// De commando's die de app nu naar de radio stuurt.
///
/// `Read…` vraagt een waarde op, `Set…` zet hem. Een Set geeft geen antwoord
/// (alleen `?;` als de radio hem weigert); lees daarna terug om te bevestigen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// `ID;` → `ID0670;`
    ReadId,
    /// `AI0;` / `AI1;`: radio meldt zelf wijzigingen (bijv. draaien aan de knop).
    SetAutoInfo(bool),
    ReadAutoInfo,
    /// `FA;` / `FB;`
    ReadFrequency(Vfo),
    /// `FA014250000;`
    SetFrequency(Vfo, u32),
    /// `MD0;`
    ReadMode,
    /// `MD02;`
    SetMode(Mode),
    /// `SH0;` bandbreedte-index
    ReadWidth,
    /// `SH016;`
    SetWidth(u8),
    /// `NA0;` narrow
    ReadNarrow,
    /// `NA01;`
    SetNarrow(bool),
    /// `IS0;` IF-shift
    ReadIfShift,
    /// `IS0+0100;`
    SetIfShift(i16),
    /// `IF;` (VFO-A) of `OI;` (VFO-B): statusregel
    ReadInfo(Vfo),
}

fn freq_letters(vfo: Vfo) -> &'static str {
    match vfo {
        Vfo::A => "FA",
        Vfo::B => "FB",
    }
}

fn info_letters(vfo: Vfo) -> &'static str {
    match vfo {
        Vfo::A => "IF",
        Vfo::B => "OI",
    }
}

impl Command {
    /// Bouwt de tekst voor de radio, inclusief `;`. Controleert het bereik,
    /// zodat er nooit een half of fout commando de deur uit gaat.
    pub fn encode(&self) -> Result<String, CatError> {
        let body = match *self {
            Command::ReadId => "ID".to_string(),
            Command::SetAutoInfo(on) => format!("AI{}", on as u8),
            Command::ReadAutoInfo => "AI".to_string(),
            Command::ReadFrequency(vfo) => freq_letters(vfo).to_string(),
            Command::SetFrequency(vfo, hz) => {
                let hz = check_frequency(hz as u64)?;
                format!("{}{:09}", freq_letters(vfo), hz)
            }
            Command::ReadMode => "MD0".to_string(),
            Command::SetMode(mode) => format!("MD0{}", mode.code()),
            Command::ReadWidth => "SH0".to_string(),
            Command::SetWidth(index) => {
                if index > 21 {
                    return Err(CatError::WidthOutOfRange(index));
                }
                format!("SH0{index:02}")
            }
            Command::ReadNarrow => "NA0".to_string(),
            Command::SetNarrow(on) => format!("NA0{}", on as u8),
            Command::ReadIfShift => "IS0".to_string(),
            Command::SetIfShift(hz) => {
                if hz.abs() > IF_SHIFT_MAX_HZ {
                    return Err(CatError::IfShiftOutOfRange(hz as i32));
                }
                let sign = if hz < 0 { '-' } else { '+' };
                format!("IS0{sign}{:04}", hz.unsigned_abs())
            }
            Command::ReadInfo(vfo) => info_letters(vfo).to_string(),
        };
        Ok(format!("{body}{TERMINATOR}"))
    }

    /// De letters waarmee het antwoord begint, of `None` als er geen antwoord komt.
    pub fn answer_prefix(&self) -> Option<&'static str> {
        match *self {
            Command::ReadId => Some("ID"),
            Command::ReadAutoInfo => Some("AI"),
            Command::ReadFrequency(vfo) => Some(freq_letters(vfo)),
            Command::ReadMode => Some("MD"),
            Command::ReadWidth => Some("SH"),
            Command::ReadNarrow => Some("NA"),
            Command::ReadIfShift => Some("IS"),
            Command::ReadInfo(vfo) => Some(info_letters(vfo)),
            Command::SetAutoInfo(_)
            | Command::SetFrequency(..)
            | Command::SetMode(_)
            | Command::SetWidth(_)
            | Command::SetNarrow(_)
            | Command::SetIfShift(_) => None,
        }
    }

    /// Leest een commando zoals de radio het ontvangt (zonder `;`).
    /// Gebruikt door de simulator; later ook nuttig voor een rigctld-brug.
    pub fn parse(frame: &str) -> Result<Command, CatError> {
        let frame = frame
            .trim()
            .trim_end_matches(TERMINATOR)
            .to_ascii_uppercase();
        let bad = || CatError::Unparsable(frame.clone());
        if frame.len() < 2 || !frame.is_ascii() {
            return Err(bad());
        }
        let (letters, rest) = frame.split_at(2);
        let digits = |s: &str, n: usize| -> Result<u32, CatError> {
            if s.len() == n && s.bytes().all(|b| b.is_ascii_digit()) {
                s.parse().map_err(|_| bad())
            } else {
                Err(bad())
            }
        };
        let flag = |s: &str| match s {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(bad()),
        };
        // Commando's met een vaste "0" na de letters (MD0, SH0, NA0, IS0).
        let after_zero = || rest.strip_prefix('0').ok_or_else(bad);

        let cmd = match letters {
            "ID" if rest.is_empty() => Command::ReadId,
            "AI" if rest.is_empty() => Command::ReadAutoInfo,
            "AI" => Command::SetAutoInfo(flag(rest)?),
            "FA" | "FB" => {
                let vfo = if letters == "FA" { Vfo::A } else { Vfo::B };
                if rest.is_empty() {
                    Command::ReadFrequency(vfo)
                } else {
                    Command::SetFrequency(vfo, check_frequency(digits(rest, 9)? as u64)?)
                }
            }
            "IF" if rest.is_empty() => Command::ReadInfo(Vfo::A),
            "OI" if rest.is_empty() => Command::ReadInfo(Vfo::B),
            "MD" => match after_zero()? {
                "" => Command::ReadMode,
                code if code.len() == 1 => {
                    Command::SetMode(Mode::from_code(code.chars().next().unwrap()).ok_or_else(bad)?)
                }
                _ => return Err(bad()),
            },
            "SH" => match after_zero()? {
                "" => Command::ReadWidth,
                idx => {
                    let idx = digits(idx, 2)? as u8;
                    if idx > 21 {
                        return Err(CatError::WidthOutOfRange(idx));
                    }
                    Command::SetWidth(idx)
                }
            },
            "NA" => match after_zero()? {
                "" => Command::ReadNarrow,
                on => Command::SetNarrow(flag(on)?),
            },
            "IS" => match after_zero()? {
                "" => Command::ReadIfShift,
                value => {
                    let (sign, num) = value.split_at(1.min(value.len()));
                    let hz = digits(num, 4)? as i16;
                    let hz = match sign {
                        "+" => hz,
                        "-" => -hz,
                        _ => return Err(bad()),
                    };
                    if hz.abs() > IF_SHIFT_MAX_HZ {
                        return Err(CatError::IfShiftOutOfRange(hz as i32));
                    }
                    Command::SetIfShift(hz)
                }
            },
            _ => return Err(bad()),
        };
        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(c: Command) -> String {
        c.encode().unwrap()
    }

    #[test]
    fn encodes_with_fixed_digit_counts() {
        assert_eq!(
            enc(Command::SetFrequency(Vfo::A, 14_250_000)),
            "FA014250000;"
        );
        assert_eq!(
            enc(Command::SetFrequency(Vfo::B, 438_650_000)),
            "FB438650000;"
        );
        assert_eq!(enc(Command::SetMode(Mode::Usb)), "MD02;");
        assert_eq!(enc(Command::SetMode(Mode::C4fm)), "MD0E;");
        assert_eq!(enc(Command::SetWidth(6)), "SH006;");
        assert_eq!(enc(Command::SetNarrow(true)), "NA01;");
        assert_eq!(enc(Command::SetIfShift(100)), "IS0+0100;");
        assert_eq!(enc(Command::SetIfShift(-1200)), "IS0-1200;");
        assert_eq!(enc(Command::SetIfShift(0)), "IS0+0000;");
        assert_eq!(enc(Command::ReadInfo(Vfo::B)), "OI;");
        assert_eq!(enc(Command::SetAutoInfo(true)), "AI1;");
    }

    #[test]
    fn refuses_out_of_range_values() {
        assert!(Command::SetFrequency(Vfo::A, 10_000).encode().is_err());
        assert!(Command::SetIfShift(1220).encode().is_err());
        assert!(Command::SetWidth(22).encode().is_err());
    }

    #[test]
    fn parse_is_the_inverse_of_encode() {
        let all = [
            Command::ReadId,
            Command::SetAutoInfo(true),
            Command::ReadAutoInfo,
            Command::ReadFrequency(Vfo::A),
            Command::SetFrequency(Vfo::B, 7_074_000),
            Command::ReadMode,
            Command::SetMode(Mode::DataFm),
            Command::ReadWidth,
            Command::SetWidth(21),
            Command::ReadNarrow,
            Command::SetNarrow(false),
            Command::ReadIfShift,
            Command::SetIfShift(-40),
            Command::ReadInfo(Vfo::A),
            Command::ReadInfo(Vfo::B),
        ];
        for cmd in all {
            assert_eq!(Command::parse(&enc(cmd)), Ok(cmd), "{}", enc(cmd));
        }
    }

    #[test]
    fn parse_rejects_wrong_digit_counts() {
        assert!(Command::parse("FA14250000;").is_err());
        assert!(Command::parse("SH06;").is_err());
        assert!(Command::parse("IS0100;").is_err());
        assert!(Command::parse("MD0F;").is_err());
        assert!(Command::parse("XX;").is_err());
    }

    #[test]
    fn read_commands_expect_an_answer() {
        assert_eq!(Command::ReadMode.answer_prefix(), Some("MD"));
        assert_eq!(Command::SetMode(Mode::Fm).answer_prefix(), None);
    }
}
