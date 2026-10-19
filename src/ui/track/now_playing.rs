//! Left panel of the Track view: header text, the decorative visualizer and the
//! single-row progress line. Receives already-prepared data and never touches
//! the player or the queue.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{TrackData, truncate, visualizer};
use crate::ui::{glyphs::GlyphSet, theme::Theme};

impl<'a> TrackData<'a> {
    /// Up to three header rows are used: title, artist and (if any) album.
    fn header_lines(&self) -> usize {
        2 + usize::from(self.album.is_some())
    }
}

/// The progress row always sits on the bottom row of the panel's inner area.
pub fn progress_row(inner: Rect) -> Rect {
    if inner.height == 0 {
        return Rect {
            width: 0,
            ..inner
        };
    }
    Rect {
        x: inner.x,
        y: inner.y + inner.height - 1,
        width: inner.width,
        height: 1,
    }
}

/// Paints header, visualizer band and progress line inside `inner`.
pub fn draw(
    frame: &mut Frame<'_>,
    inner: Rect,
    data: &TrackData<'_>,
    theme: &Theme,
    glyphs: GlyphSet,
) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let header_lines = data.header_lines().min(inner.height as usize);

    let mut lines: Vec<Line> = Vec::with_capacity(header_lines);
    if data.has_track {
        lines.push(Line::from(Span::styled(
            truncate(data.title, width, glyphs.ellipsis),
            Style::default().fg(theme.bright).add_modifier(Modifier::BOLD),
        )));
        if header_lines > 1 {
            lines.push(Line::from(Span::styled(
                truncate(data.artist, width, glyphs.ellipsis),
                Style::default().fg(theme.mid),
            )));
        }
        if header_lines > 2 {
            let album = data.album.clone().unwrap_or_default();
            lines.push(Line::from(Span::styled(
                truncate(&album, width, glyphs.ellipsis),
                Style::default().fg(theme.dark),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            truncate(data.text.nothing_playing, width, glyphs.ellipsis),
            Style::default().fg(theme.mid),
        )));
    }
    let header = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: header_lines as u16,
    };
    frame.render_widget(Paragraph::new(lines), header);

    let progress = progress_row(inner);
    let available = inner
        .height
        .saturating_sub(header_lines as u16 + 1);
    let vis_height = if visualizer::enabled() && available >= 4 {
        available.min(10)
    } else {
        0
    };
    if vis_height > 0 {
        let vis = Rect {
            x: inner.x,
            y: progress.y - vis_height,
            width: inner.width,
            height: vis_height,
        };
        visualizer::render(vis, frame.buffer_mut(), theme, glyphs);
    }

    draw_progress(frame, progress, data, theme, glyphs);
}

