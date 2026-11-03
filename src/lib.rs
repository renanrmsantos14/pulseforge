pub mod audio;
pub mod autostart;
pub mod config;
pub mod output_rules;
pub mod profiles;
#[cfg(feature = "gui")]
pub mod ui;
#[cfg(feature = "gui")]
pub mod tray;
pub mod updater;

pub use audio::dsp::{Biquad, EqualizerChain};
pub use config::model::{AppConfig, Band, OutputRule, Profile, SinkInfo};
