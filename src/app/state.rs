use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    app::{Action, Event},
    config::paths,
    player::{
        audio::PlayerSnapshot,
        state::{PlaybackStatus, PlayerState},
    },
    terminal::ImageProtocol,
    ui::{
        cover::CoverImage,
        keymap::KeymapPreset,
        theme::Theme,
    },
    youtube::models::Song,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Search,
    History,
    Favorites,
    Playlists,
    Queue,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Queue,
    Library,
    NowPlaying,
    Search,
}

impl View {
    pub const ALL: [View; 4] = [View::Queue, View::Library, View::NowPlaying, View::Search];

    pub fn title(self) -> &'static str {
        match self {
            View::Queue => "Queue",
            View::Library => "Library",
            View::NowPlaying => "Track",
            View::Search => "Search",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            View::Queue => "queue",
            View::Library => "library",
            View::NowPlaying => "now_playing",
            View::Search => "search",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "queue" => Some(View::Queue),
            "library" => Some(View::Library),
            "now_playing" => Some(View::NowPlaying),
            "search" => Some(View::Search),
            _ => None,
        }
    }
}

/// Order of the library columns: categories, then items, then playlist songs.
const LIBRARY_CATEGORIES: [Screen; 3] = [Screen::History, Screen::Favorites, Screen::Playlists];

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
    pub view: View,
    pub search_mode: bool,
    pub searching: bool,
    pub status: String,
    pub has_audio: bool,
    pub should_quit: bool,
    pub favorites: Vec<Song>,
    pub history: Vec<Song>,
    pub playlists: Vec<(String, String)>,
    pub is_favorite: bool,
    pub selected_playlist: Option<String>,
    pub playlist_songs: Vec<Song>,
    pub player: PlayerState,
    pub debug_visible: bool,
    pub account_status: String,
    pub terminal_note: Option<String>,
    pub truecolor: bool,
    pub image_protocol: ImageProtocol,
    pub cover_epoch: u64,
    pub cover_drawn: Option<(u64, ratatui::layout::Rect)>,
    pub theme: Theme,
    pub theme_name: String,
    pub theme_seed: String,
    pub available_themes: Vec<String>,
    pub keymap: KeymapPreset,
    pub show_menu: bool,
    pub menu_option: usize,
    pub library_focus: usize,
    pub library_indices: [usize; 3],
    pub cover: Option<CoverImage>,
    /// True when the pending related-track request must start playback
    /// (the queue ran dry) instead of only appending DJ recommendations.
    pub related_autoplay: bool,
}

