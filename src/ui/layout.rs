use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, Screen};
use crate::player::state::{PlaybackStatus, RepeatMode};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new(format!("YT-DJ  ○ {}", app.account_status))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL)),
        root[0],
    );

    let content = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(1)])
        .split(root[1]);
    let sidebar = vec![
        ListItem::new("YT-DJ"),
        ListItem::new(""),
        ListItem::new("1  Search"),
        ListItem::new("2  History"),
        ListItem::new("3  Favorites"),
        ListItem::new("4  Playlists"),
        ListItem::new("5  Queue"),
    ];
    frame.render_widget(
        List::new(sidebar).block(Block::default().title("Library").borders(Borders::ALL)),
        content[0],
    );

    let title = match app.screen {
        Screen::Search => format!("🔎 Search: {}", app.search_query),
        Screen::History => "History".into(),
        Screen::Favorites => "Favorites".into(),
        Screen::Playlists => "Playlists".into(),
        Screen::Queue => "Queue".into(),
    };
    let items: Vec<(String, bool)> = match app.screen {
        Screen::Search => app
            .search_results
            .iter()
            .map(|s| (format!("{} - {}", s.artist, s.title), false))
            .collect(),
        Screen::History => app
            .history
            .iter()
            .map(|s| (format!("{} - {}", s.artist, s.title), false))
            .collect(),
        Screen::Favorites => app
            .favorites
            .iter()
            .map(|s| (format!("{} - {}", s.artist, s.title), true))
            .collect(),
        Screen::Queue => app
            .queue
            .iter()
            .map(|s| (format!("{} - {}", s.artist, s.title), false))
            .collect(),
        Screen::Playlists if app.selected_playlist.is_some() => app
            .playlist_songs
            .iter()
            .map(|s| (format!("{} - {}", s.artist, s.title), false))
            .collect(),
        Screen::Playlists => app
            .playlists
            .iter()
            .map(|(_, name)| (name.clone(), false))
            .collect(),
    };
    let mut results = vec![
        ListItem::new(Line::from(title)),
        ListItem::new(Line::from("")),
        ListItem::new(Span::styled(
            app.status.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    results.extend(items.iter().enumerate().map(|(index, (label, favorite))| {
        let marker = if index == app.selected_result {
            "> "
        } else {
            "  "
        };
        let heart = if *favorite { "♥ " } else { "" };
        ListItem::new(format!("{marker}{heart}{label}"))
    }));
    let main_content = if app.debug_visible {
        let debug_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(content[1]);
        frame.render_widget(
            Paragraph::new(format!(
                "status={} playing={} audio={}",
                app.status, app.is_playing, app.has_audio
            ))
            .block(Block::default().title("DEBUG").borders(Borders::ALL)),
            debug_layout[1],
        );
        debug_layout[0]
    } else {
        content[1]
    };
    frame.render_widget(
        List::new(results).block(Block::default().borders(Borders::ALL)),
        main_content,
    );

    let current = app
        .current_song
        .as_ref()
        .map(|song| format!("{} - {}", song.artist, song.title))
        .unwrap_or_else(|| "Sin canción seleccionada".to_owned());
    let status = match app.player.status {
        PlaybackStatus::Loading => "…",
        PlaybackStatus::Playing => "▶",
        PlaybackStatus::Paused => "⏸",
        PlaybackStatus::Stopped => "■",
        PlaybackStatus::Finished => "✓",
        PlaybackStatus::Error => "!",
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!("{status} {current}")),
            Line::from(format!(
                "{}{}",
                "Progress: ",
                progress_bar(app.current_position, app.duration)
            )),
            Line::from(format!(
                "Volumen: {:>3}% {} {} {}",
                app.volume,
                if app.player.muted { "🔇" } else { "🔊" },
                if app.player.shuffle {
                    "🔀 ON"
                } else {
                    "🔀 OFF"
                },
                match app.player.repeat {
                    RepeatMode::Off => "↻ OFF",
                    RepeatMode::Queue => "↻ QUEUE",
                    RepeatMode::Song => "↻ SONG",
                }
            )),
        ])
        .block(Block::default().title("PLAYER").borders(Borders::ALL)),
        root[2],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "[Space] Pause  [N/P] Track  [←/→] Seek  [+/-] Volume  [M] Mute  [L] Login  [D] Debug  [Q] Quit  • {}",
            app.account_status,
        ))
        .block(Block::default().borders(Borders::ALL)),
        root[3],
    );
}

fn progress_bar(position: u64, duration: u64) -> String {
    let width: usize = 20;
    let filled = if duration == 0 {
        0
    } else {
        ((position.min(duration) * width as u64) / duration) as usize
    };
    format!(
        "[{}{}] {:02}:{:02} / {:02}:{:02}",
        "█".repeat(filled),
        "░".repeat(width - filled),
        position / 60,
        position % 60,
        duration / 60,
        duration % 60
    )
}
