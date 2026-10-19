use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{
    Action, App,
    state::{MENU_LEN, View, menu_labels},
};
use crate::player::state::RepeatMode;
use crate::ui::{
    keymap,
    theme::Theme,
    track::{self, TrackData},
};

/// Single source of truth for every panel rectangle, shared by rendering and
/// the mouse hit-testing / cover painting.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub tabs: Rect,
    pub body: Rect,
    pub player: Rect,
    pub help: Rect,
    pub library: Vec<Rect>,
    pub cover: Rect,
}

pub fn geometry(area: Rect) -> Geometry {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(area);
    let body = rows[1];
    let library = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Percentage(36),
            Constraint::Percentage(36),
        ])
        .split(body)
        .to_vec();
    Geometry {
        tabs: rows[0],
        body,
        player: rows[2],
        help: rows[3],
        library,
        cover: cover_rect(body),
    }
}

fn cover_rect(body: Rect) -> Rect {
    let width = (body.width / 2).min(44);
    let height = body.height.saturating_sub(2);
    if width < 6 || height < 3 {
        return Rect::default();
    }
    Rect {
        x: body.x + body.width - width - 1,
        y: body.y + 1,
        width,
        height,
    }
}

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let geo = geometry(frame.area());
    draw_tabs(frame, geo.tabs, app);
    match app.view {
        View::Library => draw_library(frame, &geo, app),
        View::NowPlaying => {
            let data = track_data(app);
            track::draw(frame, track_area(&geo), &data, &app.theme, app.glyphs);
        }
        View::Queue | View::Search => draw_list(frame, geo.body, app),
    }
    if app.view != View::NowPlaying {
        draw_player(frame, geo.player, app);
    }
    draw_help(frame, geo.help, app);
    if app.show_menu {
        draw_menu(frame, frame.area(), app);
    }
}

/// The Track view owns the body plus the player band, so the two panels fill
/// the space left by the hidden bottom player bar.
fn track_area(geo: &Geometry) -> Rect {
    Rect {
        x: geo.body.x,
        y: geo.body.y,
        width: geo.body.width,
        height: geo.body.height + geo.player.height,
    }
}

fn track_data(app: &App) -> TrackData<'_> {
    let (title, artist) = app
        .current_song
        .as_ref()
        .map(|song| (song.title.as_str(), song.artist.as_str()))
        .unwrap_or(("", ""));
    let album = if app.current_song.is_some() {
        track::now_playing::album_from_title(title)
    } else {
        None
    };
    TrackData {
        title,
        artist,
        album,
        position: app.current_position,
        duration: app.duration,
        playing: app.player.status == crate::player::state::PlaybackStatus::Playing,
        has_track: app.current_song.is_some(),
        queue: &app.queue,
        queue_index: app.queue_index,
        text: app.text,
    }
}

fn panel<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(theme.bright).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.dark))
}

fn draw_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel("", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(40)])
        .split(inner);

    let tabs: Vec<Span> = View::ALL
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let style = if *view == app.view {
                Style::default()
                    .fg(theme.bright)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(theme.mid)
            };
            Span::styled(format!(" {} {} ", index + 1, view.title(&app.text)), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(tabs)), columns[0]);

    let queue = format!(
        "{}/{}",
        (app.queue_index + 1).min(app.queue.len().max(1)),
        app.queue.len()
    );
    let repeat = match app.player.repeat {
        RepeatMode::Off => app.glyphs.repeat_off,
        RepeatMode::Queue => app.glyphs.repeat_all,
        RepeatMode::Song => app.glyphs.repeat_one,
    };
    let shuffle = if app.player.shuffle {
        app.glyphs.shuffle_on
    } else {
        app.glyphs.shuffle_off
    };
    let mut status = vec![
        Span::styled(format!("{} ", app.account_status), Style::default().fg(theme.mid)),
        Span::styled(format!("q {queue} "), Style::default().fg(theme.mid)),
        Span::styled(format!("{shuffle} {repeat} "), Style::default().fg(theme.mid)),
        volume_meter(app, theme),
    ];
    status.shrink_to_fit();
    frame.render_widget(
        Paragraph::new(Line::from(status)).alignment(Alignment::Right),
        columns[1],
    );
}

fn volume_meter<'a>(app: &App, theme: &Theme) -> Span<'a> {
    if app.player.muted || app.volume == 0 {
        return Span::styled(
            app.text.muted,
            Style::default()
                .fg(theme.bright)
                .add_modifier(Modifier::BOLD),
        );
    }
    let filled = ((app.volume as u32 * 5) / 100).min(5) as usize;
    let meter = format!(
        "{}{}",
        app.glyphs.volume_full.repeat(filled),
        app.glyphs.volume_empty.repeat(5 - filled)
    );
    Span::styled(meter, Style::default().fg(theme.bright))
}

