use std::{fs, path::Path};

use ratatui::style::Color;
use serde::Deserialize;

pub const MIN_CONTRAST: f32 = 4.5;
pub const DEFAULT_THEME: &str = "cover";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub dark: Color,
    pub mid: Color,
    pub bright: Color,
}

#[derive(Deserialize)]
struct ThemeFile {
    name: Option<String>,
    dark: String,
    mid: String,
    bright: String,
}

const BUILT_INS: &[(&str, u32, u32, u32)] = &[
    ("gruvbox", 0x282828, 0xa89984, 0xfabd2f),
    ("nord", 0x2e3440, 0xd8dee9, 0x88c0d0),
    ("dracula", 0x282a36, 0xf8f8f2, 0xbd93f9),
    ("catppuccin", 0x1e1e2e, 0xcdd6f4, 0x89b4fa),
    ("tokyonight", 0x1a1b26, 0xc0caf5, 0x7aa2f7),
    ("everforest", 0x2d353b, 0xd3c6aa, 0xa7c080),
    ("rosepine", 0x191724, 0xe0def4, 0xebbcba),
    ("solarized", 0x002b36, 0x93a1a1, 0xb58900),
    ("matrix", 0x000000, 0x00b000, 0x00ff00),
    ("mono", 0x000000, 0xc0c0c0, 0xffffff),
];

impl Theme {
    pub fn builtin(name: &str) -> Option<Self> {
        BUILT_INS
            .iter()
            .find(|(candidate, ..)| *candidate == name)
            .map(|(name, dark, mid, bright)| Self {
                name: (*name).to_owned(),
                dark: rgb_from_u32(*dark),
                mid: rgb_from_u32(*mid),
                bright: rgb_from_u32(*bright),
            })
            .map(Self::with_contrast_floor)
    }

    pub fn names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = BUILT_INS.iter().map(|(name, ..)| *name).collect();
        names.push(DEFAULT_THEME);
        names
    }

    /// Deterministic palette derived from an image seed (used before/without a
    /// decoded cover).
    pub fn from_seed(seed: &str) -> Self {
        let hash = fnv1a(seed.as_bytes());
        let hue = (hash % 360) as f32;
        let base = hsv_to_rgb(hue, 0.45, 0.55);
        Self::from_cover(base)
    }

    /// Palette derived from an average cover colour.
    pub fn from_cover(base: (u8, u8, u8)) -> Self {
        let dark = scale(base, 0.22);
        let mid = scale(base, 1.0);
        let bright = scale(base, 1.9);
        Self {
            name: DEFAULT_THEME.to_owned(),
            dark: Color::Rgb(dark.0, dark.1, dark.2),
            mid: Color::Rgb(mid.0, mid.1, mid.2),
            bright: Color::Rgb(bright.0, bright.1, bright.2),
        }
        .with_contrast_floor()
    }

    pub fn from_toml(contents: &str) -> Option<Self> {
        let file: ThemeFile = toml::from_str(contents).ok()?;
        let theme = Self {
            name: file.name.unwrap_or_else(|| "user".into()),
            dark: parse_hex(&file.dark)?,
            mid: parse_hex(&file.mid)?,
            bright: parse_hex(&file.bright)?,
        };
        Some(theme.with_contrast_floor())
    }

    /// Loads `<dir>/<name>.toml`, returning `None` when it does not exist.
    pub fn load_user(dir: &Path, name: &str) -> Option<Self> {
        let path = dir.join(format!("{name}.toml"));
        let contents = fs::read_to_string(path).ok()?;
        Self::from_toml(&contents)
    }

    /// Resolves a theme by name: built-in, then user themes, then cover seed.
    pub fn resolve(name: &str, seed: &str, user_dir: &Path) -> Self {
        let requested = if name.trim().is_empty() {
            DEFAULT_THEME
        } else {
            name.trim()
        };
        if requested == DEFAULT_THEME {
            return Self::from_seed(seed);
        }
        Self::builtin(requested)
            .or_else(|| Self::load_user(user_dir, requested))
            .unwrap_or_else(|| Self::from_seed(seed))
    }

    /// Raises the text colours until they clear the contrast floor against the
    /// background, so a very dark cover palette stays readable.
    pub fn with_contrast_floor(mut self) -> Self {
        for _ in 0..32 {
            if contrast(self.dark, self.mid) >= MIN_CONTRAST
                && contrast(self.dark, self.bright) >= MIN_CONTRAST
            {
                break;
            }
            self.mid = lighten(self.mid, 0.12);
            self.bright = lighten(self.bright, 0.12);
        }
        self
    }

    /// Converts RGB colours to the 256-colour cube for terminals without
    /// truecolor support.
    pub fn degrade_to_256(&mut self) {
        self.dark = to_indexed(self.dark);
        self.mid = to_indexed(self.mid);
        self.bright = to_indexed(self.bright);
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_seed("yt-dj")
    }
}