fn draw_progress(
    frame: &mut Frame<'_>,
    area: Rect,
    data: &TrackData<'_>,
    theme: &Theme,
    glyphs: GlyphSet,
) {
    if area.width == 0 {
        return;
    }
    let left = format_time(data.position);
    let right = if data.duration > 0 {
        format_time(data.duration)
    } else {
        "--:--".to_string()
    };
    let reserved = left.chars().count() + right.chars().count() + 2;
    let track_width = (area.width as usize).saturating_sub(reserved);

    let fraction = if data.duration > 0 {
        (data.position.min(data.duration) as f64) / (data.duration as f64)
    } else {
        0.0
    };
    let marker = if track_width == 0 {
        0
    } else {
        ((track_width as f64 - 1.0) * fraction).round() as usize
    };
    let mut bar = String::new();
    for i in 0..track_width {
        if i == marker && track_width > 0 {
            bar.push_str(glyphs.progress_marker);
        } else {
            bar.push_str(glyphs.progress_line);
        }
    }

    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(theme.mid)),
        Span::raw(" "),
        Span::styled(bar, Style::default().fg(theme.bright)),
        Span::raw(" "),
        Span::styled(right, Style::default().fg(theme.mid)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// Best-effort album extraction from a YouTube title (the model has no album
/// field). Uses a bracketed segment or the middle part of an
/// `Artist - Album - Track` title, ignoring common noise tags.
pub fn album_from_title(title: &str) -> Option<String> {
    for (open, close) in [('[', ']'), ('(', ')')] {
        if let Some(start) = title.find(open)
            && let Some(offset) = title[start + 1..].find(close)
        {
            let inner = title[start + 1..start + 1 + offset].trim();
            if is_album_candidate(inner) {
                return Some(inner.to_string());
            }
        }
    }
    let parts: Vec<&str> = title.split(" - ").map(str::trim).collect();
    if parts.len() >= 3 && is_album_candidate(parts[1]) {
        return Some(parts[1].to_string());
    }
    None
}

fn is_album_candidate(text: &str) -> bool {
    if text.is_empty() || text.chars().count() > 60 {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    const NOISE: &[&str] = &[
        "official",
        "video",
        "audio",
        "lyric",
        "hd",
        "4k",
        "m/v",
        "remaster",
        "live",
        "visualizer",
        "radio edit",
        "extended",
        "explicit",
        "topic",
        "full album",
        "feat",
        "ft.",
        "prod.",
        "mv",
    ];
    !NOISE.iter().any(|noise| lower.contains(noise))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Text;
    use crate::ui::glyphs;
    use ratatui::style::Color;

    fn theme() -> Theme {
        Theme {
            name: "test".into(),
            dark: Color::Rgb(10, 10, 10),
            mid: Color::Rgb(80, 80, 80),
            bright: Color::Rgb(240, 240, 240),
        }
    }

    fn data<'a>(title: &'a str, album: Option<String>) -> TrackData<'a> {
        TrackData {
            title,
            artist: "Artista",
            album,
            position: 30,
            duration: 120,
            playing: true,
            has_track: true,
            queue: &[],
            queue_index: 0,
            text: Text::es(),
        }
    }

    #[test]
    fn album_is_extracted_from_brackets() {
        assert_eq!(
            album_from_title("Song (Midnight Album)"),
            Some("Midnight Album".to_string())
        );
    }

    #[test]
    fn album_ignores_noise_tags() {
        assert_eq!(album_from_title("Song (Official Video)"), None);
        assert_eq!(album_from_title("Song [Official Audio]"), None);
    }

    #[test]
    fn album_from_triple_dash_title() {
        assert_eq!(
            album_from_title("Artist - The Album - Track"),
            Some("The Album".to_string())
        );
        assert_eq!(album_from_title("Artist - Track"), None);
    }

    #[test]
    fn progress_row_is_the_bottom_row() {
        let inner = Rect::new(2, 3, 10, 6);
        let row = progress_row(inner);
        assert_eq!(row.y, 8);
        assert_eq!(row.height, 1);
        assert_eq!(row.width, 10);
    }

    #[test]
    fn zero_area_progress_does_not_panic() {
        let inner = Rect::new(0, 0, 0, 0);
        assert_eq!(progress_row(inner).height, 0);
    }

    #[test]
    fn draw_renders_within_inner() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let backend = TestBackend::new(40, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let inner = Rect::new(2, 2, 30, 12);
        let data = data("Título", Some("Album".to_string()));
        terminal
            .draw(|frame| draw(frame, inner, &data, &theme(), glyphs::full()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let inner_has_text = buffer.content().iter().enumerate().any(|(index, cell)| {
            let x = index as u16 % 40;
            let y = index as u16 / 40;
            !cell.symbol().trim().is_empty()
                && x >= inner.x
                && x < inner.x + inner.width
                && y >= inner.y
                && y < inner.y + inner.height
        });
        assert!(inner_has_text);
    }

    #[test]
    fn empty_track_header_does_not_panic() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let data = TrackData {
            title: "",
            artist: "",
            album: None,
            position: 0,
            duration: 0,
            playing: false,
            has_track: false,
            queue: &[],
            queue_index: 0,
            text: Text::es(),
        };
        terminal
            .draw(|frame| draw(frame, Rect::new(0, 0, 30, 12), &data, &theme(), glyphs::basic()))
            .unwrap();
    }
}