fn song_items<'a>(songs: &'a [crate::youtube::models::Song], selected: usize, theme: &Theme) -> Vec<ListItem<'a>> {
    songs
        .iter()
        .enumerate()
        .map(|(index, song)| {
            let marker = if index == selected { "\u{25b8} " } else { "  " };
            let style = if index == selected {
                Style::default()
                    .fg(theme.bright)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.mid)
            };
            ListItem::new(format!("{marker}{} - {}", song.artist, song.title)).style(style)
        })
        .collect()
}

fn draw_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.view {
        View::Search => draw_search(frame, area, app),
        _ => {
            let theme = &app.theme;
            let block = panel(app.text.panel_queue, theme);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                List::new(song_items(&app.queue, app.queue_index, theme)),
                inner,
            );
        }
    }
}

fn draw_search(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let border = if app.search_mode {
        theme.bright
    } else {
        theme.dark
    };
    let block = Block::default()
        .title(Span::styled(
            app.text.panel_search,
            Style::default().fg(theme.bright).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let prompt = if app.searching {
        Span::styled(app.text.searching, Style::default().fg(theme.bright))
    } else if app.search_query.is_empty() && !app.search_mode {
        Span::styled(app.text.search_prompt, Style::default().fg(theme.mid))
    } else {
        let cursor = if app.search_mode { "\u{2588}" } else { "" };
        Span::styled(
            format!("> {}{}", app.search_query, cursor),
            Style::default()
                .fg(theme.bright)
                .add_modifier(Modifier::BOLD),
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(prompt)),
        Rect { height: 1, ..inner },
    );

    let results = Rect {
        y: inner.y + 1,
        height: inner.height.saturating_sub(1),
        ..inner
    };
    if results.height == 0 {
        return;
    }
    frame.render_widget(
        List::new(song_items(&app.search_results, app.selected_result, theme)),
        results,
    );
}

fn draw_library(frame: &mut Frame<'_>, geo: &Geometry, app: &App) {
    let theme = &app.theme;
    let titles = app.text.library_columns;
    for (column, rect) in geo.library.iter().enumerate() {
        let block = panel(titles[column], theme);
        let inner = block.inner(*rect);
        frame.render_widget(block, *rect);
        let selected = app.library_indices[column];
        let focused = app.library_focus == column;
        let highlight = |index: usize| -> Style {
            if focused && index == selected {
                Style::default()
                    .fg(theme.bright)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.mid)
            }
        };
        let items: Vec<ListItem> = match column {
            0 => (0..3)
                .map(|index| {
                    let name = match app.library_category(index) {
                        crate::app::state::Screen::History => app.text.screen_history,
                        crate::app::state::Screen::Favorites => app.text.screen_favorites,
                        _ => app.text.screen_playlists,
                    };
                    ListItem::new(format!("  {name}")).style(highlight(index))
                })
                .collect(),
            1 => match app.screen {
                crate::app::state::Screen::History => song_items_styled(&app.history, selected, theme, focused),
                crate::app::state::Screen::Favorites => song_items_styled(&app.favorites, selected, theme, focused),
                crate::app::state::Screen::Playlists => app
                    .playlists
                    .iter()
                    .enumerate()
                    .map(|(index, (_, name))| ListItem::new(format!("  {name}")).style(highlight(index)))
                    .collect(),
                _ => Vec::new(),
            },
            _ => song_items_styled(&app.playlist_songs, selected, theme, focused),
        };
        frame.render_widget(List::new(items), inner);
    }
}

fn song_items_styled<'a>(
    songs: &'a [crate::youtube::models::Song],
    selected: usize,
    theme: &Theme,
    focused: bool,
) -> Vec<ListItem<'a>> {
    songs
        .iter()
        .enumerate()
        .map(|(index, song)| {
            let marker = if focused && index == selected { "\u{25b8} " } else { "  " };
            let style = if focused && index == selected {
                Style::default().fg(theme.bright).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.mid)
            };
            ListItem::new(format!("{marker}{} - {}", song.artist, song.title)).style(style)
        })
        .collect()
}