fn rgb_from_u32(value: u32) -> Color {
    Color::Rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

fn parse_hex(value: &str) -> Option<Color> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    let parsed = u32::from_str_radix(value, 16).ok()?;
    Some(rgb_from_u32(parsed))
}

/// Linear interpolation between two colours (`t` clamped to `0.0..=1.0`).
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = to_rgb(a);
    let (br, bg, bb) = to_rgb(b);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

fn to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        _ => (128, 128, 128),
    }
}

fn fnv1a(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (u8, u8, u8) {
    let c = value * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - c;
    let (r, g, b) = match hue as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

fn scale((r, g, b): (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    (
        ((r as f32 * factor).clamp(0.0, 255.0)) as u8,
        ((g as f32 * factor).clamp(0.0, 255.0)) as u8,
        ((b as f32 * factor).clamp(0.0, 255.0)) as u8,
    )
}

fn lighten(color: Color, amount: f32) -> Color {
    let (r, g, b) = to_rgb(color);
    let lift = |channel: u8| {
        let value = channel as f32 + (255.0 - channel as f32) * amount;
        value.round().clamp(0.0, 255.0) as u8
    };
    Color::Rgb(lift(r), lift(g), lift(b))
}

fn channel_luminance(channel: u8) -> f32 {
    let value = channel as f32 / 255.0;
    if value <= 0.03928 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(color: Color) -> f32 {
    let (r, g, b) = to_rgb(color);
    0.2126 * channel_luminance(r) + 0.7152 * channel_luminance(g) + 0.0722 * channel_luminance(b)
}

pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn to_indexed(color: Color) -> Color {
    let (r, g, b) = to_rgb(color);
    if r == g && g == b {
        if r < 8 {
            return Color::Indexed(16);
        }
        if r > 248 {
            return Color::Indexed(231);
        }
        let grey = ((r as f32 - 8.0) / 247.0 * 24.0).round() as u8;
        return Color::Indexed(232 + grey.min(23));
    }
    let quant = |channel: u8| ((channel as f32 / 255.0 * 5.0).round()) as u8;
    Color::Indexed(16 + 36 * quant(r) + 6 * quant(g) + quant(b))
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_THEME, MIN_CONTRAST, Theme, contrast};
    use std::path::Path;

    #[test]
    fn built_in_themes_clear_the_contrast_floor() {
        for name in Theme::names() {
            let theme = if name == DEFAULT_THEME {
                Theme::from_seed("seed")
            } else {
                Theme::builtin(name).expect("built-in theme")
            };
            assert!(
                contrast(theme.dark, theme.mid) >= MIN_CONTRAST,
                "{name} mid contrast too low"
            );
            assert!(
                contrast(theme.dark, theme.bright) >= MIN_CONTRAST,
                "{name} bright contrast too low"
            );
        }
    }

    #[test]
    fn a_dark_cover_palette_is_lifted_to_the_floor() {
        let theme = Theme::from_cover((4, 4, 6));
        assert!(contrast(theme.dark, theme.mid) >= MIN_CONTRAST);
        assert!(contrast(theme.dark, theme.bright) >= MIN_CONTRAST);
    }

    #[test]
    fn cover_palette_is_deterministic() {
        assert_eq!(Theme::from_seed("abc"), Theme::from_seed("abc"));
        assert_ne!(Theme::from_seed("abc"), Theme::from_seed("xyz"));
    }

    #[test]
    fn user_themes_parse_from_toml() {
        let theme = Theme::from_toml(
            "name = \"mine\"\ndark = \"#101010\"\nmid = \"#808080\"\nbright = \"#ffcc00\"\n",
        )
        .expect("valid theme");
        assert_eq!(theme.name, "mine");
    }

    #[test]
    fn malformed_user_themes_are_rejected() {
        assert!(Theme::from_toml("dark = \"nope\"").is_none());
        assert!(Theme::load_user(Path::new("does-not-exist"), "x").is_none());
    }

    #[test]
    fn resolve_falls_back_to_cover_for_unknown_names() {
        let theme = Theme::resolve("no-such-theme", "seed", Path::new("missing"));
        assert_eq!(theme.name, DEFAULT_THEME);
    }

    #[test]
    fn degrade_to_256_produces_indexed_colours() {
        let mut theme = Theme::builtin("gruvbox").expect("built-in");
        theme.degrade_to_256();
        assert!(matches!(theme.dark, ratatui::style::Color::Indexed(_)));
        assert!(matches!(theme.bright, ratatui::style::Color::Indexed(_)));
    }
}
