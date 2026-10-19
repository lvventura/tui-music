//! Right panel of the Track view: a read-only view of the playback queue.
//! Artist (dim), title (bright) and duration (right-aligned); the currently
//! playing row is highlighted with the accent colour as a full-row background.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{TrackData, truncate};
use crate::ui::{glyphs::GlyphSet, theme::Theme};

pub fn draw(frame: &mut Frame<'_>, inner: Rect, data: &TrackData<'_>, theme: &Theme, glyphs: GlyphSet) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let rows = inner.height as usize;
    let current = data.queue_index;
    let start = if current >= rows { current + 1 - rows } else { 0 };

    for row in 0..rows {
        let index = start + row;
        let Some(song) = data.queue.get(index) else {
            break;
        };
        let rect = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        let line = row_line(song, index == current, inner.width as usize, theme, glyphs);
        let style = if index == current {
            Style::default().bg(theme.bright)
        } else {
            Style::default()
        };
        frame.render_widget(Paragraph::new(line).style(style), rect);
    }
}

fn row_line(
    song: &crate::youtube::models::Song,
    highlighted: bool,
    width: usize,
    theme: &Theme,
    glyphs: GlyphSet,
) -> Line<'static> {
    let duration = song
        .duration
        .map(format_time)
        .unwrap_or_else(|| "--:--".to_string());
    let duration_width = duration.chars().count();
    let marker_width = glyphs.row_marker.chars().count() + 1;
    let text_width = width.saturating_sub(marker_width + 1 + duration_width);
    let artist_width = (text_width * 2 / 5).max(1);
    let title_width = text_width.saturating_sub(artist_width);

    let artist = truncate(&song.artist, artist_width, glyphs.ellipsis);
    let title = truncate(&song.title, title_width, glyphs.ellipsis);
    let artist_padded = format!("{artist:<artist_width$}");
    let title_padded = format!("{title:<title_width$}");
    let marker = if highlighted {
        glyphs.row_marker
    } else {
        ""
    };
    let marker_padded = format!("{marker:<marker_width$}");

    let (marker_style, artist_style, title_style, duration_style) = if highlighted {
        let base = Style::default().fg(theme.dark);
        (
            base.add_modifier(Modifier::BOLD),
            base,
            base.add_modifier(Modifier::BOLD),
            base,
        )
    } else {
        (
            Style::default().fg(theme.mid),
            Style::default().fg(theme.mid),
            Style::default().fg(theme.bright),
            Style::default().fg(theme.mid),
        )
    };

    Line::from(vec![
        Span::styled(marker_padded, marker_style),
        Span::styled(artist_padded, artist_style),
        Span::styled(title_padded, title_style),
        Span::raw(" "),
        Span::styled(duration, duration_style),
    ])
}

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::glyphs;
    use crate::youtube::models::Song;
    use ratatui::style::Color;

    fn theme() -> Theme {
        Theme {
            name: "test".into(),
            dark: Color::Rgb(10, 10, 10),
            mid: Color::Rgb(80, 80, 80),
            bright: Color::Rgb(240, 240, 240),
        }
    }

    fn song(artist: &str, title: &str, duration: u64) -> Song {
        Song {
            id: title.into(),
            title: title.into(),
            artist: artist.into(),
            duration: Some(duration),
            thumbnail: None,
            webpage_url: None,
        }
    }

    #[test]
    fn row_line_fits_the_width() {
        let line = row_line(&song("A very long artist name", "A long title here", 336), true, 40, &theme(), glyphs::full());
        let width: usize = line.spans.iter().map(|span| span.content.chars().count()).sum();
        assert_eq!(width, 40);
    }

    #[test]
    fn highlighted_row_uses_dim_text() {
        let line = row_line(&song("Artist", "Title", 60), true, 30, &theme(), glyphs::full());
        assert_eq!(line.spans[1].style.fg, Some(theme().dark));
    }

    #[test]
    fn drawing_an_empty_queue_does_not_panic() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let backend = TestBackend::new(20, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let data = TrackData {
            title: "T",
            artist: "A",
            album: None,
            position: 0,
            duration: 0,
            playing: false,
            has_track: true,
            queue: &[],
            queue_index: 0,
            text: crate::i18n::Text::es(),
        };
        terminal
            .draw(|frame| draw(frame, Rect::new(0, 0, 20, 6), &data, &theme(), glyphs::basic()))
            .unwrap();
    }
}