fn draw_player(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = panel(app.text.panel_player, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (title, artist) = app
        .current_song
        .as_ref()
        .map(|song| (song.title.clone(), song.artist.clone()))
        .unwrap_or_else(|| (app.text.no_track_selected.to_string(), String::new()));
    let status = app.glyphs.status(app.player.status);
    let line1 = Line::from(vec![
        Span::styled(format!("{status} "), Style::default().fg(theme.bright)),
        Span::styled(title, Style::default().fg(theme.bright).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  \u{2014} {artist}"), Style::default().fg(theme.mid)),
    ]);
    let line2 = Line::from(vec![
        Span::styled(
            progress_bar(app.current_position, app.duration),
            Style::default().fg(theme.bright),
        ),
        Span::styled(
            format!(
                "  {} / {}",
                format_time(app.current_position),
                format_time(app.duration)
            ),
            Style::default().fg(theme.mid),
        ),
    ]);
    frame.render_widget(Paragraph::new(vec![line1, line2]), inner);
}

fn draw_help(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let mut text = keymap::help_line(app.keymap, &app.text);
    if let Some(note) = &app.terminal_note {
        text.push_str("  \u{2022} ");
        text.push_str(note);
    }
    frame.render_widget(
        Paragraph::new(Span::styled(text, Style::default().fg(theme.mid))),
        area,
    );
}

fn draw_menu(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let width = area.width.min(56);
    let height = area.height.clamp(12, 24);
    let popup = centered(area, width, height);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(Span::styled(
            app.text.menu_title,
            Style::default().fg(theme.bright).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.bright))
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines: Vec<Line> = BANNER
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                *line,
                Style::default().fg(theme.bright),
            ))
        })
        .collect();
    lines.push(Line::from(Span::styled(
        format!("  {}", app.text.app_name),
        Style::default().fg(theme.mid).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let labels = menu_labels(&app.text);
    let values = [
        app.theme_name.clone(),
        format!("{}%", app.volume),
        if app.player.shuffle { app.text.value_on } else { app.text.value_off }.to_string(),
        match app.player.repeat {
            RepeatMode::Off => app.text.repeat_off_value,
            RepeatMode::Queue => app.text.repeat_all_value,
            RepeatMode::Song => app.text.repeat_one_value,
        }
        .to_string(),
        app.keymap.label().to_string(),
        app.language.label().to_string(),
        String::new(),
    ];
    for index in 0..MENU_LEN {
        let focused = index == app.menu_option;
        let marker = if focused { "\u{25b8} " } else { "  " };
        let style = if focused {
            Style::default().fg(theme.bright).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.mid)
        };
        let arrow = if focused { "\u{2190} \u{2192}" } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{marker}{:<10} {:<16} {arrow}", labels[index], values[index]),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        app.text.menu_help,
        Style::default().fg(theme.mid),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

const BANNER: [&str; 6] = [
    "██╗      ██╗   ████████╗ ███████╗ ",
    "██║      ██║   ╚══██╔══╝ ██╔════╝ ",
    "██║      ██║      ██║    █████╗   ",
    "██║      ██║      ██║    ██╔══╝   ",
    "███████╗ ██║      ██║    ███████╗ ",
    "╚══════╝ ╚═╝      ╚═╝    ╚══════╝ ",
];

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn progress_bar(position: u64, duration: u64) -> String {
    let width: usize = 24;
    let filled = position
        .min(duration)
        .saturating_mul(width as u64)
        .checked_div(duration)
        .unwrap_or(0) as usize;
    format!(
        "[{}{}]",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(width - filled)
    )
}

/// Translates a mouse event into an action using the shared geometry.
pub fn hit_test(area: Rect, app: &App, event: MouseEvent) -> Option<Action> {
    let geo = geometry(area);
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if contains(geo.tabs, event.column, event.row) {
                return tab_at(geo.tabs, event.column).map(Action::SwitchView);
            }
            if app.view == View::NowPlaying {
                let progress = track::progress_rect(track_area(&geo));
                if contains(progress, event.column, event.row) {
                    return seek_rect(progress, event.column, app).map(Action::SeekTo);
                }
                return None;
            }
            if contains(geo.player, event.column, event.row) {
                return seek_at(geo.player, event.column, app).map(Action::SeekTo);
            }
            if app.view == View::Library {
                if let Some((column, index)) = library_at(&geo, event.column, event.row) {
                    return Some(Action::LibraryClick { column, index });
                }
            } else if let Some(index) = list_at(geo.body, event.column, event.row) {
                return Some(Action::SelectIndex(index));
            }
            None
        }
        MouseEventKind::Down(MouseButton::Right) => Some(Action::PlayPause),
        MouseEventKind::ScrollUp => wheel(&geo, app, event, -1),
        MouseEventKind::ScrollDown => wheel(&geo, app, event, 1),
        _ => None,
    }
}

fn wheel(geo: &Geometry, app: &App, event: MouseEvent, direction: i32) -> Option<Action> {
    let over_list = contains(geo.body, event.column, event.row)
        && matches!(app.view, View::Queue | View::Search | View::Library);
    if over_list {
        Some(Action::ScrollSelection(direction))
    } else {
        let volume = (app.volume as i32 + direction * 5).clamp(0, 100) as u8;
        Some(Action::SetVolume(volume))
    }
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

fn tab_at(tabs: Rect, x: u16) -> Option<View> {
    let inner_x = tabs.x + 1;
    let inner_w = tabs.width.saturating_sub(2).max(1);
    if x < inner_x || x >= inner_x + inner_w {
        return None;
    }
    let index = (((x - inner_x) as usize * View::ALL.len()) / inner_w as usize).min(View::ALL.len() - 1);
    View::ALL.get(index).copied()
}

fn list_at(body: Rect, x: u16, y: u16) -> Option<usize> {
    if x < body.x || x >= body.x + body.width {
        return None;
    }
    let content_start = body.y + 1;
    if y < content_start || y >= body.y + body.height.saturating_sub(1) {
        return None;
    }
    Some((y - content_start) as usize)
}

fn library_at(geo: &Geometry, x: u16, y: u16) -> Option<(usize, usize)> {
    for (column, rect) in geo.library.iter().enumerate() {
        if contains(*rect, x, y) {
            let content_start = rect.y + 1;
            if y < content_start {
                return None;
            }
            return Some((column, (y - content_start) as usize));
        }
    }
    None
}

fn seek_at(player: Rect, x: u16, app: &App) -> Option<u64> {
    if app.duration == 0 {
        return None;
    }
    let inner_x = player.x + 1;
    let inner_w = player.width.saturating_sub(2).max(1);
    if x < inner_x {
        return Some(0);
    }
    let fraction = ((x - inner_x).min(inner_w) as f64) / inner_w as f64;
    Some((fraction * app.duration as f64).round() as u64)
}

/// Maps a click inside the borderless Track progress row to a timestamp.
fn seek_rect(rect: Rect, x: u16, app: &App) -> Option<u64> {
    if app.duration == 0 || rect.width == 0 {
        return None;
    }
    let clamped = x.clamp(rect.x, rect.x + rect.width - 1);
    let fraction = (clamped - rect.x) as f64 / rect.width as f64;
    Some((fraction * app.duration as f64).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::{geometry, progress_bar};
    use ratatui::layout::Rect;

    #[test]
    fn geometry_partitions_the_area_without_overlap() {
        let area = Rect::new(0, 0, 100, 40);
        let geo = geometry(area);
        assert_eq!(geo.tabs.height, 3);
        assert_eq!(geo.player.height, 4);
        assert_eq!(geo.help.height, 1);
        assert_eq!(geo.body.height, 40 - 3 - 4 - 1);
        assert_eq!(geo.tabs.y, 0);
        assert_eq!(geo.body.y, 3);
        assert_eq!(geo.player.y, geo.body.y + geo.body.height);
        assert_eq!(geo.help.y, geo.player.y + geo.player.height);
    }

    #[test]
    fn library_columns_tile_the_body_width() {
        let area = Rect::new(0, 0, 100, 40);
        let geo = geometry(area);
        assert_eq!(geo.library.len(), 3);
        assert_eq!(geo.library[0].x, geo.body.x);
        let last = &geo.library[2];
        assert_eq!(last.x + last.width, geo.body.x + geo.body.width);
        for column in &geo.library {
            assert_eq!(column.height, geo.body.height);
        }
    }

    #[test]
    fn the_cover_stays_inside_the_body() {
        let area = Rect::new(0, 0, 100, 40);
        let geo = geometry(area);
        assert!(geo.cover.width > 0);
        assert!(geo.cover.x >= geo.body.x);
        assert!(geo.cover.x + geo.cover.width <= geo.body.x + geo.body.width);
        assert!(geo.cover.y + geo.cover.height <= geo.body.y + geo.body.height);
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        let geo = geometry(Rect::new(0, 0, 8, 8));
        assert_eq!(geo.cover.width, 0);
    }

    #[test]
    fn the_progress_bar_fills_proportionally() {
        assert!(progress_bar(0, 60).starts_with("["));
        assert!(progress_bar(60, 60).contains("\u{2588}\u{2588}"));
    }
}
