use crate::app::state::View;
use crate::ui::cover::CoverImage;
use crate::youtube::models::Song;
use crate::youtube::provider::AudioStream;
use crossterm::event::KeyEvent;

pub enum Event {
    Key(KeyEvent),
    SearchFinished(Result<Vec<Song>, String>),
    AudioReady(Result<AudioStream, String>),
    RelatedFinished(Result<Vec<Song>, String>),
    AuthFinished(Result<(), String>),
    CoverReady(Result<CoverImage, String>),
}

pub enum Action {
    Search(String),
    Select(Song),
    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    SetVolume(u8),
    Quit,
    ToggleFavorite,
    SwitchView(View),
    CreatePlaylist,
    AddToPlaylist,
    DeleteSelected,
    OpenPlaylist(String),
    SeekForward(bool),
    SeekBackward(bool),
    SeekTo(u64),
    SelectIndex(usize),
    LibraryClick { column: usize, index: usize },
    ScrollSelection(i32),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    MoveQueueItem { from: usize, to: usize },
    PlayNext(Song),
    PlayNow(Song),
    Login,
}
