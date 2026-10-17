use std::path::{Path, PathBuf};

pub const APP_DIR: &str = "yt-dj";

pub fn app_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

pub fn config_file() -> PathBuf {
    app_dir().join("config.toml")
}

pub fn state_file() -> PathBuf {
    app_dir().join("state.toml")
}

pub fn cookie_file() -> PathBuf {
    app_dir().join("cookie.txt")
}

pub fn cookies_export_file() -> PathBuf {
    app_dir().join("cookies.txt")
}

pub fn themes_dir() -> PathBuf {
    app_dir().join("themes")
}

pub fn log_dir() -> PathBuf {
    app_dir().join("logs")
}

pub fn log_file() -> PathBuf {
    log_dir().join("yt-dj.log")
}

pub fn database_file() -> PathBuf {
    app_dir().join("yt-dj.db")
}

pub fn ensure_parent(path: &Path) -> std::io::Result<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => std::fs::create_dir_all(parent),
        _ => Ok(()),
    }
}
