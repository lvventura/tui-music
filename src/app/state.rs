use crossterm::event::{KeyCode, KeyEventKind};

use crate::{
    app::{Action, Event},
    player::{
        audio::PlayerSnapshot,
        state::{PlaybackStatus, PlayerState},
    },
    youtube::models::Song,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Search,
    History,
    Favorites,
    Playlists,
    Queue,
}

pub struct App {
    pub current_song: Option<Song>,
    pub search_query: String,
    pub search_results: Vec<Song>,
    pub selected_result: usize,
    pub queue: Vec<Song>,
    pub queue_index: usize,
    pub is_playing: bool,
    pub volume: u8,
    pub current_position: u64,
    pub duration: u64,
    pub screen: Screen,
    pub search_mode: bool,
    pub searching: bool,
    pub status: String,
    pub has_audio: bool,
    pub should_quit: bool,
    pub favorites: Vec<Song>,
    pub history: Vec<Song>,
    pub playlists: Vec<(String, String)>,
    pub selected_item: usize,
    pub is_favorite: bool,
    pub selected_playlist: Option<String>,
    pub playlist_songs: Vec<Song>,
    pub player: PlayerState,
    pub debug_visible: bool,
    pub account_status: String,
}

impl App {
    pub fn new(volume: u8) -> Self {
        Self {
            current_song: None,
            search_query: String::new(),
            search_results: Vec::new(),
            selected_result: 0,
            queue: Vec::new(),
            queue_index: 0,
            is_playing: false,
            volume: volume.min(100),
            current_position: 0,
            duration: 0,
            screen: Screen::Search,
            search_mode: false,
            searching: false,
            status: "Pulsa / para buscar".into(),
            has_audio: false,
            should_quit: false,
            favorites: Vec::new(),
            history: Vec::new(),
            playlists: Vec::new(),
            selected_item: 0,
            is_favorite: false,
            selected_playlist: None,
            playlist_songs: Vec::new(),
            player: PlayerState::new(volume, 5, 15),
            debug_visible: false,
            account_status: "Unauthenticated".into(),
        }
    }

