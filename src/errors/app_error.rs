use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(rusqlite::Error),
    #[error("HTTP error: {0}")]
    Http(reqwest::Error),
    #[error("configuration error: {0}")]
    Config(toml::de::Error),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("player error: {0}")]
    Player(String),
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}
impl From<toml::de::Error> for AppError {
    fn from(error: toml::de::Error) -> Self {
        Self::Config(error)
    }
}
