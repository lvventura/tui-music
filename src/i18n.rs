//! Internationalisation. A [`Text`] catalogue of every user-visible string,
//! selected by [`Language`]. Spanish is the default; English is selectable via
//! `[ui] language` or the `Esc` menu. The catalogue is `Copy` so it can be
//! threaded through the render code like [`crate::ui::glyphs::GlyphSet`].

/// Selectable UI language.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    Es,
    En,
}

impl Language {
    pub fn parse(value: &str) -> Self {
        let value = value.trim().to_ascii_lowercase();
        if value == "en"
            || value.starts_with("en-")
            || value.starts_with("en_")
            || value.starts_with("english")
        {
            Language::En
        } else {
            Language::Es
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Language::Es => "es",
            Language::En => "en",
        }
    }

    /// Name shown in the menu.
    pub fn label(self) -> &'static str {
        match self {
            Language::Es => "Español",
            Language::En => "English",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Language::Es => Language::En,
            Language::En => Language::Es,
        }
    }
}

/// Replaces each `{}` placeholder with the given arguments, in order.
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = template.to_string();
    for arg in args {
        if let Some(position) = out.find("{}") {
            out.replace_range(position..position + 2, arg);
        }
    }
    out
}

/// The whole catalogue of UI strings for one language.
#[derive(Clone, Copy, Debug)]
pub struct Text {
    // Brand
    pub app_name: &'static str,
    // Views / tabs
    pub view_queue: &'static str,
    pub view_library: &'static str,
    pub view_track: &'static str,
    pub view_search: &'static str,
    // Panels
    pub panel_queue: &'static str,
    pub panel_search: &'static str,
    pub panel_player: &'static str,
    pub panel_now_playing: &'static str,
    pub panel_up_next: &'static str,
    pub library_columns: [&'static str; 3],
    pub screen_history: &'static str,
    pub screen_favorites: &'static str,
    pub screen_playlists: &'static str,
    pub menu_title: &'static str,
    // Search
    pub searching: &'static str,
    pub search_prompt: &'static str,
    // Track
    pub nothing_playing: &'static str,
    pub no_track_selected: &'static str,
    // Status messages
    pub results_fmt: &'static str,
    pub audio_ready: &'static str,
    pub mix_ready: &'static str,
    pub session_authenticated: &'static str,
    pub account_signed_in: &'static str,
    pub account_signed_out: &'static str,
    pub loading_next: &'static str,
    pub playback_error_fmt: &'static str,
    pub audio_error_fmt: &'static str,
    pub session_hint: &'static str,
    pub no_recommendations: &'static str,
    pub dj_added_fmt: &'static str,
    pub dj_none: &'static str,
    pub dj_searching: &'static str,
    pub already_authenticated: &'static str,
    pub validating: &'static str,
    pub validating_cookie: &'static str,
    pub authenticated_browser: &'static str,
    pub browser_opened: &'static str,
    pub auth_failed_fmt: &'static str,
    pub no_previous: &'static str,
    pub ytdlp_missing_fmt: &'static str,
    pub volume_restored: &'static str,
    // Menu
    pub menu_labels: [&'static str; 7],
    pub value_on: &'static str,
    pub value_off: &'static str,
    pub muted: &'static str,
    pub repeat_off_value: &'static str,
    pub repeat_all_value: &'static str,
    pub repeat_one_value: &'static str,
    pub menu_help: &'static str,
    // Keymap actions
    pub kp_play_pause: &'static str,
    pub kp_track: &'static str,
    pub kp_seek: &'static str,
    pub kp_volume: &'static str,
    pub kp_mute: &'static str,
    pub kp_views: &'static str,
    pub kp_search: &'static str,
    pub kp_shuffle: &'static str,
    pub kp_repeat: &'static str,
    pub kp_bars: &'static str,
    pub kp_menu: &'static str,
    pub kp_debug: &'static str,
    pub kp_quit: &'static str,
    pub kp_seek_back: &'static str,
    pub kp_move: &'static str,
    // Terminal
    pub term_note: &'static str,
    // CLI (--auth / --diagnose / --help)
    pub cli_store_ok_fmt: &'static str,
    pub cli_paste_prompt: &'static str,
    pub cli_creds_heading: &'static str,
    pub cli_present: &'static str,
    pub cli_absent: &'static str,
    pub cli_library_label: &'static str,
    pub cli_library_ok: &'static str,
    pub cli_library_empty: &'static str,
    pub cli_search_label: &'static str,
    pub cli_search_ok_fmt: &'static str,
    pub cli_ytdlp_unavailable: &'static str,
}

impl Text {
    pub fn new(lang: Language) -> Self {
        match lang {
            Language::Es => Self::es(),
            Language::En => Self::en(),
        }
    }

    pub fn es() -> Self {
        Self {
            app_name: "Lite|Music",
            view_queue: "Cola",
            view_library: "Biblioteca",
            view_track: "Pista",
            view_search: "Buscar",
            panel_queue: "Cola",
            panel_search: "Buscar",
            panel_player: "Reproductor",
            panel_now_playing: " reproduciendo ",
            panel_up_next: " a continuación ",
            library_columns: ["Biblioteca", "Elementos", "Canciones"],
            screen_history: "Historial",
            screen_favorites: "Favoritos",
            screen_playlists: "Listas",
            menu_title: " Menú ",
            searching: "Buscando…",
            search_prompt: "Pulsa / para buscar",
            nothing_playing: "Sin reproducción",
            no_track_selected: "Sin canción seleccionada",
            results_fmt: "{} resultados",
            audio_ready: "Audio listo",
            mix_ready: "Mix listo",
            session_authenticated: "Sesión autenticada",
            account_signed_in: "Con sesión",
            account_signed_out: "Sin sesión",
            loading_next: "Cargando siguiente canción…",
            playback_error_fmt: "Error de reproducción: {}",
            audio_error_fmt: "No se pudo obtener el audio: {}{}",
            session_hint: " · YouTube pide sesión: copia la cookie (DevTools > Network) y ejecuta `lite-music --auth clipboard`",
            no_recommendations: "No se encontraron recomendaciones",
            dj_added_fmt: "DJ: {} recomendaciones añadidas a la cola",
            dj_none: "DJ: sin nuevas recomendaciones",
            dj_searching: "DJ: buscando recomendaciones…",
            already_authenticated: "Sesión ya autenticada",
            validating: "Validación en curso…",
            validating_cookie: "Validando cookie guardada…",
            authenticated_browser: "Sesión autenticada (navegador)",
            browser_opened: "Navegador abierto; vuelve a pulsar L para verificar. Alternativa: `lite-music --auth cookie|browser`",
            auth_failed_fmt: "{}. Usa `lite-music --auth cookie` o `--auth browser`",
            no_previous: "No hay canción anterior en la cola",
            ytdlp_missing_fmt: "yt-dlp no encontrado o no ejecutable: {}",
            volume_restored: "El volumen guardado era 0; restaurado a un valor audible",
            menu_labels: ["Tema", "Volumen", "Aleatorio", "Repetir", "Teclas", "Idioma", "Salir"],
            value_on: "SÍ",
            value_off: "NO",
            muted: "SILENCIO",
            repeat_off_value: "No",
            repeat_all_value: "Cola",
            repeat_one_value: "Canción",
            menu_help: "Esc cerrar   ←/→ cambiar   ↑/↓ mover",
            kp_play_pause: "Reproducir/Pausar",
            kp_track: "Pista",
            kp_seek: "Desplazar",
            kp_volume: "Volumen",
            kp_mute: "Silenciar",
            kp_views: "Vistas",
            kp_search: "Buscar",
            kp_shuffle: "Aleatorio",
            kp_repeat: "Repetir",
            kp_bars: "Barras",
            kp_menu: "Menú",
            kp_debug: "Depurar",
            kp_quit: "Salir",
            kp_seek_back: "Retroceder",
            kp_move: "Mover",
            term_note: "Terminal sin truecolor: se usan 256 colores. Recomendado: Windows Terminal",
            cli_store_ok_fmt: "Sesión validada. Cookie guardada en {}",
            cli_paste_prompt:
                "Pega la cabecera Cookie de una petición a music.youtube.com (DevTools > Network):",
            cli_creds_heading: "credenciales y rutas:",
            cli_present: "presente",
            cli_absent: "ausente ",
            cli_library_label: "biblioteca (Liked Music)",
            cli_library_ok: "accesible",
            cli_library_empty: "sin cookie guardada",
            cli_search_label: "búsqueda (ytsearch)",
            cli_search_ok_fmt: "{} resultados",
            cli_ytdlp_unavailable: "no disponible",
        }
    }

    pub fn en() -> Self {
        Self {
            app_name: "Lite|Music",
            view_queue: "Queue",
            view_library: "Library",
            view_track: "Track",
            view_search: "Search",
            panel_queue: "Queue",
            panel_search: "Search",
            panel_player: "Player",
            panel_now_playing: " now playing ",
            panel_up_next: " up next ",
            library_columns: ["Library", "Items", "Songs"],
            screen_history: "History",
            screen_favorites: "Favorites",
            screen_playlists: "Playlists",
            menu_title: " Menu ",
            searching: "Searching…",
            search_prompt: "Press / to search",
            nothing_playing: "Nothing playing",
            no_track_selected: "No track selected",
            results_fmt: "{} results",
            audio_ready: "Audio ready",
            mix_ready: "Mix ready",
            session_authenticated: "Session authenticated",
            account_signed_in: "Signed in",
            account_signed_out: "Signed out",
            loading_next: "Loading next track…",
            playback_error_fmt: "Playback error: {}",
            audio_error_fmt: "Could not fetch audio: {}{}",
            session_hint: " · YouTube wants a session: copy the cookie (DevTools > Network) and run `lite-music --auth clipboard`",
            no_recommendations: "No recommendations found",
            dj_added_fmt: "DJ: {} recommendations added to the queue",
            dj_none: "DJ: no new recommendations",
            dj_searching: "DJ: looking for recommendations…",
            already_authenticated: "Session already authenticated",
            validating: "Validating…",
            validating_cookie: "Validating stored cookie…",
            authenticated_browser: "Session authenticated (browser)",
            browser_opened: "Browser opened; press L again to verify. Alternative: `lite-music --auth cookie|browser`",
            auth_failed_fmt: "{}. Use `lite-music --auth cookie` or `--auth browser`",
            no_previous: "No previous track in the queue",
            ytdlp_missing_fmt: "yt-dlp not found or not executable: {}",
            volume_restored: "Saved volume was 0; restored to an audible value",
            menu_labels: ["Theme", "Volume", "Shuffle", "Repeat", "Keymap", "Language", "Quit"],
            value_on: "ON",
            value_off: "OFF",
            muted: "MUTED",
            repeat_off_value: "Off",
            repeat_all_value: "Queue",
            repeat_one_value: "Song",
            menu_help: "Esc close   ←/→ change   ↑/↓ move",
            kp_play_pause: "Play/Pause",
            kp_track: "Track",
            kp_seek: "Seek",
            kp_volume: "Volume",
            kp_mute: "Mute",
            kp_views: "Views",
            kp_search: "Search",
            kp_shuffle: "Shuffle",
            kp_repeat: "Repeat",
            kp_bars: "Bars",
            kp_menu: "Menu",
            kp_debug: "Debug",
            kp_quit: "Quit",
            kp_seek_back: "Seek back",
            kp_move: "Move",
            term_note: "No truecolor: using 256 colors. Recommended: Windows Terminal",
            cli_store_ok_fmt: "Session validated. Cookie stored in {}",
            cli_paste_prompt:
                "Paste the Cookie header from a music.youtube.com request (DevTools > Network):",
            cli_creds_heading: "credentials and paths:",
            cli_present: "present",
            cli_absent: "absent ",
            cli_library_label: "library (Liked Music)",
            cli_library_ok: "accessible",
            cli_library_empty: "no stored cookie",
            cli_search_label: "search (ytsearch)",
            cli_search_ok_fmt: "{} results",
            cli_ytdlp_unavailable: "unavailable",
        }
    }

    pub fn results(&self, count: usize) -> String {
        fill(self.results_fmt, &[count.to_string()])
    }

    pub fn dj_added(&self, count: usize) -> String {
        fill(self.dj_added_fmt, &[count.to_string()])
    }

    pub fn playback_error(&self, error: &str) -> String {
        fill(self.playback_error_fmt, &[error.to_string()])
    }

    pub fn audio_error(&self, error: &str, hint: &str) -> String {
        fill(self.audio_error_fmt, &[error.to_string(), hint.to_string()])
    }

    pub fn auth_failed(&self, error: &str) -> String {
        fill(self.auth_failed_fmt, &[error.to_string()])
    }

    pub fn ytdlp_missing(&self, path: &str) -> String {
        fill(self.ytdlp_missing_fmt, &[path.to_string()])
    }
}

impl Default for Text {
    fn default() -> Self {
        Text::es()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_to_spanish() {
        assert_eq!(Language::parse("es"), Language::Es);
        assert_eq!(Language::parse("en"), Language::En);
        assert_eq!(Language::parse("EN-gb"), Language::En);
        assert_eq!(Language::parse(""), Language::Es);
        assert_eq!(Language::parse("desconocido"), Language::Es);
    }

    #[test]
    fn languages_toggle() {
        assert_eq!(Language::Es.next(), Language::En);
        assert_eq!(Language::En.next(), Language::Es);
    }

    #[test]
    fn fill_replaces_placeholders_in_order() {
        assert_eq!(fill("{} + {}", &["1".into(), "2".into()]), "1 + 2");
        assert_eq!(fill("x{}", &["y".into()]), "xy");
    }

    #[test]
    fn catalogues_differ_and_keep_the_brand() {
        let es = Text::es();
        let en = Text::en();
        assert_eq!(es.view_queue, "Cola");
        assert_eq!(en.view_queue, "Queue");
        assert_eq!(es.app_name, "Lite|Music");
        assert_eq!(en.app_name, "Lite|Music");
        assert_eq!(es.menu_labels.len(), en.menu_labels.len());
    }

    #[test]
    fn formatted_messages_use_the_language() {
        assert_eq!(Text::es().results(3), "3 resultados");
        assert_eq!(Text::en().results(3), "3 results");
    }
}
