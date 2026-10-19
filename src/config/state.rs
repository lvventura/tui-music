use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::errors::AppResult;
use crate::player::state::RepeatMode;

pub const DEFAULT_VOLUME: u8 = 70;

const DEFAULT_THEME: &str = "cover";
const DEFAULT_VIEW: &str = "search";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub volume: u8,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: String,
    pub theme: String,
    pub keymap: String,
    pub language: String,
    pub last_view: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            volume: DEFAULT_VOLUME,
            muted: false,
            shuffle: false,
            repeat: "Off".into(),
            theme: DEFAULT_THEME.into(),
            keymap: "default".into(),
            language: String::new(),
            last_view: DEFAULT_VIEW.into(),
        }
    }
}

impl State {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        crate::config::paths::ensure_parent(path)?;
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Volume to start with, plus whether the stored value had to be lifted
    /// back to an audible level (the caller localises the warning).
    pub fn starting_volume(&self) -> (u8, bool) {
        if self.volume == 0 {
            (DEFAULT_VOLUME, true)
        } else {
            (self.volume.min(100), false)
        }
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        match self.repeat.as_str() {
            "Queue" => RepeatMode::Queue,
            "Song" => RepeatMode::Song,
            _ => RepeatMode::Off,
        }
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat = format!("{mode:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_VOLUME, State};

    #[test]
    fn a_saved_zero_volume_does_not_come_back_silent() {
        let state = State {
            volume: 0,
            ..State::default()
        };
        let (volume, warning) = state.starting_volume();
        assert!(volume > 0, "a stored zero volume must not start silent");
        assert!(warning, "the silent restore must be announced");
        assert_ne!(volume, 0);
    }

    #[test]
    fn a_normal_volume_is_kept() {
        let state = State {
            volume: 42,
            ..State::default()
        };
        assert_eq!(state.starting_volume(), (42, false));
    }

    #[test]
    fn default_volume_is_audible() {
        assert_eq!(State::default().volume, DEFAULT_VOLUME);
    }
}
