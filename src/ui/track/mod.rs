//! Track view: two side-by-side panels ("now playing" and "up next").
//!
//! The modules here only receive already-prepared data (title, artist, album,
//! position, duration, playing flag and the queue rows); they never know about
//! the player, Rodio or the queue's mutation logic. All backgrounds are left as
//! `Color::Reset` except the highlighted queue row, so the terminal's own
//! transparency shows through.

pub mod now_playing;
pub mod up_next;
pub mod visualizer;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::Block,
};

use crate::i18n::Text;
use crate::ui::{glyphs::GlyphSet, theme::Theme};
use crate::youtube::models::Song;

/// Everything the Track view needs to paint itself.
pub struct TrackData<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: Option<String>,
    pub position: u64,
    pub duration: u64,
    pub playing: bool,
    pub has_track: bool,
    pub queue: &'a [Song],
    pub queue_index: usize,
    pub text: Text,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackLayout {
    pub now: Rect,
    pub up_next: Rect,
    pub now_inner: Rect,
    pub up_inner: Rect,
}

/// Splits the Track area into the two panels and their inner rectangles.
pub fn layout(area: Rect) -> TrackLayout {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    TrackLayout {
        now: columns[0],
        up_next: columns[1],
        now_inner: inset(columns[0]),
        up_inner: inset(columns[1]),
    }
}

/// The progress row, shared by rendering and mouse hit-testing.
pub fn progress_rect(area: Rect) -> Rect {
    now_playing::progress_row(layout(area).now_inner)
}

/// Paints the whole Track view inside `area`.
pub fn draw(frame: &mut Frame<'_>, area: Rect, data: &TrackData<'_>, theme: &Theme, glyphs: GlyphSet) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let layout = layout(area);
    draw_panel(frame, layout.now, data.text.panel_now_playing, theme);
    draw_panel(frame, layout.up_next, data.text.panel_up_next, theme);

    visualizer::set_width(layout.now_inner.width as usize);
    if data.has_track {
        visualizer::advance(data.playing);
    } else {
        visualizer::reset();
    }

    now_playing::draw(frame, layout.now_inner, data, theme, glyphs);
    up_next::draw(frame, layout.up_inner, data, theme, glyphs);
}

fn draw_panel(frame: &mut Frame<'_>, area: Rect, title: &str, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let block = Block::bordered()
        .title(Span::styled(
            title,
            Style::default().fg(theme.mid).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(theme.dark));
    frame.render_widget(block, area);
}

fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

/// Truncates `text` to `width` characters, appending the ellipsis when clipped.
pub fn truncate(text: &str, width: usize, ellipsis: &str) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    let ellipsis_len = ellipsis.chars().count();
    if ellipsis_len >= width {
        return ellipsis.chars().take(width).collect();
    }
    let keep = width - ellipsis_len;
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(ellipsis);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::glyphs;

    #[test]
    fn truncate_keeps_short_text() {
        assert_eq!(truncate("abc", 5, glyphs::full().ellipsis), "abc");
    }

    #[test]
    fn truncate_adds_ellipsis_when_clipped() {
        assert_eq!(truncate("abcdefgh", 5, "…"), "abcd…");
        assert_eq!(truncate("abcdefgh", 5, "..."), "ab...");
    }

    #[test]
    fn truncate_handles_zero_width() {
        assert_eq!(truncate("abc", 0, "…"), "");
    }

    #[test]
    fn layout_panels_tile_the_area() {
        let area = Rect::new(0, 0, 100, 20);
        let layout = layout(area);
        assert_eq!(layout.now.x, 0);
        assert_eq!(layout.up_next.x + layout.up_next.width, 100);
        assert!(!layout.now.is_empty());
        assert!(!layout.up_next.is_empty());
    }

    #[test]
    fn progress_rect_is_inside_the_left_panel() {
        let area = Rect::new(0, 0, 100, 20);
        let row = progress_rect(area);
        let layout = layout(area);
        assert_eq!(row.y, layout.now_inner.y + layout.now_inner.height - 1);
        assert!(row.x >= layout.now.x);
        assert!(row.x + row.width <= layout.now.x + layout.now.width);
    }
}
