//! Settings / Record 数据模型与校验(§4.1、§4.5)。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub colors: usize,
    pub slots: usize,
    pub repeats: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError { ColorsOutOfRange, SlotsOutOfRange, SlotsExceedColors }

impl Default for Settings {
    fn default() -> Self {
        Self { colors: 6, slots: 4, repeats: true }
    }
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ColorsOutOfRange => write!(f, "颜色数必须在 4..=8"),
            Self::SlotsOutOfRange => write!(f, "槽位数必须在 3..=6"),
            Self::SlotsExceedColors => write!(f, "不允许重复时槽位数不能超过颜色数"),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !(4..=8).contains(&self.colors) {
            return Err(SettingsError::ColorsOutOfRange);
        }
        if !(3..=6).contains(&self.slots) {
            return Err(SettingsError::SlotsOutOfRange);
        }
        if !self.repeats && self.slots > self.colors {
            return Err(SettingsError::SlotsExceedColors);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub guess: Vec<u8>,
    pub exact: u8,
    pub partial: u8,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordError { WrongLength, ColorOutOfRange, CountsTooLarge }

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongLength => write!(f, "宝石数必须等于槽位数"),
            Self::ColorOutOfRange => write!(f, "宝石索引超出颜色数范围"),
            Self::CountsTooLarge => write!(f, "蓝标数+金标数不能超过槽位数"),
        }
    }
}

impl Record {
    pub fn new(guess: Vec<u8>, exact: u8, partial: u8) -> Record {
        Record { guess, exact, partial, enabled: true }
    }

    pub fn validate(&self, settings: &Settings) -> Result<(), RecordError> {
        if self.guess.len() != settings.slots {
            return Err(RecordError::WrongLength);
        }
        if self.guess.iter().any(|&g| g as usize >= settings.colors) {
            return Err(RecordError::ColorOutOfRange);
        }
        if self.exact as usize + self.partial as usize > settings.slots {
            return Err(RecordError::CountsTooLarge);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_matches_game() {
        let s = Settings::default();
        assert_eq!((s.colors, s.slots, s.repeats), (6, 4, true));
    }

    #[test]
    fn settings_validate_bounds() {
        assert!(Settings { colors: 4, slots: 3, repeats: true }.validate().is_ok());
        assert!(Settings { colors: 8, slots: 6, repeats: true }.validate().is_ok());
        assert_eq!(Settings { colors: 3, slots: 4, repeats: true }.validate(), Err(SettingsError::ColorsOutOfRange));
        assert_eq!(Settings { colors: 9, slots: 4, repeats: true }.validate(), Err(SettingsError::ColorsOutOfRange));
        assert_eq!(Settings { colors: 6, slots: 2, repeats: true }.validate(), Err(SettingsError::SlotsOutOfRange));
        assert_eq!(Settings { colors: 6, slots: 7, repeats: true }.validate(), Err(SettingsError::SlotsOutOfRange));
    }

    #[test]
    fn settings_no_repeat_requires_slots_le_colors() {
        assert_eq!(
            Settings { colors: 4, slots: 5, repeats: false }.validate(),
            Err(SettingsError::SlotsExceedColors)
        );
        assert!(Settings { colors: 6, slots: 4, repeats: false }.validate().is_ok());
    }

    #[test]
    fn record_new_defaults_enabled() {
        let r = Record::new(vec![1, 2, 3, 4], 1, 0);
        assert!(r.enabled);
        assert_eq!(r.guess, vec![1, 2, 3, 4]);
        assert_eq!((r.exact, r.partial), (1, 0));
    }

    #[test]
    fn record_validate() {
        let s = Settings::default();
        assert!(Record::new(vec![0, 1, 2, 3], 4, 0).validate(&s).is_ok());
        assert!(Record::new(vec![5, 5, 5, 5], 2, 2).validate(&s).is_ok());
        assert_eq!(
            Record::new(vec![0, 1, 2], 0, 0).validate(&s),
            Err(RecordError::WrongLength)
        );
        assert_eq!(
            Record::new(vec![0, 1, 2, 6], 0, 0).validate(&s),
            Err(RecordError::ColorOutOfRange)
        );
        // exact + partial > slots(3+2=5 > 4);同时覆盖 exact ≤ slots
        assert_eq!(
            Record::new(vec![0, 1, 2, 3], 3, 2).validate(&s),
            Err(RecordError::CountsTooLarge)
        );
        assert_eq!(
            Record::new(vec![0, 1, 2, 3], 5, 0).validate(&s),
            Err(RecordError::CountsTooLarge)
        );
    }
}
