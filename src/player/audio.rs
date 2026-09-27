use std::future::Future;
use std::io::BufReader;
use std::pin::Pin;

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use tracing::{debug, warn};

use crate::errors::{AppError, AppResult};
use crate::player::state::PlaybackStatus;
use crate::youtube::provider::AudioStream;

#[derive(Clone, Copy, Debug)]
pub struct PlayerSnapshot {
    pub position: u64,
    pub volume: f32,
    pub muted: bool,
    pub status: PlaybackStatus,
}

pub trait AudioPlayer {
    fn play(
        &mut self,
        audio: AudioStream,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>>;
    fn resume(&mut self);
    fn pause(&mut self);
    fn toggle_pause(&mut self) -> bool;
    fn stop(&mut self);
    fn set_volume(&mut self, volume: u8) -> AppResult<()>;
}

pub struct RodioPlayer {
    _stream: OutputStream,
    sink: Sink,
    volume: f32,
    muted: bool,
}

impl RodioPlayer {
    pub fn new() -> AppResult<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|error| AppError::Player(error.to_string()))?;
        let sink = Sink::connect_new(stream.mixer());
        Ok(Self {
            _stream: stream,
            sink,
            volume: 0.7,
            muted: false,
        })
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        let position = self.sink.get_pos().as_secs();
        let status = if self.sink.empty() {
            PlaybackStatus::Finished
        } else if self.sink.is_paused() {
            PlaybackStatus::Paused
        } else {
            PlaybackStatus::Playing
        };
        PlayerSnapshot {
            position,
            volume: self.volume,
            muted: self.muted,
            status,
        }
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.sink.set_volume(if muted { 0.0 } else { self.volume });
    }
}

impl AudioPlayer for RodioPlayer {
    fn play(
        &mut self,
        audio: AudioStream,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(async move {
            debug!(content_type = ?audio.content_type, "initializing audio decoder");
            let source = tokio::task::spawn_blocking(move || {
                Decoder::builder()
                    .with_data(BufReader::new(audio.reader()))
                    .with_seekable(false)
                    .build()
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| AppError::Player(format!("decoder task failed: {error}")))?
            .map_err(AppError::Player)?;
            debug!("audio decoder initialized");
            self.sink.stop();
            self.sink.append(source);
            self.sink.play();
            debug!(
                paused = self.sink.is_paused(),
                empty = self.sink.empty(),
                "audio playback started"
            );
            Ok(())
        })
    }

    fn pause(&mut self) {
        debug!(
            paused = self.sink.is_paused(),
            empty = self.sink.empty(),
            "pausing audio"
        );
        self.sink.pause();
        debug!(
            paused = self.sink.is_paused(),
            empty = self.sink.empty(),
            "audio paused"
        );
    }
    fn resume(&mut self) {
        debug!(
            paused = self.sink.is_paused(),
            empty = self.sink.empty(),
            "resuming audio"
        );
        if self.sink.empty() {
            warn!("cannot resume audio: sink is empty");
            return;
        }
        self.sink.play();
        debug!(
            paused = self.sink.is_paused(),
            empty = self.sink.empty(),
            "audio resumed"
        );
    }
    fn toggle_pause(&mut self) -> bool {
        let paused = self.sink.is_paused();
        debug!(paused, empty = self.sink.empty(), "toggling audio pause");
        if paused {
            self.resume();
        } else {
            self.pause();
        }
        !self.sink.is_paused() && !self.sink.empty()
    }
    fn stop(&mut self) {
        debug!("stopping audio");
        self.sink.stop();
    }
    fn set_volume(&mut self, volume: u8) -> AppResult<()> {
        self.volume = volume.min(100) as f32 / 100.0;
        self.sink
            .set_volume(if self.muted { 0.0 } else { self.volume });
        Ok(())
    }
}
