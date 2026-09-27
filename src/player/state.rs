#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    Queue,
    Song,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackStatus {
    Loading,
    Playing,
    Paused,
    Stopped,
    Finished,
    Error,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::Queue,
            Self::Queue => Self::Song,
            Self::Song => Self::Off,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub position: u64,
    pub duration: u64,
    pub volume: u8,
    pub is_playing: bool,
    pub status: PlaybackStatus,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub seek_step: u64,
    pub large_seek_step: u64,
}

impl PlayerState {
    pub fn new(volume: u8, seek_step: u64, large_seek_step: u64) -> Self {
        Self {
            position: 0,
            duration: 0,
            volume: volume.min(100),
            is_playing: false,
            status: PlaybackStatus::Stopped,
            muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            seek_step,
            large_seek_step,
        }
    }

    pub fn seek_by(&mut self, amount: i64) {
        self.position = if amount.is_negative() {
            self.position.saturating_sub(amount.unsigned_abs())
        } else {
            self.position
                .saturating_add(amount as u64)
                .min(self.duration)
        };
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerState, RepeatMode};

    #[test]
    fn seek_and_repeat_are_bounded() {
        let mut state = PlayerState::new(70, 5, 15);
        state.duration = 10;
        state.seek_by(15);
        assert_eq!(state.position, 10);
        state.seek_by(-20);
        assert_eq!(state.position, 0);
        assert_eq!(RepeatMode::Off.cycle(), RepeatMode::Queue);
    }
}
