#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalCaps {
    pub truecolor: bool,
    pub windows_terminal: bool,
    pub image: ImageProtocol,
}

/// How cover art can be painted in the current terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Kitty (and Ghostty) graphics protocol: crisp, pixel-perfect.
    Kitty,
    /// Sixel: widely supported by pixel-capable terminals.
    Sixel,
    /// Truecolor upper/lower half blocks: universal fallback.
    HalfBlock,
    /// No colour support at all: covers are skipped.
    None,
}

impl TerminalCaps {
    pub fn detect() -> Self {
        let windows_terminal = std::env::var_os("WT_SESSION").is_some();
        let truecolor = windows_terminal || colorterm_reports_truecolor();
        Self {
            truecolor,
            windows_terminal,
            image: detect_image_protocol(truecolor),
        }
    }
}

fn colorterm_reports_truecolor() -> bool {
    std::env::var_os("COLORTERM")
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .map(|value| value.contains("truecolor") || value.contains("24bit"))
        .unwrap_or(false)
}

fn env_lower(key: &str) -> String {
    std::env::var(key)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default()
}

fn detect_image_protocol(truecolor: bool) -> ImageProtocol {
    classify_image(
        &env_lower("TERM"),
        &env_lower("TERM_PROGRAM"),
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
        std::env::var_os("WEZTERM_EXECUTABLE").is_some(),
        truecolor,
        &env_lower("YT_DJ_IMAGE_PROTOCOL"),
    )
}

/// Pure classification so it can be unit-tested without touching the real env.
fn classify_image(
    term: &str,
    program: &str,
    kitty_window: bool,
    wezterm_env: bool,
    truecolor: bool,
    override_protocol: &str,
) -> ImageProtocol {
    match override_protocol {
        "kitty" => return ImageProtocol::Kitty,
        "sixel" => return ImageProtocol::Sixel,
        "none" => return ImageProtocol::None,
        "halfblock" | "half" => {
            return if truecolor {
                ImageProtocol::HalfBlock
            } else {
                ImageProtocol::None
            };
        }
        _ => {}
    }

    let kitty = kitty_window
        || term.contains("kitty")
        || term.contains("ghostty")
        || program == "ghostty";
    if kitty {
        return ImageProtocol::Kitty;
    }

    let sixel = term.contains("sixel")
        || term.contains("foot")
        || term.contains("mlterm")
        || matches!(program, "wezterm" | "mintty" | "mlterm" | "contour")
        || wezterm_env;
    if sixel {
        return ImageProtocol::Sixel;
    }

    if truecolor {
        ImageProtocol::HalfBlock
    } else {
        ImageProtocol::None
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageProtocol, classify_image};

    #[test]
    fn kitty_terminals_are_detected() {
        assert_eq!(
            classify_image("xterm-kitty", "", false, false, true, ""),
            ImageProtocol::Kitty
        );
        assert_eq!(
            classify_image("xterm-256color", "ghostty", false, false, true, ""),
            ImageProtocol::Kitty
        );
    }

    #[test]
    fn sixel_terminals_are_detected() {
        assert_eq!(
            classify_image("xterm-256color", "WezTerm", false, true, true, ""),
            ImageProtocol::Sixel
        );
        assert_eq!(
            classify_image("foot", "", false, false, true, ""),
            ImageProtocol::Sixel
        );
    }

    #[test]
    fn unknown_terminals_fall_back_to_half_blocks() {
        assert_eq!(
            classify_image("xterm-256color", "Windows_Terminal", false, false, true, ""),
            ImageProtocol::HalfBlock
        );
        assert_eq!(
            classify_image("dumb", "", false, false, false, ""),
            ImageProtocol::None
        );
    }

    #[test]
    fn the_override_wins() {
        assert_eq!(
            classify_image("xterm-256color", "", false, false, true, "none"),
            ImageProtocol::None
        );
        assert_eq!(
            classify_image("dumb", "", false, false, false, "kitty"),
            ImageProtocol::Kitty
        );
    }
}
