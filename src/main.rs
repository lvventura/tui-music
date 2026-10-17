mod ai;
mod app;
mod auth;
mod cache;
mod cli;
mod commands;
mod config;
mod database;
mod dj;
mod errors;
mod metrics;
mod player;
mod terminal;
mod ui;
mod youtube;

use std::{fs::OpenOptions, io, sync::Arc, time::{Duration, Instant}};

use app::{Action, App, Event, state::View};
use cli::Command;
use config::{Settings, State, paths};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, MouseButton,
        MouseEventKind,
    },
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
use tracing::{debug, error, info};
use youtube::provider::{MusicProvider, YtDlpProvider};

#[tokio::main]
async fn main() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match cli::parse(&args) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("error: {error}\n{}", cli::usage());
            std::process::exit(2);
        }
    };
    std::fs::create_dir_all(paths::log_dir())?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::log_file())?;
    tracing_subscriber::fmt()
        .with_env_filter("info,yt_dj=debug")
        .with_ansi(false)
        .with_writer(log_file)
        .init();
    let settings = Settings::load_effective()?;

    match command {
        Command::Help => {
            commands::print_help();
            return Ok(());
        }
        Command::Auth(mode) => return finish_cli(commands::run_auth(mode, &settings).await),
        Command::Diagnose => return finish_cli(commands::run_diagnose(&settings).await),
        Command::Run => {}
    }

    let database = Database::open(settings.database_path())?;
    let stored_cookie = auth::load_stored_cookie().and_then(|raw| auth::CookieHeader::parse(&raw).ok());
    let cookies_file = stored_cookie
        .as_ref()
        .and_then(|header| auth::export_netscape(header).ok());
    let concrete_provider = YtDlpProvider::new(&settings.yt_dlp_path, &settings)
        .with_cookies(cookies_file.clone())
        .with_browser_cookies(Some(settings.auth.browser.clone()));
    let provider_available = concrete_provider.available().await;
    let provider: Arc<dyn MusicProvider> = Arc::new(concrete_provider);
    info!("yt-dj started");
    let (events, mut incoming) = mpsc::unbounded_channel();

    let mut state = State::load(&paths::state_file());
    let (starting_volume, volume_warning) = state.starting_volume();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(starting_volume);
    let mut auth = auth::AuthManager::with_cookie(settings.auth.browser.clone(), stored_cookie);
    app.account_status = auth.status().label().into();
    if let (Some(header), true) = (auth.cookie_header().cloned(), provider_available) {
        auth.set_status(auth::AuthStatus::Verifying);
        app.account_status = auth::AuthStatus::Verifying.label().into();
        let yt_dlp = settings.yt_dlp_path.clone();
        let sender = events.clone();
        tokio::spawn(async move {
            let result = auth::validate_with_ytdlp(&yt_dlp, &header).await;
            let _ = sender.send(Event::AuthFinished(result.map_err(|error| error.to_string())));
        });
    }
    if let Some(warning) = volume_warning {
        app.status = warning.into();
    }
    let caps = terminal::TerminalCaps::detect();
    app.truecolor = caps.truecolor;
    app.image_protocol = caps.image;
    app.terminal_note = caps.note();
    let theme_name = if state.theme.is_empty() {
        settings.ui.theme.clone()
    } else {
        state.theme.clone()
    };
    app.set_theme_name(theme_name);
    app.keymap = ui::keymap::KeymapPreset::parse(&state.keymap);
    if let Some(view) = View::from_name(&state.last_view) {
        app.navigate(view);
    }
    if !provider_available {
        app.status = format!(
            "yt-dlp no encontrado o no ejecutable: {}",
            settings.yt_dlp_path
        );
    }

    app.player.seek_step = settings.seek_step;
    app.player.large_seek_step = settings.large_seek_step;
    app.player.shuffle = state.shuffle;
    app.player.muted = state.muted;
    app.player.repeat = state.repeat_mode();
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
    state.volume = app.volume;
    state.muted = app.player.muted;
    state.shuffle = app.player.shuffle;
    state.theme = app.theme_name.clone();
    state.keymap = app.keymap.label().to_string();
    state.last_view = app.view.name().to_string();
    state.set_repeat_mode(app.player.repeat);
    if let Err(error) = state.save(&paths::state_file()) {
        tracing::warn!(%error, "no se pudo guardar el estado");
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    if let Some(path) = &cookies_file {
        let _ = std::fs::remove_file(path);
    }
    result
}

