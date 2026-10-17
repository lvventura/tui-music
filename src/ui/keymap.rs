#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeymapPreset {
    Default,
    Vim,
}

impl KeymapPreset {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "vim" => Self::Vim,
            _ => Self::Default,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Vim => "vim",
        }
    }
}

struct Binding {
    keys: &'static str,
    action: &'static str,
}

const DEFAULT: &[Binding] = &[
    Binding { keys: "Space", action: "Play/Pause" },
    Binding { keys: "N/P", action: "Track" },
    Binding { keys: "\u{2190}/\u{2192}", action: "Seek" },
    Binding { keys: "+/-", action: "Volume" },
    Binding { keys: "M", action: "Mute" },
    Binding { keys: "1-4", action: "Views" },
    Binding { keys: "/", action: "Search" },
    Binding { keys: "S", action: "Shuffle" },
    Binding { keys: "R", action: "Repeat" },
    Binding { keys: "Esc", action: "Menu" },
    Binding { keys: "D", action: "Debug" },
    Binding { keys: "Q", action: "Quit" },
];

const VIM_EXTRA: &[Binding] = &[
    Binding { keys: "H", action: "Seek back" },
    Binding { keys: "J/K", action: "Move" },
];

/// Builds the bottom help line from the active keymap.
pub fn help_line(preset: KeymapPreset) -> String {
    let mut parts: Vec<String> = DEFAULT
        .iter()
        .map(|binding| format!("[{}] {}", binding.keys, binding.action))
        .collect();
    if preset == KeymapPreset::Vim {
        parts.extend(
            VIM_EXTRA
                .iter()
                .map(|binding| format!("[{}] {}", binding.keys, binding.action)),
        );
    }
    parts.join("  ")
}

#[cfg(test)]
mod tests {
    use super::{KeymapPreset, help_line};

    #[test]
    fn the_help_line_mentions_core_keys() {
        let help = help_line(KeymapPreset::Default);
        for expected in ["Space", "Esc", "1-4", "Volume", "Quit"] {
            assert!(help.contains(expected), "missing {expected} in {help}");
        }
    }

    #[test]
    fn the_vim_preset_adds_its_keys() {
        let help = help_line(KeymapPreset::Vim);
        assert!(help.contains("[H] Seek back"));
        assert!(help.contains("J/K"));
    }

    #[test]
    fn presets_parse_from_config() {
        assert_eq!(KeymapPreset::parse("vim"), KeymapPreset::Vim);
        assert_eq!(KeymapPreset::parse("  VIM "), KeymapPreset::Vim);
        assert_eq!(KeymapPreset::parse("default"), KeymapPreset::Default);
        assert_eq!(KeymapPreset::parse(""), KeymapPreset::Default);
    }
}
