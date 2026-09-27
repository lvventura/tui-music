mod ai;
mod app;
mod auth;
mod cache;
mod config;
mod database;
mod dj;
mod errors;
mod metrics;
mod player;
mod ui;
mod youtube;

use std::{fs::OpenOptions, io, sync::Arc, time::Duration};

use app::{Action, App, Event};
use config::Settings;
use crossterm::{
    event::{self, Event as CrosstermEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use database::Database;
use errors::AppResult;
use player::{
    audio::{AudioPlayer, RodioPlayer},
    queue::PlaybackQueue,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;
use tracing::{debug, info};
use youtube::provider::{MusicProvider, YtDlpProvider};

#[tokio::main]
async fn main() -> AppResult<()> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/yt-dj.log")?;
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(log_file)
        .init();
    let settings = Settings::load("config/default.toml")?;
    let database = Database::open(settings.database_path())?;
    let concrete_provider = YtDlpProvider::new(&settings.yt_dlp_path, &settings);
    let provider_available = concrete_provider.available().await;
    let provider: Arc<dyn MusicProvider> = Arc::new(concrete_provider);
    info!("yt-dj started");
    let (events, mut incoming) = mpsc::unbounded_channel();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(settings.initial_volume(&database));
    let mut auth = auth::AuthManager::new(settings.auth.browser.clone());
    app.account_status = format!("{:?}", auth.status());
    if !provider_available {
        app.status = format!(
            "yt-dlp no encontrado o no ejecutable: {}",
            settings.yt_dlp_path
        );
    }

    app.player.seek_step = settings.seek_step;
    app.player.large_seek_step = settings.large_seek_step;
    app.player.shuffle = database::repositories::preferences::get(database.connection(), "shuffle")
        .ok()
        .flatten()
        .as_deref()
        == Some("true");
    app.player.muted = database::repositories::preferences::get(database.connection(), "muted")
        .ok()
        .flatten()
        .as_deref()
        == Some("true");
    app.player.repeat =
        match database::repositories::preferences::get(database.connection(), "repeat")
            .ok()
            .flatten()
            .as_deref()
        {
            Some("Queue") => player::state::RepeatMode::Queue,
            Some("Song") => player::state::RepeatMode::Song,
            _ => player::state::RepeatMode::Off,
        };
    database.load_app_data(&mut app)?;
    let mut player: Option<RodioPlayer> = None;
    let mut queue = PlaybackQueue::default();
    let result = run(
        &mut terminal,
        &mut app,
        provider,
        events.clone(),
        &mut incoming,
        &mut player,
        &mut queue,
        &database,
        &mut auth,
        &settings.yt_dlp_path,
    )
    .await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    provider: Arc<dyn MusicProvider>,
    events: mpsc::UnboundedSender<Event>,
    incoming: &mut mpsc::UnboundedReceiver<Event>,
    player: &mut Option<RodioPlayer>,
    queue: &mut PlaybackQueue,
    database: &Database,
    auth: &mut auth::AuthManager,
    yt_dlp_path: &str,
) -> AppResult<()> {
    loop {
        if app.has_audio {
            let snapshot = player.as_ref().map(RodioPlayer::snapshot);
            if let Some(snapshot) = snapshot {
                let was_playing = app.is_playing;
                app.apply_player_snapshot(snapshot);
                if was_playing && snapshot.status == player::state::PlaybackStatus::Finished {
                    if let Some(song) = app.play_next() {
                        if let Some(audio_player) = player.as_mut() {
                            audio_player.stop();
                        }
                        request_audio(provider.clone(), song, events.clone());
                    } else {
                        app.has_audio = false;
                        app.status = "Cargando siguiente canción…".into();
                        if let Some(song) = app.current_song.clone() {
                            request_related(provider.clone(), song, events.clone());
                        }
                    }
                }
            }
        }
        terminal.draw(|frame| ui::draw(frame, app))?;
        while let Ok(event) = incoming.try_recv() {
            match event {
                Event::AudioReady(result) => {
                    if let Ok(audio) = result {
                        if let Some(audio_player) = player.as_mut() {
                            if let Err(error) = audio_player.play(audio).await {
                                app.status = error.to_string();
                                app.has_audio = false;
                            } else {
                                app.has_audio = true;
                                app.is_playing = true;
                            }
                        }
                        if let Some(song) = &app.current_song {
                            let _ = database::repositories::history::record(
                                database.connection(),
                                &song.id,
                                app.current_position,
                                app.current_position >= app.duration,
                            );
                            if let Ok(items) =
                                database::repositories::history::list(database.connection(), 100)
                            {
                                app.history = items;
                            }
                        }
                    }
                }
                Event::RelatedFinished(result) => {
                    if let Ok(songs) = result {
                        if let Some(song) = songs.first().cloned() {
                            app.queue.extend(songs);
                            app.select_song(song.clone());
                            if let Some(audio_player) = player.as_mut() {
                                audio_player.stop();
                            }
                            request_audio(provider.clone(), song, events.clone());
                        } else {
                            app.status = "No se encontraron recomendaciones".into();
                        }
                    } else if let Err(error) = result {
                        app.status = error;
                    }
                }
                event => {
                    app.handle_event(event);
                }
            }
        }
        if event::poll(Duration::from_millis(50))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if let Some(action) = app.handle_event(Event::Key(key)) {
                    handle_action(
                        action,
                        app,
                        provider.clone(),
                        events.clone(),
                        player,
                        queue,
                        database,
                        auth,
                        yt_dlp_path,
                    )
                    .await;
                }
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

async fn handle_action(
    action: Action,
    app: &mut App,
    provider: Arc<dyn MusicProvider>,
    events: mpsc::UnboundedSender<Event>,
    player: &mut Option<RodioPlayer>,
    queue: &mut PlaybackQueue,
    database: &Database,
    auth: &mut auth::AuthManager,
    yt_dlp_path: &str,
) {
    match action {
        Action::Search(query) => {
            app.set_searching();
            tokio::spawn(async move {
                let result = provider
                    .search(query)
                    .await
                    .map_err(|error| error.to_string());
                let _ = events.send(Event::SearchFinished(result));
            });
        }
        Action::Select(song) => {
            if app.screen == app::state::Screen::Search {
                app.queue.clear();
                app.queue_index = 0;
                app.queue.push(song.clone());
            }
            app.select_song(song.clone());
            if let Err(error) = database::repositories::songs::save(database.connection(), &song) {
                app.status = error.to_string();
            }
            app.is_favorite =
                database::repositories::favorites::is_favorite(database.connection(), &song.id)
                    .unwrap_or(false);
            queue.add(song.clone());
            if player.is_none() {
                match RodioPlayer::new() {
                    Ok(mut audio) => {
                        let _ = audio.set_volume(app.volume);
                        audio.set_muted(app.player.muted);
                        *player = Some(audio);
                    }
                    Err(error) => app.status = error.to_string(),
                }
            }
            request_audio(provider, song, events);
        }
        Action::PlayPause => {
            debug!(
                app_playing = app.is_playing,
                has_audio = app.has_audio,
                "[INPUT] Space / PlayPause"
            );
            if let Some(audio) = player.as_mut() {
                app.is_playing = audio.toggle_pause();
                debug!(
                    app_playing = app.is_playing,
                    "[PLAYER] pause toggle completed"
                );
            }
        }
        Action::VolumeUp => {
            app.volume = app.volume.saturating_add(5).min(100);
            if let Some(audio) = player.as_mut() {
                if let Err(error) = audio.set_volume(app.volume) {
                    app.status = error.to_string();
                }
            }
            let _ = database::repositories::preferences::set(
                database.connection(),
                "volume",
                &app.volume.to_string(),
            );
            app.player.volume = app.volume;
        }
        Action::VolumeDown => {
            app.volume = app.volume.saturating_sub(5);
            if let Some(audio) = player.as_mut() {
                if let Err(error) = audio.set_volume(app.volume) {
                    app.status = error.to_string();
                }
            }
            let _ = database::repositories::preferences::set(
                database.connection(),
                "volume",
                &app.volume.to_string(),
            );
            app.player.volume = app.volume;
        }
        Action::SeekForward(large) => {
            let step = if large {
                app.player.large_seek_step
            } else {
                app.player.seek_step
            };
            app.player.seek_by(step as i64);
            app.current_position = app.player.position;
        }
        Action::SeekBackward(large) => {
            let step = if large {
                app.player.large_seek_step
            } else {
                app.player.seek_step
            };
            app.player.seek_by(-(step as i64));
            app.current_position = app.player.position;
        }
        Action::ToggleMute => {
            app.player.toggle_mute();
            if let Some(audio) = player.as_mut() {
                audio.set_muted(app.player.muted);
            }
            let _ = database::repositories::preferences::set(
                database.connection(),
                "muted",
                &app.player.muted.to_string(),
            );
        }
        Action::Login => {
            if auth.status() == auth::AuthStatus::Unauthenticated {
                if auth.login_pending() {
                    if let Err(error) = auth.verify(yt_dlp_path).await {
                        app.status = error.to_string();
                    } else {
                        app.account_status = "Authenticated".into();
                        app.status = "Sesión autenticada".into();
                    }
                } else {
                    match auth.begin_login() {
                        Ok(()) => {
                            app.status =
                                "Navegador abierto; vuelve a pulsar L para verificar la sesión"
                                    .into()
                        }
                        Err(error) => app.status = error.to_string(),
                    }
                }
            } else {
                app.status = "Sesión ya autenticada".into();
            }
        }
        Action::ToggleShuffle => {
            app.player.shuffle = !app.player.shuffle;
            queue.set_shuffle(app.player.shuffle);
            let _ = database::repositories::preferences::set(
                database.connection(),
                "shuffle",
                &app.player.shuffle.to_string(),
            );
        }
        Action::CycleRepeat => {
            app.player.repeat = app.player.repeat.cycle();
            let _ = database::repositories::preferences::set(
                database.connection(),
                "repeat",
                &format!("{:?}", app.player.repeat),
            );
        }
        Action::Next => {
            if let Some(song) = app.play_next() {
                if let Some(audio) = player.as_mut() {
                    audio.stop();
                }
                request_audio(provider, song, events);
            } else {
                app.status = "Cargando siguiente canción…".into();
                if let Some(song) = app.current_song.clone() {
                    request_related(provider, song, events);
                }
            }
        }
        Action::Previous => {
            if let Some(song) = app.play_previous() {
                if let Some(audio) = player.as_mut() {
                    audio.stop();
                }
                request_audio(provider, song, events);
            } else {
                app.status = "No hay canción anterior en la cola".into();
            }
        }
        Action::ToggleFavorite => {
            if let Some(song) = &app.current_song {
                let result = if app.is_favorite {
                    database::repositories::favorites::remove(database.connection(), &song.id)
                } else {
                    database::repositories::favorites::add(database.connection(), &song.id)
                };
                if let Err(error) = result {
                    app.status = error.to_string();
                } else {
                    app.is_favorite = !app.is_favorite;
                    app.favorites = database::repositories::favorites::list(database.connection())
                        .unwrap_or_default();
                }
            }
        }
        Action::Navigate(screen) => {
            app.navigate(screen);
            let loaded = match screen {
                app::state::Screen::History => {
                    database::repositories::history::list(database.connection(), 100)
                        .map(|songs| app.history = songs)
                }
                app::state::Screen::Favorites => {
                    database::repositories::favorites::list(database.connection())
                        .map(|songs| app.favorites = songs)
                }
                app::state::Screen::Playlists => {
                    database::repositories::playlists::list(database.connection())
                        .map(|items| app.playlists = items)
                }
                _ => Ok(()),
            };
            if let Err(error) = loaded {
                app.status = error.to_string();
            }
        }
        Action::CreatePlaylist => {
            let id = format!("playlist-{}", app.playlists.len() + 1);
            if let Err(error) = database::repositories::playlists::create(
                database.connection(),
                &id,
                &format!("Playlist {}", app.playlists.len() + 1),
            ) {
                app.status = error.to_string();
            } else if let Ok(items) = database::repositories::playlists::list(database.connection())
            {
                app.playlists = items;
            }
        }
        Action::AddToPlaylist => {
            let playlist_id = app
                .selected_playlist
                .as_deref()
                .or_else(|| app.playlists.first().map(|(id, _)| id.as_str()));
            if let (Some(song), Some(playlist_id)) = (&app.current_song, playlist_id) {
                if let Err(error) = database::repositories::playlists::add_song(
                    database.connection(),
                    playlist_id,
                    &song.id,
                ) {
                    app.status = error.to_string();
                }
            }
        }
        Action::DeleteSelected => {
            let song = match app.screen {
                app::state::Screen::History => app.history.get(app.selected_result),
                app::state::Screen::Favorites => app.favorites.get(app.selected_result),
                app::state::Screen::Queue => app.queue.get(app.selected_result),
                app::state::Screen::Playlists => app.playlist_songs.get(app.selected_result),
                app::state::Screen::Search => None,
            }
            .cloned();
            if let Some(song) = song {
                let result = match app.screen {
                    app::state::Screen::History => database::repositories::history::remove_song(
                        database.connection(),
                        &song.id,
                    ),
                    app::state::Screen::Favorites => {
                        database::repositories::favorites::remove(database.connection(), &song.id)
                    }
                    app::state::Screen::Playlists => app
                        .selected_playlist
                        .as_deref()
                        .map(|id| {
                            database::repositories::playlists::remove_song(
                                database.connection(),
                                id,
                                &song.id,
                            )
                        })
                        .unwrap_or(Ok(())),
                    _ => Ok(()),
                };
                if let Err(error) = result {
                    app.status = error.to_string();
                } else {
                    let _ = database.load_app_data(app);
                    if app.screen == app::state::Screen::Playlists {
                        if let Some(id) = &app.selected_playlist {
                            app.playlist_songs =
                                database::repositories::playlists::songs(database.connection(), id)
                                    .unwrap_or_default();
                        }
                    }
                }
            }
        }
        Action::OpenPlaylist(id) => {
            match database::repositories::playlists::songs(database.connection(), &id) {
                Ok(songs) => {
                    app.selected_playlist = Some(id);
                    app.playlist_songs = songs;
                    app.selected_result = 0;
                }
                Err(error) => app.status = error.to_string(),
            }
        }
        Action::MoveQueueItem { from, to } => queue.move_item(from, to),
        Action::PlayNext(song) => queue.add_next(song),
        Action::PlayNow(song) => {
            app.select_song(song.clone());
            queue.play_now(song.clone());
            request_audio(provider, song, events);
        }
        Action::Quit => {}
    }
}

fn request_audio(
    provider: Arc<dyn MusicProvider>,
    song: youtube::models::Song,
    events: mpsc::UnboundedSender<Event>,
) {
    tokio::spawn(async move {
        let result = provider
            .stream_url(song)
            .await
            .map_err(|error| error.to_string());
        let _ = events.send(Event::AudioReady(result));
    });
}

fn request_related(
    provider: Arc<dyn MusicProvider>,
    song: youtube::models::Song,
    events: mpsc::UnboundedSender<Event>,
) {
    tokio::spawn(async move {
        let result = provider
            .related(song)
            .await
            .map_err(|error| error.to_string());
        let _ = events.send(Event::RelatedFinished(result));
    });
}
