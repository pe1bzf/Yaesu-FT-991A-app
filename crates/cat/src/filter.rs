use serde::{Deserialize, Serialize};

use crate::Mode;

/// De drie bandbreedtetabellen van `SH`. Welke geldt hangt af van de mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterGroup {
    /// LSB, USB
    Ssb,
    /// CW, CW-R
    Cw,
    /// RTTY-LSB/USB, DATA-LSB/USB (PSK)
    Data,
}

/// Eén keuze in de bandbreedtelijst: de `SH`-index en wat die betekent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidthOption {
    pub index: u8,
    pub hz: u32,
}

const SSB_NARROW: &[(u8, u32)] = &[
    (0, 1500),
    (1, 200),
    (2, 400),
    (3, 600),
    (4, 850),
    (5, 1100),
    (6, 1350),
    (7, 1500),
    (8, 1650),
    (9, 1800),
];
const SSB_WIDE: &[(u8, u32)] = &[
    (0, 2400),
    (9, 1800),
    (10, 1950),
    (11, 2100),
    (12, 2200),
    (13, 2300),
    (14, 2400),
    (15, 2500),
    (16, 2600),
    (17, 2700),
    (18, 2800),
    (19, 2900),
    (20, 3000),
    (21, 3200),
];
const CW_NARROW: &[(u8, u32)] = &[
    (0, 500),
    (1, 50),
    (2, 100),
    (3, 150),
    (4, 200),
    (5, 250),
    (6, 300),
    (7, 350),
    (8, 400),
    (9, 450),
    (10, 500),
];
const CW_WIDE: &[(u8, u32)] = &[
    (0, 2400),
    (10, 500),
    (11, 800),
    (12, 1200),
    (13, 1400),
    (14, 1700),
    (15, 2000),
    (16, 2400),
    (17, 3000),
];
const DATA_NARROW: &[(u8, u32)] = &[
    (0, 300),
    (1, 50),
    (2, 100),
    (3, 150),
    (4, 200),
    (5, 250),
    (6, 300),
    (7, 350),
    (8, 400),
    (9, 450),
    (10, 500),
];
const DATA_WIDE: &[(u8, u32)] = &[
    (0, 500),
    (10, 500),
    (11, 800),
    (12, 1200),
    (13, 1400),
    (14, 1700),
    (15, 2000),
    (16, 2400),
    (17, 3000),
];

fn table(group: FilterGroup, narrow: bool) -> &'static [(u8, u32)] {
    match (group, narrow) {
        (FilterGroup::Ssb, true) => SSB_NARROW,
        (FilterGroup::Ssb, false) => SSB_WIDE,
        (FilterGroup::Cw, true) => CW_NARROW,
        (FilterGroup::Cw, false) => CW_WIDE,
        (FilterGroup::Data, true) => DATA_NARROW,
        (FilterGroup::Data, false) => DATA_WIDE,
    }
}

/// Bandbreedte in Hz voor een `SH`-index, of `None` als die index in deze
/// mode/narrow-stand niet bestaat.
pub fn bandwidth_hz(group: FilterGroup, narrow: bool, index: u8) -> Option<u32> {
    table(group, narrow)
        .iter()
        .find(|(i, _)| *i == index)
        .map(|(_, hz)| *hz)
}

/// Alle geldige bandbreedtes voor een mode, of een lege lijst voor FM/AM/C4FM.
pub fn width_options(mode: Mode, narrow: bool) -> Vec<WidthOption> {
    match mode.filter_group() {
        Some(group) => table(group, narrow)
            .iter()
            .map(|&(index, hz)| WidthOption { index, hz })
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_known_widths() {
        assert_eq!(bandwidth_hz(FilterGroup::Ssb, false, 21), Some(3200));
        assert_eq!(bandwidth_hz(FilterGroup::Ssb, true, 4), Some(850));
        assert_eq!(bandwidth_hz(FilterGroup::Cw, true, 1), Some(50));
        assert_eq!(bandwidth_hz(FilterGroup::Data, false, 0), Some(500));
    }

    #[test]
    fn rejects_indexes_that_do_not_exist_in_that_table() {
        assert_eq!(bandwidth_hz(FilterGroup::Ssb, false, 3), None);
        assert_eq!(bandwidth_hz(FilterGroup::Cw, false, 18), None);
        assert_eq!(bandwidth_hz(FilterGroup::Ssb, true, 10), None);
    }

    #[test]
    fn fm_has_no_width_choice() {
        assert!(width_options(Mode::Fm, false).is_empty());
        assert_eq!(width_options(Mode::Usb, false).len(), 14);
    }
}
