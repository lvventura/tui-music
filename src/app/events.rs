use crate::youtube::models::Song;
use crate::youtube::provider::AudioStream;
use crossterm::event::KeyEvent;

pub enum Event {
    Key(KeyEvent),
    SearchFinished(Result<Vec<Song>, String>),
    AudioReady(Result<AudioStream, String>),
    RelatedFinished(Result<Vec<Song>, String>),
}

pub enum Action {
    Search(String),
    Select(Song),
    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    Quit,
    ToggleFavorite,
    Navigate(super::state::Screen),
    CreatePlaylist,
    AddToPlaylist,
    DeleteSelected,
    OpenPlaylist(String),
    SeekForward(bool),
    SeekBackward(bool),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    MoveQueueItem { from: usize, to: usize },
    PlayNext(Song),
    PlayNow(Song),
    Login,
}
