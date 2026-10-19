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
    action: fn(&crate::i18n::Text) -> &'static str,
}

const DEFAULT: &[Binding] = &[
    Binding { keys: "Space", action: |t| t.kp_play_pause },
    Binding { keys: "N/P", action: |t| t.kp_track },
    Binding { keys: "\u{2190}/\u{2192}", action: |t| t.kp_seek },
    Binding { keys: "+/-", action: |t| t.kp_volume },
    Binding { keys: "M", action: |t| t.kp_mute },
    Binding { keys: "1-4", action: |t| t.kp_views },
    Binding { keys: "/", action: |t| t.kp_search },
    Binding { keys: "S", action: |t| t.kp_shuffle },
    Binding { keys: "R", action: |t| t.kp_repeat },
    Binding { keys: "V", action: |t| t.kp_bars },
    Binding { keys: "Esc", action: |t| t.kp_menu },
    Binding { keys: "D", action: |t| t.kp_debug },
    Binding { keys: "Q", action: |t| t.kp_quit },
];

const VIM_EXTRA: &[Binding] = &[
    Binding { keys: "H", action: |t| t.kp_seek_back },
    Binding { keys: "J/K", action: |t| t.kp_move },
];

/// Builds the bottom help line from the active keymap and language.
pub fn help_line(preset: KeymapPreset, text: &crate::i18n::Text) -> String {
    let mut parts: Vec<String> = DEFAULT
        .iter()
        .map(|binding| format!("[{}] {}", binding.keys, (binding.action)(text)))
        .collect();
    if preset == KeymapPreset::Vim {
        parts.extend(
            VIM_EXTRA
                .iter()
                .map(|binding| format!("[{}] {}", binding.keys, (binding.action)(text))),
        );
    }
    parts.join("  ")
}

#[cfg(test)]
mod tests {
    use super::{KeymapPreset, help_line};
    use crate::i18n::Text;

    #[test]
    fn the_help_line_mentions_core_keys() {
        let help = help_line(KeymapPreset::Default, &Text::en());
        for expected in ["Space", "Esc", "1-4", "Volume", "Quit"] {
            assert!(help.contains(expected), "missing {expected} in {help}");
        }
    }

    #[test]
    fn the_help_line_follows_the_language() {
        let es = help_line(KeymapPreset::Default, &Text::es());
        assert!(es.contains("Volumen"), "missing Volumen in {es}");
        assert!(es.contains("Salir"), "missing Salir in {es}");
    }

    #[test]
    fn the_vim_preset_adds_its_keys() {
        let help = help_line(KeymapPreset::Vim, &Text::en());
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
