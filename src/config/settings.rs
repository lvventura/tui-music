use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::errors::AppResult;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub volume: u8,
    pub database_path: String,
    #[serde(default = "default_yt_dlp_path")]
    pub yt_dlp_path: String,
    #[serde(default)]
    pub auth: AuthSettings,
    #[serde(default = "default_seek_step")]
    pub seek_step: u64,
    #[serde(default = "default_large_seek_step")]
    pub large_seek_step: u64,
    #[serde(default)]
    pub cache: CacheSettings,
    #[serde(default)]
    pub network: NetworkSettings,
    #[serde(default)]
    pub dj: DJSettings,
    #[serde(default)]
    pub ai: AISettings,
}

#[derive(Debug, Deserialize)]
pub struct AuthSettings {
    #[serde(default)]
    pub browser: String,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            browser: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AISettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    #[serde(default = "default_ai_model")]
    pub model: String,
    #[serde(default = "default_ai_url")]
    pub base_url: String,
    #[serde(default = "default_ai_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_ai_turns")]
    pub max_conversation_turns: usize,
}

impl Default for AISettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_ai_provider(),
            model: default_ai_model(),
            base_url: default_ai_url(),
            timeout_seconds: default_ai_timeout(),
            max_conversation_turns: default_ai_turns(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DJSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_queue_enabled: bool,
    #[serde(default = "default_auto_queue_size")]
    pub auto_queue_size: usize,
    #[serde(default = "default_minimum_queue_size")]
    pub minimum_queue_size: usize,
    #[serde(default = "default_exploration_ratio")]
    pub exploration_ratio: f32,
    #[serde(default = "default_favorite_bonus")]
    pub favorite_bonus: f32,
    #[serde(default = "default_history_weight")]
    pub history_weight: f32,
    #[serde(default = "default_diversity_weight")]
    pub diversity_weight: f32,
    #[serde(default = "default_recent_track_penalty")]
    pub recent_track_penalty: f32,
    #[serde(default = "default_recent_artist_penalty")]
    pub recent_artist_penalty: f32,
    #[serde(default = "default_strategy")]
    pub default_strategy: String,
}

impl Default for DJSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_queue_enabled: false,
            auto_queue_size: 5,
            minimum_queue_size: 2,
            exploration_ratio: 0.2,
            favorite_bonus: 4.0,
            history_weight: 1.0,
            diversity_weight: 2.0,
            recent_track_penalty: 8.0,
            recent_artist_penalty: 3.0,
            default_strategy: "balanced".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CacheSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_cache_capacity")]
    pub memory_capacity: usize,
    #[serde(default = "default_search_ttl")]
    pub search_ttl_seconds: u64,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            memory_capacity: 100,
            search_ttl_seconds: 300,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct NetworkSettings {
    #[serde(default = "default_timeout")]
    pub request_timeout_seconds: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay")]
    pub initial_retry_delay_ms: u64,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            request_timeout_seconds: 10,
            max_retries: 2,
            initial_retry_delay_ms: 300,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_cache_capacity() -> usize {
    100
}
fn default_search_ttl() -> u64 {
    300
}
fn default_timeout() -> u64 {
    10
}
fn default_retries() -> u32 {
    2
}
fn default_retry_delay() -> u64 {
    300
}
fn default_auto_queue_size() -> usize {
    5
}
fn default_minimum_queue_size() -> usize {
    2
}
fn default_exploration_ratio() -> f32 {
    0.2
}
fn default_favorite_bonus() -> f32 {
    4.0
}
fn default_history_weight() -> f32 {
    1.0
}
fn default_diversity_weight() -> f32 {
    2.0
}
fn default_recent_track_penalty() -> f32 {
    8.0
}
fn default_recent_artist_penalty() -> f32 {
    3.0
}
fn default_strategy() -> String {
    "balanced".into()
}
fn default_ai_provider() -> String {
    "ollama".into()
}
fn default_ai_model() -> String {
    "llama3.2".into()
}
fn default_ai_url() -> String {
    "http://localhost:11434".into()
}
fn default_ai_timeout() -> u64 {
    30
}
fn default_ai_turns() -> usize {
    10
}

fn default_seek_step() -> u64 {
    5
}
fn default_large_seek_step() -> u64 {
    15
}

fn default_yt_dlp_path() -> String {
    "yt-dlp".into()
}

impl Settings {
    pub fn load(path: impl AsRef<Path>) -> AppResult<Self> {
        Ok(toml::from_str(&fs::read_to_string(path)?)?)
    }

    pub fn database_path(&self) -> PathBuf {
        if !self.database_path.is_empty() {
            return PathBuf::from(&self.database_path);
        }
        if cfg!(windows) {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("yt-dj")
                .join("yt-dj.db")
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
                })
                .unwrap_or_else(|| PathBuf::from("."))
                .join("yt-dj")
                .join("yt-dj.db")
        }
    }

    pub fn initial_volume(&self, database: &crate::database::Database) -> u8 {
        crate::database::repositories::preferences::get(database.connection(), "volume")
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
            .unwrap_or(self.volume)
            .min(100)
    }
}
