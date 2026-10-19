//! Decorative equalizer-bar visualizer for the Track view.
//!
//! This widget is purely cosmetic: it does **not** analyze the audio or read
//! real playback levels. It only reacts to whether the player is currently
//! playing, so the bars move while music is on and freeze while paused.
//!
//! The shape is deterministic (an internal LCG provides the pseudo-random
//! variation) with more energy on the left (bass) decaying towards the right,
//! fast attack and slow release. Colour is a vertical gradient interpolated
//! between two theme colours. All background is left as `Color::Reset` so the
//! terminal's own transparency shows through.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
};

use crate::ui::{glyphs::GlyphSet, theme::{self, Theme}};

/// Upper bound on the number of logical bars (only `area.width` are drawn).
const MAX_BARS: usize = 256;

/// Animated, decorative equalizer bars.
pub struct Visualizer {
    bars: Vec<f32>,
    phase: f32,
    rng: u32,
    enabled: bool,
    last: Option<Instant>,
}

impl Visualizer {
    pub fn new() -> Self {
        Self {
            bars: vec![0.0; MAX_BARS],
            phase: 0.0,
            rng: 0x1234_5678,
            enabled: true,
            last: None,
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Drop every bar back to zero (used when there is no current track).
    pub fn reset(&mut self) {
        self.bars.fill(0.0);
        self.phase = 0.0;
    }

    /// Fit the bar count to the panel width so the shape spans exactly the
    /// visible columns.
    pub fn set_len(&mut self, len: usize) {
        self.bars.resize(len.clamp(1, MAX_BARS), 0.0);
    }

    /// Advance the animation using the wall-clock delta since the last call.
    fn advance(&mut self, playing: bool) {
        let now = Instant::now();
        let dt = self.last.map(|last| now.duration_since(last)).unwrap_or_default();
        self.last = Some(now);
        self.tick(dt, playing);
    }

    /// Advance the animation by `dt`. When `playing` is false the bars freeze
    /// (paused/stopped), so they never drift on their own.
    pub fn tick(&mut self, dt: Duration, playing: bool) {
        if !playing {
            return;
        }
        let dt = dt.as_secs_f32().clamp(0.0, 0.25);
        self.phase = (self.phase + dt * 2.6) % std::f32::consts::TAU;
        let count = self.bars.len().max(1) as f32;
        for i in 0..self.bars.len() {
            let position = i as f32 / count;
            let decay = 1.0 - 0.55 * position;
            let a = (self.phase * (1.0 + 0.03 * i as f32) + i as f32 * 0.5).sin();
            self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (self.rng >> 9) as f32 / 8_388_608.0;
            let wave = 0.5 + 0.5 * (0.7 * a + 0.3 * (noise * 2.0 - 1.0));
            let target = (decay * wave).clamp(0.0, 1.0);
            let bar = &mut self.bars[i];
            let factor = if target > *bar { 0.6 } else { 0.10 };
            *bar += (target - *bar) * factor;
            *bar = bar.clamp(0.0, 1.0);
        }
    }

    /// Draw the bars into `area`. Small/zero-sized areas are ignored, and every
    /// cell written stays strictly inside `area`.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme, glyphs: GlyphSet) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let count = (area.width as usize).min(self.bars.len());
        for i in 0..count {
            let value = self.bars[i].clamp(0.0, 1.0);
            let x = area.x + i as u16;
            if glyphs.octave {
                self.render_octave(area, buf, x, value, theme, glyphs);
            } else {
                self.render_half(area, buf, x, value, theme, glyphs);
            }
        }
    }

    fn render_octave(
        &self,
        area: Rect,
        buf: &mut Buffer,
        x: u16,
        value: f32,
        theme: &Theme,
        glyphs: GlyphSet,
    ) {
        let height = area.height as f32;
        let eighths = (value * height * 8.0).round() as u32;
        let full = eighths / 8;
        let rem = (eighths % 8) as usize;
        for r in 0..full {
            let y = area.y + area.height - 1 - r as u16;
            let t = (r as f32 + 1.0) / height;
            buf[(x, y)]
                .set_symbol(glyphs.blocks[7])
                .set_style(Style::default().fg(theme::lerp_color(theme.mid, theme.bright, t)));
        }
        if rem > 0 && full < area.height as u32 {
            let y = area.y + area.height - 1 - full as u16;
            let t = (full as f32 + rem as f32 / 8.0) / height;
            buf[(x, y)]
                .set_symbol(glyphs.blocks[rem - 1])
                .set_style(Style::default().fg(theme::lerp_color(theme.mid, theme.bright, t)));
        }
    }

    fn render_half(
        &self,
        area: Rect,
        buf: &mut Buffer,
        x: u16,
        value: f32,
        theme: &Theme,
        glyphs: GlyphSet,
    ) {
        let height = area.height as f32;
        let halves = (value * height * 2.0).round() as u32;
        let full = halves / 2;
        let rem = halves % 2;
        for r in 0..full {
            let y = area.y + area.height - 1 - r as u16;
            let t = (r as f32 + 1.0) / height;
            buf[(x, y)]
                .set_symbol(glyphs.bar_full)
                .set_style(Style::default().fg(theme::lerp_color(theme.mid, theme.bright, t)));
        }
        if rem == 1 && full < area.height as u32 {
            let y = area.y + area.height - 1 - full as u16;
            let t = (full as f32 + 0.5) / height;
            buf[(x, y)]
                .set_symbol(glyphs.bar_half)
                .set_style(Style::default().fg(theme::lerp_color(theme.mid, theme.bright, t)));
        }
    }
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static VISUALIZER: RefCell<Visualizer> = RefCell::new(Visualizer::new());
}

/// Flip the singleton on/off (the `v` key). Returns the new state.
pub fn toggle() -> bool {
    VISUALIZER.with(|cell| cell.borrow_mut().toggle())
}

/// Whether the singleton is currently enabled.
pub fn enabled() -> bool {
    VISUALIZER.with(|cell| cell.borrow().enabled())
}

/// Reset the singleton bars to zero.
pub fn reset() {
    VISUALIZER.with(|cell| cell.borrow_mut().reset());
}

/// Fit the singleton to the current panel width.
pub fn set_width(width: usize) {
    VISUALIZER.with(|cell| cell.borrow_mut().set_len(width));
}

/// Advance the singleton using the wall-clock delta.
pub fn advance(playing: bool) {
    VISUALIZER.with(|cell| cell.borrow_mut().advance(playing));
}

/// Render the singleton into `area`.
pub fn render(area: Rect, buf: &mut Buffer, theme: &Theme, glyphs: GlyphSet) {
    VISUALIZER.with(|cell| cell.borrow().render(area, buf, theme, glyphs));
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn blocks() -> GlyphSet {
        glyphs::full()
    }

    fn is_block(symbol: &str) -> bool {
        blocks().blocks.contains(&symbol)
    }

    #[test]
    fn stopped_bars_are_zero_and_draw_nothing() {
        let mut visualizer = Visualizer::new();
        for _ in 0..30 {
            visualizer.tick(Duration::from_millis(50), false);
        }
        assert!(visualizer.bars.iter().all(|bar| *bar == 0.0));

        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        visualizer.render(area, &mut buffer, &theme(), blocks());
        assert!(buffer.content().iter().all(|cell| !is_block(cell.symbol())));
    }

    #[test]
    fn paused_freezes_the_bars() {
        let mut visualizer = Visualizer::new();
        visualizer.bars[0] = 1.0;
        visualizer.bars[1] = 0.5;
        for _ in 0..10 {
            visualizer.tick(Duration::from_millis(80), false);
        }
        assert_eq!(visualizer.bars[0], 1.0);
        assert_eq!(visualizer.bars[1], 0.5);
    }

    #[test]
    fn values_stay_in_unit_range() {
        let mut visualizer = Visualizer::new();
        for _ in 0..200 {
            visualizer.tick(Duration::from_millis(40), true);
        }
        assert!(visualizer.bars.iter().all(|bar| (0.0..=1.0).contains(bar)));
    }

    #[test]
    fn bars_never_leave_the_area() {
        let mut visualizer = Visualizer::new();
        for _ in 0..40 {
            visualizer.tick(Duration::from_millis(50), true);
        }
        let area = Rect::new(5, 2, 20, 6);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 12));
        visualizer.render(area, &mut buffer, &theme(), blocks());
        for (index, cell) in buffer.content().iter().enumerate() {
            if !is_block(cell.symbol()) {
                continue;
            }
            let x = index as u16 % 40;
            let y = index as u16 / 40;
            assert!(
                x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height,
                "block at ({x},{y}) left the area"
            );
        }
    }

    #[test]
    fn basic_glyph_set_draws_half_blocks() {
        let mut visualizer = Visualizer::new();
        for _ in 0..30 {
            visualizer.tick(Duration::from_millis(50), true);
        }
        let area = Rect::new(0, 0, 30, 6);
        let mut buffer = Buffer::empty(area);
        visualizer.render(area, &mut buffer, &theme(), glyphs::basic());
        assert!(buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() == "█" || cell.symbol() == "▄"));
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        let mut visualizer = Visualizer::new();
        visualizer.tick(Duration::from_millis(50), true);
        let mut zero = Buffer::empty(Rect::new(0, 0, 0, 0));
        visualizer.render(Rect::new(0, 0, 0, 0), &mut zero, &theme(), blocks());
        let mut flat = Buffer::empty(Rect::new(0, 0, 8, 0));
        visualizer.render(Rect::new(0, 0, 8, 0), &mut flat, &theme(), blocks());
    }

    #[test]
    fn toggle_flips_enabled() {
        let mut visualizer = Visualizer::new();
        assert!(visualizer.enabled());
        assert!(!visualizer.toggle());
        assert!(!visualizer.enabled());
        assert!(visualizer.toggle());
    }
}
