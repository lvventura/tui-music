//! Central glyph catalogue for the UI.
//!
//! Windows Terminal (detected via `WT_SESSION`) and other truecolor terminals
//! can render the full Unicode set, but the classic Windows console
//! (`cmd.exe`/conhost) uses the raster/Consolas CP437 set and shows missing
//! glyphs as `▯` boxes. Every character used by the Track view (and the tab
//! strip) therefore comes from a [`GlyphSet`]: `full` for capable terminals and
//! `basic` for CP437-only ones. No loose glyph literals belong in the render
//! code.

/// The full Unicode glyph set for capable (truecolor) terminals.
pub fn full() -> GlyphSet {
    GlyphSet {
        blocks: ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"],
        bar_full: "█",
        bar_half: "▄",
        octave: true,
        progress_line: "─",
        progress_marker: "●",
        row_marker: "▌",
        ellipsis: "…",
        play: "▶",
        pause: "⏸",
        stop: "■",
        loading: "…",
        finished: "✓",
        error: "!",
        shuffle_on: "shuffle",
        shuffle_off: "-",
        repeat_off: "↻off",
        repeat_all: "↻all",
        repeat_one: "↻one",
        volume_full: "█",
        volume_empty: "░",
        basic: false,
    }
}

/// The reduced glyph set that exists in code page 437 (classic console).
pub fn basic() -> GlyphSet {
    GlyphSet {
        blocks: ["▄", "▄", "▄", "▄", "▄", "▄", "▄", "█"],
        bar_full: "█",
        bar_half: "▄",
        octave: false,
        progress_line: "-",
        progress_marker: "o",
        row_marker: ">",
        ellipsis: "...",
        play: ">",
        pause: "||",
        stop: "#",
        loading: "...",
        finished: "ok",
        error: "!",
        shuffle_on: "shuf",
        shuffle_off: "-",
        repeat_off: "rep",
        repeat_all: "rep all",
        repeat_one: "rep one",
        volume_full: "#",
        volume_empty: "-",
        basic: true,
    }
}

/// A whole catalogue of UI glyphs. `Copy` so it can be threaded through the
/// render functions without borrowing the `App`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphSet {
    /// Octave block ramp (`▁▂▃▄▅▆▇█`); only used when `octave` is true.
    pub blocks: [&'static str; 8],
    pub bar_full: &'static str,
    pub bar_half: &'static str,
    /// True for the fine (eighth-cell) ramp, false for half-cell bars.
    pub octave: bool,
    pub progress_line: &'static str,
    pub progress_marker: &'static str,
    pub row_marker: &'static str,
    pub ellipsis: &'static str,
    pub play: &'static str,
    pub pause: &'static str,
    pub stop: &'static str,
    pub loading: &'static str,
    pub finished: &'static str,
    pub error: &'static str,
    pub shuffle_on: &'static str,
    pub shuffle_off: &'static str,
    pub repeat_off: &'static str,
    pub repeat_all: &'static str,
    pub repeat_one: &'static str,
    pub volume_full: &'static str,
    pub volume_empty: &'static str,
    /// Whether this is the reduced CP437 set.
    pub basic: bool,
}

impl GlyphSet {
    /// Resolves the configured `mode` (`auto` | `full` | `basic`) together with
    /// the detected terminal capabilities.
    pub fn resolve(mode: &str, windows_terminal: bool, truecolor: bool) -> Self {
        match mode.trim().to_ascii_lowercase().as_str() {
            "full" => full(),
            "basic" => basic(),
            _ => {
                if windows_terminal && truecolor {
                    full()
                } else {
                    basic()
                }
            }
        }
    }

    /// Status glyph for the given playback state.
    pub fn status(&self, status: crate::player::state::PlaybackStatus) -> &'static str {
        use crate::player::state::PlaybackStatus;
        match status {
            PlaybackStatus::Loading => self.loading,
            PlaybackStatus::Playing => self.play,
            PlaybackStatus::Paused => self.pause,
            PlaybackStatus::Stopped => self.stop,
            PlaybackStatus::Finished => self.finished,
            PlaybackStatus::Error => self.error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl GlyphSet {
        fn all_strings(&self) -> Vec<&'static str> {
            let mut all = vec![
                self.bar_full,
                self.bar_half,
                self.progress_line,
                self.progress_marker,
                self.row_marker,
                self.ellipsis,
                self.play,
                self.pause,
                self.stop,
                self.loading,
                self.finished,
                self.error,
                self.shuffle_on,
                self.shuffle_off,
                self.repeat_off,
                self.repeat_all,
                self.repeat_one,
                self.volume_full,
                self.volume_empty,
            ];
            all.extend(self.blocks.iter().copied());
            all
        }
    }

    fn is_cp437(ch: char) -> bool {
        ch.is_ascii() || matches!(ch, '█' | '▄' | '▀' | '░')
    }

    #[test]
    fn basic_uses_only_cp437_characters() {
        let glyphs = basic();
        assert!(glyphs.basic);
        for text in glyphs.all_strings() {
            for ch in text.chars() {
                assert!(is_cp437(ch), "basic glyph {text:?} has non-CP437 {ch:?}");
            }
        }
    }

    #[test]
    fn basic_never_uses_the_octave_ramp() {
        let glyphs = basic();
        assert!(!glyphs.octave);
        for block in ["▁", "▂", "▃", "▅", "▆", "▇"] {
            assert!(!glyphs.blocks.contains(&block));
        }
    }

    #[test]
    fn resolve_honours_the_mode_and_terminal() {
        assert!(!GlyphSet::resolve("full", false, false).basic);
        assert!(GlyphSet::resolve("basic", true, true).basic);
        assert!(GlyphSet::resolve("auto", false, false).basic);
        assert!(!GlyphSet::resolve("auto", true, true).basic);
        assert!(GlyphSet::resolve("unknown", false, false).basic);
    }
}