async fn run<B: ratatui::backend::Backend + io::Write>(
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
    let mut last_click: Option<(Instant, u16, u16)> = None;
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
                            app.related_autoplay = true;
                            request_related(provider.clone(), song, events.clone());
                        }
                    }
                }
            }
        }
        terminal.draw(|frame| ui::draw(frame, app))?;
        ui::cover::flush(terminal, app)?;
        while let Ok(event) = incoming.try_recv() {
            match event {
                Event::AudioReady(result) => {
                    match result {
                        Ok(audio) => {
                            if let Some(audio_player) = player.as_mut() {
                                if let Err(error) = audio_player.play(audio).await {
                                    app.status = format!("Error de reproducción: {error}");
                                    app.has_audio = false;
                                    app.player.status = player::state::PlaybackStatus::Error;
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
                        Err(error) => {
                            let hint = if error.contains("DPAPI")
                                || error.contains("Sign in to confirm")
                                || error.contains("429")
                                || error.contains("cookies")
                            {
                                " · YouTube pide sesión: copia la cookie (DevTools > Network) y ejecuta `yt-dj --auth clipboard`"
                            } else {
                                ""
                            };
                            app.status = format!("No se pudo obtener el audio: {error}{hint}");
                            app.has_audio = false;
                            app.is_playing = false;
                            app.player.status = player::state::PlaybackStatus::Error;
                        }
                    }
                }
                Event::RelatedFinished(result) => {
                    let autoplay = app.related_autoplay;
                    app.related_autoplay = false;
                    match result {
                        Ok(songs) => {
                            let added = app.enqueue_related(songs);
                            if autoplay {
                                match app.play_next() {
                                    Some(song) => {
                                        app.set_cover_seed(song.id.clone());
                                        if let Some(thumbnail) = song.thumbnail.clone() {
                                            request_cover(thumbnail, events.clone());
                                        }
                                        if let Some(audio_player) = player.as_mut() {
                                            audio_player.stop();
                                        }
                                        request_audio(provider.clone(), song, events.clone());
                                    }
                                    None => {
                                        app.status = "No se encontraron recomendaciones".into()
                                    }
                                }
                            } else if added > 0 {
                                app.status =
                                    format!("DJ: {added} recomendaciones añadidas a la cola");
                            } else {
                                app.status = "DJ: sin nuevas recomendaciones".into();
                            }
                        }
                        Err(error) => app.status = error,
                    }
                }
                Event::AuthFinished(result) => {
                    let status = if result.is_ok() {
                        auth::AuthStatus::Authenticated
                    } else {
                        auth::AuthStatus::Unauthenticated
                    };
                    auth.set_status(status);
                    app.handle_event(Event::AuthFinished(result));
                }
                event => {
                    app.handle_event(event);
                }
            }
        }
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                CrosstermEvent::Key(key) => {
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
                CrosstermEvent::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                    if let Some(action) = ui::layout::hit_test(area, app, mouse) {
                        let left_click =
                            matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left));
                        let double = left_click
                            && last_click
                                .map(|(time, x, y)| {
                                    time.elapsed() < Duration::from_millis(400)
                                        && x == mouse.column
                                        && y == mouse.row
                                })
                                .unwrap_or(false);
                        let playable =
                            matches!(action, Action::SelectIndex(_) | Action::LibraryClick { .. });
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
                        if double && playable {
                            if let Some(song) = app.selected_click_song() {
                                handle_action(
                                    Action::Select(song),
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
                        last_click = if left_click {
                            Some((Instant::now(), mouse.column, mouse.row))
                        } else {
                            None
                        };
                    }
                }
                _ => {}
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
            app.set_cover_seed(song.id.clone());
            app.select_song(song.clone());
            if let Some(thumbnail) = song.thumbnail.clone() {
                request_cover(thumbnail, events.clone());
            }
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
            let dj_provider = provider.clone();
            let dj_events = events.clone();
            request_audio(provider, song, events);
            maybe_request_dj(app, dj_provider, dj_events);
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
            app.player.volume = app.volume;
        }
        Action::VolumeDown => {
            app.volume = app.volume.saturating_sub(5);
            if let Some(audio) = player.as_mut() {
                if let Err(error) = audio.set_volume(app.volume) {
                    app.status = error.to_string();
                }
            }
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
        }
        Action::Login => match auth.status() {
            auth::AuthStatus::Authenticated => {
                app.status = "Sesión ya autenticada".into();
            }
            auth::AuthStatus::Verifying => {
                app.status = "Validación en curso…".into();
            }
            auth::AuthStatus::Unauthenticated => {
                if let Some(header) = auth.cookie_header().cloned() {
                    auth.set_status(auth::AuthStatus::Verifying);
                    app.account_status = auth::AuthStatus::Verifying.label().into();
                    app.status = "Validando cookie guardada…".into();
                    let yt_dlp = yt_dlp_path.to_owned();
                    let sender = events.clone();
                    tokio::spawn(async move {
                        let result = auth::validate_with_ytdlp(&yt_dlp, &header).await;
                        let _ =
                            sender.send(Event::AuthFinished(result.map_err(|error| error.to_string())));
                    });
                } else if auth.login_pending() {
                    match auth.verify(yt_dlp_path).await {
                        Ok(()) => {
                            app.account_status = auth.status().label().into();
                            app.status = "Sesión autenticada (navegador)".into();
                        }
                        Err(error) => {
                            app.account_status = auth.status().label().into();
                            app.status = error.to_string();
                        }
                    }
                } else {
                    match auth.begin_login() {
                        Ok(()) => {
                            app.status = "Navegador abierto; vuelve a pulsar L para verificar. Alternativa: `yt-dj --auth cookie|browser`".into()
                        }
                        Err(error) => {
                            app.status = format!(
                                "{error}. Usa `yt-dj --auth cookie` o `--auth browser`"
                            )
                        }
                    }
                }
            }
        },
        Action::ToggleShuffle => {
            app.player.shuffle = !app.player.shuffle;
            queue.set_shuffle(app.player.shuffle);
        }
        Action::CycleRepeat => {
            app.player.repeat = app.player.repeat.cycle();
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
        Action::SwitchView(view) => {
            app.navigate(view);
            let loaded = if view == View::Library {
                database::repositories::history::list(database.connection(), 100)
                    .map(|songs| app.history = songs)
                    .and_then(|()| {
                        database::repositories::favorites::list(database.connection())
                            .map(|songs| app.favorites = songs)
                    })
                    .and_then(|()| {
                        database::repositories::playlists::list(database.connection())
                            .map(|items| app.playlists = items)
                    })
            } else {
                Ok(())
            };
            if let Err(error) = loaded {
                app.status = error.to_string();
            }
        }
        Action::SetVolume(volume) => {
            app.volume = volume.min(100);
            if let Some(audio) = player.as_mut() {
                if let Err(error) = audio.set_volume(app.volume) {
                    app.status = error.to_string();
                }
            }
            app.player.volume = app.volume;
        }
        Action::SeekTo(position) => {
            app.player.position = position;
            app.current_position = position;
        }
        Action::SelectIndex(index) => {
            app.selected_result = index;
        }
        Action::LibraryClick { column, index } => {
            app.apply_library_click(column, index);
            if column == 0 {
                let _ = database.load_app_data(app);
            } else if column == 1 && app.screen == app::state::Screen::Playlists {
                if let Some((id, _)) = app.playlists.get(index).cloned() {
                    match database::repositories::playlists::songs(database.connection(), &id) {
                        Ok(songs) => {
                            app.selected_playlist = Some(id);
                            app.playlist_songs = songs;
                            app.library_indices[2] = 0;
                        }
                        Err(error) => app.status = error.to_string(),
                    }
                }
            }
        }
        Action::ScrollSelection(direction) => app.scroll_selection(direction),
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
            app.set_cover_seed(song.id.clone());
            app.select_song(song.clone());
            if let Some(thumbnail) = song.thumbnail.clone() {
                request_cover(thumbnail, events.clone());
            }
            queue.play_now(song.clone());
            let dj_provider = provider.clone();
            let dj_events = events.clone();
            request_audio(provider, song, events);
            maybe_request_dj(app, dj_provider, dj_events);
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
        info!(song = %song.title, id = %song.id, "resolviendo audio");
        let result = provider
            .stream_url(song)
            .await
            .map_err(|error| {
                error!(%error, "no se pudo resolver el audio");
                error.to_string()
            });
        let _ = events.send(Event::AudioReady(result));
    });
}

fn request_cover(url: String, events: mpsc::UnboundedSender<Event>) {
    tokio::spawn(async move {
        let result = ui::cover::fetch(&url)
            .await
            .map_err(|error| error.to_string());
        let _ = events.send(Event::CoverReady(result));
    });
}

/// DJ auto-queue: keep a few recommendations ready so playback can continue
/// without the user hunting for the next track. Only fires when the queue is
/// short, which also limits how often we hit YouTube.
fn maybe_request_dj(
    app: &mut App,
    provider: Arc<dyn MusicProvider>,
    events: mpsc::UnboundedSender<Event>,
) {
    const MIN_QUEUE: usize = 5;
    if app.searching || app.queue.len() >= MIN_QUEUE {
        return;
    }
    let Some(song) = app.current_song.clone() else {
        return;
    };
    info!(song = %song.title, "DJ: pidiendo recomendaciones");
    app.related_autoplay = false;
    app.status = "DJ: buscando recomendaciones…".into();
    request_related(provider, song, events);
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

fn finish_cli(result: AppResult<()>) -> AppResult<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