    pub fn handle_event(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Press => None,
            Event::Key(key) => {
                if self.search_mode {
                    return match key.code {
                        KeyCode::Esc => {
                            self.search_mode = false;
                            None
                        }
                        KeyCode::Enter => {
                            self.search_mode = false;
                            if self.search_query.trim().is_empty() {
                                None
                            } else {
                                Some(Action::Search(self.search_query.clone()))
                            }
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                            None
                        }
                        KeyCode::Char(value) => {
                            self.search_query.push(value);
                            None
                        }
                        _ => None,
                    };
                }
                match key.code {
                    KeyCode::Char('q') => {
                        self.should_quit = true;
                        Some(Action::Quit)
                    }
                    KeyCode::Char('/') => {
                        self.search_mode = true;
                        None
                    }
                    KeyCode::Char('1') => Some(Action::Navigate(Screen::Search)),
                    KeyCode::Char('2') => Some(Action::Navigate(Screen::History)),
                    KeyCode::Char('3') => Some(Action::Navigate(Screen::Favorites)),
                    KeyCode::Char('4') => Some(Action::Navigate(Screen::Playlists)),
                    KeyCode::Char('5') => Some(Action::Navigate(Screen::Queue)),
                    KeyCode::Char('f') => Some(Action::ToggleFavorite),
                    KeyCode::Char('d') => Some(Action::DeleteSelected),
                    KeyCode::Char('c') => Some(Action::CreatePlaylist),
                    KeyCode::Char('a') => Some(Action::AddToPlaylist),
                    KeyCode::Char('m') => Some(Action::ToggleMute),
                    KeyCode::Char('l') => Some(Action::Login),
                    KeyCode::Char('D') => {
                        self.debug_visible = !self.debug_visible;
                        None
                    }
                    KeyCode::Char('r') => Some(Action::CycleRepeat),
                    KeyCode::Char('s') => Some(Action::ToggleShuffle),
                    KeyCode::Left => Some(Action::SeekBackward(
                        key.modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT),
                    )),
                    KeyCode::Right => Some(Action::SeekForward(
                        key.modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT),
                    )),
                    KeyCode::Char(' ') => Some(Action::PlayPause),
                    KeyCode::Char('n') => Some(Action::Next),
                    KeyCode::Char('p') => Some(Action::Previous),
                    KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::VolumeUp),
                    KeyCode::Char('-') => Some(Action::VolumeDown),
                    KeyCode::Up => {
                        self.selected_result = self.selected_result.saturating_sub(1);
                        None
                    }
                    KeyCode::Down => {
                        let length = match self.screen {
                            Screen::Search => self.search_results.len(),
                            Screen::History => self.history.len(),
                            Screen::Favorites => self.favorites.len(),
                            Screen::Queue => self.queue.len(),
                            Screen::Playlists => self.playlists.len(),
                        };
                        if length > 0 {
                            self.selected_result = (self.selected_result + 1).min(length - 1);
                        }
                        None
                    }
                    KeyCode::Enter => {
                        if self.screen == Screen::Playlists && self.selected_playlist.is_none() {
                            self.playlists
                                .get(self.selected_result)
                                .map(|(id, _)| Action::OpenPlaylist(id.clone()))
                        } else {
                            self.selected_song().map(Action::Select)
                        }
                    }
                    _ => None,
                }
            }
            Event::SearchFinished(result) => {
                self.searching = false;
                match result {
                    Ok(songs) => {
                        self.selected_result = 0;
                        self.search_results = songs;
                        self.status = format!("{} resultados", self.search_results.len());
                    }
                    Err(error) => self.status = error,
                }
                None
            }
            Event::AudioReady(result) => {
                match result {
                    Ok(_) => {
                        self.has_audio = true;
                        self.status = "Audio listo".into();
                    }
                    Err(error) => self.status = error,
                }
                None
            }
            Event::RelatedFinished(result) => {
                if let Ok(songs) = result {
                    self.queue.extend(songs);
                    self.status = "Mix listo".into();
                }
                None
            }
        }
    }

    pub fn navigate(&mut self, screen: Screen) {
        self.screen = screen;
        self.selected_item = 0;
    }

    pub fn apply_player_snapshot(&mut self, snapshot: PlayerSnapshot) {
        self.current_position = snapshot.position;
        self.player.position = snapshot.position;
        self.volume = (snapshot.volume * 100.0).round() as u8;
        self.player.volume = self.volume;
        self.player.muted = snapshot.muted;
        self.player.status = snapshot.status;
        self.player.is_playing = snapshot.status == PlaybackStatus::Playing;
        self.is_playing = self.player.is_playing;
    }

    fn selected_song(&self) -> Option<Song> {
        match self.screen {
            Screen::Search => self.search_results.get(self.selected_result).cloned(),
            Screen::History => self.history.get(self.selected_result).cloned(),
            Screen::Favorites => self.favorites.get(self.selected_result).cloned(),
            Screen::Queue => self.queue.get(self.selected_result).cloned(),
            Screen::Playlists => self.playlist_songs.get(self.selected_result).cloned(),
        }
    }

    pub fn select_song(&mut self, song: Song) {
        self.duration = song.duration.unwrap_or_default();
        self.current_position = 0;
        self.player.duration = self.duration;
        self.player.position = 0;
        self.player.status = PlaybackStatus::Loading;
        self.current_song = Some(song.clone());
        self.has_audio = false;
        self.queue_index = self
            .queue
            .iter()
            .position(|item| item.id == song.id)
            .unwrap_or(0);
        self.is_playing = false;
    }

    pub fn set_searching(&mut self) {
        self.searching = true;
        self.status = "Searching...".into();
    }

    pub fn play_next(&mut self) -> Option<Song> {
        if self.queue_index + 1 >= self.queue.len() {
            return None;
        }
        self.queue_index += 1;
        let song = self.queue[self.queue_index].clone();
        self.select_song(song.clone());
        Some(song)
    }

    pub fn play_previous(&mut self) -> Option<Song> {
        if self.queue_index == 0 {
            return None;
        }
        self.queue_index -= 1;
        let song = self.queue[self.queue_index].clone();
        self.select_song(song.clone());
        Some(song)
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    #[test]
    fn volume_is_clamped() {
        assert_eq!(App::new(150).volume, 100);
    }
}
