use crate::{CatError, Command, InfoLine, Mode, Vfo};

/// Een bericht dat van de radio komt: een antwoord op een vraag, of een
/// melding die de radio zelf stuurt na `AI1;`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    /// `ID0670;`
    Id(String),
    AutoInfo(bool),
    Frequency(Vfo, u32),
    Mode(Mode),
    Width(u8),
    Narrow(bool),
    IfShift(i16),
    Info(Vfo, InfoLine),
    /// `?;`: de radio weigert het commando (bijv. niet toegestaan in deze mode).
    Rejected,
    /// Een geldig bericht dat de app (nog) niet gebruikt, zoals `SM0123;`.
    Other(String),
}

impl Response {
    /// Leest één bericht (zonder `;`).
    pub fn parse(frame: &str) -> Result<Response, CatError> {
        let frame = frame.trim().trim_end_matches(';').to_ascii_uppercase();
        if frame == "?" {
            return Ok(Response::Rejected);
        }
        if frame.len() < 2 || !frame.is_ascii() {
            return Err(CatError::Unparsable(frame));
        }
        let (letters, rest) = frame.split_at(2);
        let response = match letters {
            "ID" => Response::Id(rest.to_string()),
            "IF" => Response::Info(Vfo::A, InfoLine::parse(rest)?),
            "OI" => Response::Info(Vfo::B, InfoLine::parse(rest)?),
            // Een antwoord heeft dezelfde vorm als het Set-commando, dus de
            // commando-parser leest de rest.
            "AI" | "FA" | "FB" | "MD" | "SH" | "NA" | "IS" => match Command::parse(&frame)? {
                Command::SetAutoInfo(on) => Response::AutoInfo(on),
                Command::SetFrequency(vfo, hz) => Response::Frequency(vfo, hz),
                Command::SetMode(mode) => Response::Mode(mode),
                Command::SetWidth(index) => Response::Width(index),
                Command::SetNarrow(on) => Response::Narrow(on),
                Command::SetIfShift(hz) => Response::IfShift(hz),
                _ => return Err(CatError::Unparsable(frame)),
            },
            _ => Response::Other(frame),
        };
        Ok(response)
    }

    /// Schrijft het antwoord zoals de radio het stuurt (gebruikt door de simulator).
    pub fn encode(&self) -> String {
        let as_set = |c: Command| c.encode().expect("waarde komt uit geldige toestand");
        match self {
            Response::Id(id) => format!("ID{id};"),
            Response::AutoInfo(on) => as_set(Command::SetAutoInfo(*on)),
            Response::Frequency(vfo, hz) => as_set(Command::SetFrequency(*vfo, *hz)),
            Response::Mode(m) => as_set(Command::SetMode(*m)),
            Response::Width(i) => as_set(Command::SetWidth(*i)),
            Response::Narrow(on) => as_set(Command::SetNarrow(*on)),
            Response::IfShift(hz) => as_set(Command::SetIfShift(*hz)),
            Response::Info(Vfo::A, line) => format!("IF{};", line.encode()),
            Response::Info(Vfo::B, line) => format!("OI{};", line.encode()),
            Response::Rejected => "?;".to_string(),
            Response::Other(raw) => format!("{raw};"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_answers() {
        assert_eq!(Response::parse("ID0670"), Ok(Response::Id("0670".into())));
        assert_eq!(
            Response::parse("FA014250000;"),
            Ok(Response::Frequency(Vfo::A, 14_250_000))
        );
        assert_eq!(Response::parse("md0b"), Ok(Response::Mode(Mode::FmN)));
        assert_eq!(Response::parse("SH016"), Ok(Response::Width(16)));
        assert_eq!(Response::parse("NA01"), Ok(Response::Narrow(true)));
        assert_eq!(Response::parse("IS0-0060"), Ok(Response::IfShift(-60)));
        assert_eq!(Response::parse("?"), Ok(Response::Rejected));
        assert_eq!(
            Response::parse("SM0123"),
            Ok(Response::Other("SM0123".into()))
        );
    }

    #[test]
    fn encode_then_parse() {
        for r in [
            Response::Frequency(Vfo::B, 145_500_000),
            Response::Mode(Mode::CwR),
            Response::Width(0),
            Response::IfShift(1200),
            Response::Rejected,
        ] {
            assert_eq!(Response::parse(&r.encode()), Ok(r));
        }
    }

    #[test]
    fn broken_answer_is_an_error() {
        assert!(Response::parse("FA12").is_err());
        assert!(Response::parse("IF001").is_err());
    }
}
