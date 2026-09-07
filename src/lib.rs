pub mod core;
pub mod gui;

pub use core::judge::judge;
pub use core::model::{Record, RecordError, Settings, SettingsError};
pub use core::solver::{enumerate_space, filter_candidates, solve, suspect_records, SolveOutcome};
pub use core::strategy::{recommend, Bound, Recommendation};