impl App {
    pub fn new(volume: u8) -> Self {
        let mut available_themes: Vec<String> = Theme::names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        if let Ok(entries) = std::fs::read_dir(paths::themes_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if !available_themes.iter().any(|name| name == stem) {
                    available_themes.push(stem.to_owned());
                }
            }
        }
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
            view: View::Search,
            search_mode: false,
            searching: false,
            status: "Pulsa / para buscar".into(),
            has_audio: false,
            should_quit: false,
            favorites: Vec::new(),
            history: Vec::new(),
            playlists: Vec::new(),
            is_favorite: false,
            selected_playlist: None,
            playlist_songs: Vec::new(),
            player: PlayerState::new(volume, 5, 15),
            debug_visible: false,
            account_status: "Unauthenticated".into(),
            terminal_note: None,
            truecolor: true,
            image_protocol: ImageProtocol::HalfBlock,
            cover_epoch: 0,
            cover_drawn: None,
            theme: Theme::default(),
            theme_name: crate::ui::theme::DEFAULT_THEME.to_owned(),
            theme_seed: "yt-dj".into(),
            available_themes,
            keymap: KeymapPreset::Default,
            show_menu: false,
            menu_option: 0,
            library_focus: 0,
            library_indices: [0, 0, 0],
            cover: None,
            related_autoplay: false,
        }
    }

    pub fn apply_theme(&mut self) {
        let mut theme = Theme::resolve(&self.theme_name, &self.theme_seed, &paths::themes_dir());
        if !self.truecolor {
            theme.degrade_to_256();
        }
        self.theme = theme;
    }

    pub fn set_theme_name(&mut self, name: String) {
        self.theme_name = name;
        self.apply_theme();
    }

    /// Called when the current song (and thus the cover seed) changes.
    pub fn set_cover_seed(&mut self, seed: String) {
        self.theme_seed = seed;
        self.cover = None;
        self.cover_epoch = self.cover_epoch.wrapping_add(1);
        if self.theme_name == crate::ui::theme::DEFAULT_THEME {
            self.apply_theme();
        }
    }

    pub fn handle_event(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Press => None,
            Event::Key(key) => self.handle_key(key),
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
            Event::AuthFinished(result) => {
                match result {
                    Ok(()) => {
                        self.account_status = "Authenticated".into();
                        self.status = "Sesión autenticada".into();
                    }
                    Err(error) => {
                        self.account_status = "Unauthenticated".into();
                        self.status = error;
                    }
                }
                None
            }
            Event::CoverReady(result) => {
                self.cover_epoch = self.cover_epoch.wrapping_add(1);
                match result {
                    Ok(cover) => {
                        if self.theme_name == crate::ui::theme::DEFAULT_THEME {
                            self.theme = Theme::from_cover(cover.average_color());
                        }
                        self.cover = Some(cover);
                    }
                    Err(_) => self.cover = None,
                }
                None
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.search_mode {
            return self.handle_search_key(key);
        }
        if self.show_menu {
            return self.handle_menu_key(key);
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
            KeyCode::Char('1') => Some(Action::SwitchView(View::Queue)),
            KeyCode::Char('2') => Some(Action::SwitchView(View::Library)),
            KeyCode::Char('3') => Some(Action::SwitchView(View::NowPlaying)),
            KeyCode::Char('4') => Some(Action::SwitchView(View::Search)),
            KeyCode::Char('f') => Some(Action::ToggleFavorite),
            KeyCode::Char('d') => Some(Action::DeleteSelected),
            KeyCode::Char('c') => Some(Action::CreatePlaylist),
            KeyCode::Char('a') => Some(Action::AddToPlaylist),
            KeyCode::Char('m') => Some(Action::ToggleMute),
            KeyCode::Char('l') | KeyCode::Char('L') => Some(Action::Login),
            KeyCode::Char('D') => {
                self.debug_visible = !self.debug_visible;
                None
            }
            KeyCode::Char('r') => Some(Action::CycleRepeat),
            KeyCode::Char('s') => Some(Action::ToggleShuffle),
            KeyCode::Left => self.left_right(key, -1),
            KeyCode::Right => self.left_right(key, 1),
            KeyCode::Char(' ') => {
                // With nothing loaded, Space means "play what is selected";
                // otherwise it toggles pause/resume.
                if self.current_song.is_none() {
                    self.enter()
                } else {
                    Some(Action::PlayPause)
                }
            }
            KeyCode::Char('n') => Some(Action::Next),
            KeyCode::Char('p') => Some(Action::Previous),
            KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::VolumeUp),
            KeyCode::Char('-') => Some(Action::VolumeDown),
            KeyCode::Esc => {
                self.show_menu = true;
                self.menu_option = 0;
                None
            }
            KeyCode::Char('j') if self.keymap == KeymapPreset::Vim => self.move_selection(1),
            KeyCode::Char('k') if self.keymap == KeymapPreset::Vim => self.move_selection(-1),
            KeyCode::Char('h') if self.keymap == KeymapPreset::Vim => {
                Some(Action::SeekBackward(false))
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Enter => self.enter(),
            _ => None,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
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
        }
    }

    fn handle_menu_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.show_menu = false;
                None
            }
            KeyCode::Up => {
                self.menu_option = self.menu_option.saturating_sub(1);
                None
            }
            KeyCode::Down => {
                self.menu_option = (self.menu_option + 1).min(MENU_LEN - 1);
                None
            }
            KeyCode::Left => self.adjust_menu(-1),
            KeyCode::Right => self.adjust_menu(1),
            _ => None,
        }
    }

    fn adjust_menu(&mut self, delta: i32) -> Option<Action> {
        match self.menu_option {
            MENU_THEME => {
                self.cycle_theme(delta);
                None
            }
            MENU_VOLUME => {
                let volume = (self.volume as i32 + delta * 5).clamp(0, 100) as u8;
                self.volume = volume;
                self.player.volume = volume;
                Some(Action::SetVolume(volume))
            }
            MENU_SHUFFLE => Some(Action::ToggleShuffle),
            MENU_REPEAT => {
                let _ = delta;
                Some(Action::CycleRepeat)
            }
            MENU_KEYMAP => {
                self.keymap = match self.keymap {
                    KeymapPreset::Default => KeymapPreset::Vim,
                    KeymapPreset::Vim => KeymapPreset::Default,
                };
                None
            }
            MENU_QUIT => {
                self.show_menu = false;
                self.should_quit = true;
                Some(Action::Quit)
            }
            _ => None,
        }
    }

    fn cycle_theme(&mut self, delta: i32) {
        if self.available_themes.is_empty() {
            return;
        }
        let current = self
            .available_themes
            .iter()
            .position(|name| name == &self.theme_name)
            .unwrap_or(0) as i32;
        let count = self.available_themes.len() as i32;
        let next = (current + delta).rem_euclid(count) as usize;
        let name = self.available_themes[next].clone();
        self.set_theme_name(name);
    }

    fn left_right(&mut self, key: KeyEvent, direction: i32) -> Option<Action> {
        if self.view == View::Library {
            self.move_library_column(direction);
            return None;
        }
        let large = key.modifiers.contains(KeyModifiers::SHIFT);
        if direction < 0 {
            Some(Action::SeekBackward(large))
        } else {
            Some(Action::SeekForward(large))
        }
    }

    fn move_selection(&mut self, direction: i32) -> Option<Action> {
        if self.view == View::Library {
            self.move_library_item(direction);
            return None;
        }
        let length = self.current_list_len();
        if length == 0 {
            return None;
        }
        if direction < 0 {
            self.selected_result = self.selected_result.saturating_sub(1);
        } else {
            self.selected_result = (self.selected_result + 1).min(length - 1);
        }
        None
    }

    fn enter(&mut self) -> Option<Action> {
        if self.view == View::Library {
            return self.enter_library();
        }
        if self.screen == Screen::Playlists && self.selected_playlist.is_none() {
            return self
                .playlists
                .get(self.selected_result)
                .map(|(id, _)| Action::OpenPlaylist(id.clone()));
        }
        self.selected_song().map(Action::Select)
    }

    fn enter_library(&mut self) -> Option<Action> {
        match self.library_focus {
            0 => {
                self.library_focus = 1;
                self.library_indices[1] = 0;
                self.screen = LIBRARY_CATEGORIES[self.library_indices[0]];
                None
            }
            1 => {
                if self.screen == Screen::Playlists {
                    self.library_focus = 2;
                    self.library_indices[2] = 0;
                    let id = self
                        .playlists
                        .get(self.library_indices[1])
                        .map(|(id, _)| id.clone());
                    return id.map(Action::OpenPlaylist);
                }
                self.selected_result = self.library_indices[1];
                self.library_item(self.library_indices[1]).map(Action::Select)
            }
            _ => {
                self.selected_result = self.library_indices[2];
                let song = self.playlist_songs.get(self.library_indices[2]).cloned();
                song.map(Action::Select)
            }
        }
    }

    fn move_library_column(&mut self, direction: i32) {
        if direction < 0 {
            self.library_focus = self.library_focus.saturating_sub(1);
        } else {
            self.library_focus = (self.library_focus + 1).min(2);
        }
    }

    fn move_library_item(&mut self, direction: i32) {
        let length = self.library_column_len(self.library_focus);
        if length == 0 {
            return;
        }
        let index = &mut self.library_indices[self.library_focus];
        if direction < 0 {
            *index = index.saturating_sub(1);
        } else {
            *index = (*index + 1).min(length - 1);
        }
        if self.library_focus == 0 {
            self.screen = LIBRARY_CATEGORIES[*index];
            self.library_indices[1] = 0;
            self.library_indices[2] = 0;
        }
    }

    pub fn library_category(&self, index: usize) -> Screen {
        LIBRARY_CATEGORIES.get(index).copied().unwrap_or(Screen::History)
    }

    pub fn library_column_len(&self, column: usize) -> usize {
        match column {
            0 => LIBRARY_CATEGORIES.len(),
            1 => match self.screen {
                Screen::History => self.history.len(),
                Screen::Favorites => self.favorites.len(),
                Screen::Playlists => self.playlists.len(),
                _ => 0,
            },
            2 => self.playlist_songs.len(),
            _ => 0,
        }
    }

    pub fn library_item(&self, index: usize) -> Option<Song> {
        match self.screen {
            Screen::History => self.history.get(index).cloned(),
            Screen::Favorites => self.favorites.get(index).cloned(),
            _ => None,
        }
    }

    pub fn current_list_len(&self) -> usize {
        match self.screen {
            Screen::Search => self.search_results.len(),
            Screen::History => self.history.len(),
            Screen::Favorites => self.favorites.len(),
            Screen::Queue => self.queue.len(),
            Screen::Playlists => self.playlists.len(),
        }
    }

    pub fn navigate(&mut self, view: View) {
        self.view = view;
        self.selected_result = 0;
        self.screen = match view {
            View::Queue => Screen::Queue,
            View::Search => Screen::Search,
            View::Library => LIBRARY_CATEGORIES[self.library_indices[0]],
            View::NowPlaying => self.screen,
        };
        if view == View::Library {
            self.library_focus = 0;
        }
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

    /// Song under the library cursor, used for double-click playback.
    pub fn selected_click_song(&self) -> Option<Song> {
        if self.view != View::Library {
            return self.selected_song();
        }
        match self.library_focus {
            1 => match self.screen {
                Screen::History => self.history.get(self.library_indices[1]).cloned(),
                Screen::Favorites => self.favorites.get(self.library_indices[1]).cloned(),
                _ => None,
            },
            2 => self.playlist_songs.get(self.library_indices[2]).cloned(),
            _ => None,
        }
    }

    pub fn scroll_selection(&mut self, direction: i32) {
        let _ = self.move_selection(direction);
    }

    pub fn apply_library_click(&mut self, column: usize, index: usize) {
        self.library_focus = column.min(2);
        self.library_indices[column.min(2)] = index;
        if column == 0 {
            self.screen = LIBRARY_CATEGORIES[index.min(LIBRARY_CATEGORIES.len() - 1)];
            self.library_indices[1] = 0;
            self.library_indices[2] = 0;
        } else {
            self.selected_result = index;
        }
    }

    pub fn select_song(&mut self, song: Song) {
        self.duration = song.duration.unwrap_or_default();
        self.current_position = 0;
        self.player.duration = self.duration;
        self.player.position = 0;
        self.player.status = PlaybackStatus::Loading;
        self.set_cover_seed(song.id.clone());
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

    /// DJ auto-queue: append recommendations that are not already queued.
    /// Returns how many tracks were actually added.
    pub fn enqueue_related(&mut self, songs: Vec<Song>) -> usize {
        const MAX_QUEUE: usize = 50;
        let mut added = 0;
        for song in songs {
            if self.queue.len() >= MAX_QUEUE {
                break;
            }
            if self.queue.iter().any(|item| item.id == song.id) {
                continue;
            }
            self.queue.push(song);
            added += 1;
        }
        added
    }
}

pub const MENU_THEME: usize = 0;
pub const MENU_VOLUME: usize = 1;
pub const MENU_SHUFFLE: usize = 2;
pub const MENU_REPEAT: usize = 3;
pub const MENU_KEYMAP: usize = 4;
pub const MENU_QUIT: usize = 5;
pub const MENU_LEN: usize = 6;

pub fn menu_labels() -> [&'static str; MENU_LEN] {
    ["Theme", "Volume", "Shuffle", "Repeat", "Keymap", "Quit"]
}

#[cfg(test)]
mod tests {
    use super::{App, View};
    use crate::app::Action;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(kind: KeyEventKind, code: KeyCode) -> crate::app::Event {
        crate::app::Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn volume_is_clamped() {
        assert_eq!(App::new(150).volume, 100);
    }

    #[test]
    fn only_press_events_are_handled() {
        let mut app = App::new(70);
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            assert!(app.handle_event(key(kind, KeyCode::Char('n'))).is_none());
        }
        assert!(matches!(
            app.handle_event(key(KeyEventKind::Press, KeyCode::Char('n'))),
            Some(Action::Next)
        ));
    }

    #[test]
    fn number_keys_switch_views() {
        let mut app = App::new(70);
        assert!(matches!(
            app.handle_event(key(KeyEventKind::Press, KeyCode::Char('1'))),
            Some(Action::SwitchView(View::Queue))
        ));
        assert!(matches!(
            app.handle_event(key(KeyEventKind::Press, KeyCode::Char('3'))),
            Some(Action::SwitchView(View::NowPlaying))
        ));
        assert_eq!(app.view, View::Search, "view changes only via the action");
    }

    #[test]
    fn escape_opens_the_menu_and_escape_closes_it() {
        let mut app = App::new(70);
        app.handle_event(key(KeyEventKind::Press, KeyCode::Esc));
        assert!(app.show_menu);
        app.handle_event(key(KeyEventKind::Press, KeyCode::Esc));
        assert!(!app.show_menu);
    }
}
