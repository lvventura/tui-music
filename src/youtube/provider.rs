use std::{
    collections::VecDeque,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::errors::{AppError, AppResult};
use crate::{cache::Cache, config::Settings, metrics::Metrics};

use super::models::Song;

const AUDIO_FORMAT: &str = "bestaudio[ext=m4a]/bestaudio/best";
const AUDIO_CONTAINER: &str = "mp4";
const AUDIO_CODEC: &str = "aac";

pub trait MusicProvider: Send + Sync {
    fn search(
        &self,
        query: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Vec<Song>>> + Send>>;
    fn stream_url(
        &self,
        song: Song,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<AudioStream>> + Send>>;
    fn related(
        &self,
        song: Song,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Vec<Song>>> + Send>>;
}

pub struct AudioStream {
    reader: AudioStreamReader,
    pub content_type: Option<String>,
}

impl AudioStream {
    pub fn reader(self) -> AudioStreamReader {
        self.reader
    }
}

pub struct AudioStreamReader {
    receiver: std::sync::Mutex<tokio::sync::mpsc::Receiver<Result<Option<Vec<u8>>, String>>>,
    pending: VecDeque<u8>,
    finished: bool,
}

impl Read for AudioStreamReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        while self.pending.is_empty() && !self.finished {
            let message = self
                .receiver
                .lock()
                .map_err(|_| io::Error::other("audio stream lock poisoned"))?
                .blocking_recv()
                .ok_or_else(|| {
                    tracing::warn!("[STREAM] channel closed before EOF marker");
                    io::Error::new(io::ErrorKind::UnexpectedEof, "audio stream ended")
                })?;
            match message {
                Ok(Some(chunk)) => self.pending.extend(chunk),
                Ok(None) => {
                    self.finished = true;
                    tracing::debug!("[STREAM] reader reached EOF");
                }
                Err(error) => {
                    self.finished = true;
                    tracing::warn!(error = %error, "[STREAM] reader received body error");
                    return Err(io::Error::other(error));
                }
            }
        }
        if self.pending.is_empty() {
            return Ok(0);
        }
        let amount = output.len().min(self.pending.len());
        for byte in output.iter_mut().take(amount) {
            *byte = self.pending.pop_front().expect("pending length checked");
        }
        Ok(amount)
    }
}

impl Seek for AudioStreamReader {
    fn seek(&mut self, _position: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "network audio stream is not seekable",
        ))
    }
}

#[derive(Clone)]
pub struct YtDlpProvider {
    executable: String,
    search_cache: Arc<Mutex<Cache<String, Vec<Song>>>>,
    timeout: Duration,
    resolve_timeout: Duration,
    retries: u32,
    retry_delay: Duration,
    cookies_file: Option<PathBuf>,
    cookies_from_browser: Option<String>,
    pub metrics: Arc<Metrics>,
}

impl YtDlpProvider {
    pub fn new(executable: impl Into<String>, settings: &Settings) -> Self {
        let timeout = Duration::from_secs(settings.network.request_timeout_seconds);
        Self {
            executable: executable.into(),
            search_cache: Arc::new(Mutex::new(Cache::new(
                settings.cache.memory_capacity,
                Duration::from_secs(settings.cache.search_ttl_seconds),
            ))),
            timeout,
            // Format extraction can take noticeably longer than a plain request.
            resolve_timeout: timeout.max(Duration::from_secs(30)),
            retries: settings.network.max_retries,
            retry_delay: Duration::from_millis(settings.network.initial_retry_delay_ms),
            cookies_file: None,
            cookies_from_browser: None,
            metrics: Arc::new(Metrics::default()),
        }
    }

    pub fn with_cookies(mut self, cookies_file: Option<PathBuf>) -> Self {
        self.cookies_file = cookies_file;
        self
    }

    /// Fallback used when no cookie header is stored: let yt-dlp read the
    /// browser's cookies itself (handles Chrome/Edge/Firefox on Windows).
    pub fn with_browser_cookies(mut self, browser: Option<String>) -> Self {
        self.cookies_from_browser = browser.filter(|value| !value.trim().is_empty());
        self
    }

    fn cookie_args(&self) -> Vec<String> {
        if let Some(path) = &self.cookies_file {
            return vec!["--cookies".to_owned(), path.to_string_lossy().into_owned()];
        }
        if let Some(browser) = &self.cookies_from_browser {
            return vec![
                "--cookies-from-browser".to_owned(),
                browser.trim().to_owned(),
            ];
        }
        Vec::new()
    }

    pub async fn available(&self) -> bool {
        tokio::time::timeout(
            self.timeout,
            Command::new(&self.executable)
                .arg("--version")
                .stdin(Stdio::null())
                .output(),
        )
        .await
        .map(|result| {
            result
                .map(|output| output.status.success())
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    async fn command(&self, args: &[&str]) -> AppResult<Vec<u8>> {
        self.command_with_timeout(args, self.timeout, false).await
    }

    async fn command_authed(&self, args: &[&str], timeout: Duration) -> AppResult<Vec<u8>> {
        self.command_with_timeout(args, timeout, true).await
    }

    async fn command_with_timeout(
        &self,
        args: &[&str],
        timeout: Duration,
        with_cookies: bool,
    ) -> AppResult<Vec<u8>> {
        let mut full: Vec<String> = Vec::with_capacity(args.len() + 4);
        if with_cookies {
            full.extend(self.cookie_args());
        }
        full.extend(args.iter().map(|arg| (*arg).to_owned()));
        for attempt in 0..=self.retries {
            self.metrics
                .provider_requests
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let result = tokio::time::timeout(timeout, async {
                Command::new(&self.executable)
                    .args(&full)
                    .stdin(Stdio::null())
                    .output()
                    .await
            })
            .await;
            match result {
                Ok(Ok(output)) if output.status.success() => return Ok(output.stdout),
                Ok(Ok(output)) if attempt == self.retries => {
                    self.metrics
                        .provider_failures
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                    warn!(args = ?args, %stderr, "yt-dlp command failed");
                    return Err(AppError::Provider(stderr));
                }
                Ok(Err(error)) if attempt == self.retries => {
                    warn!(args = ?args, %error, "yt-dlp is not available");
                    return Err(AppError::Provider(format!("yt-dlp no disponible: {error}")));
                }
                Err(_) if attempt == self.retries => {
                    warn!(args = ?args, "yt-dlp command timed out");
                    return Err(AppError::Provider("timeout del proveedor".into()));
                }
                _ => {
                    self.metrics
                        .retries
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    tokio::time::sleep(self.retry_delay * 2u32.pow(attempt)).await;
                }
            }
        }
        Err(AppError::Provider("fallo inesperado del proveedor".into()))
    }

    /// Downloads the audio with yt-dlp into a temporary file and streams that
    /// file into the decoder. yt-dlp is the only reliable way to fetch
    /// googlevideo URLs (it resolves `n` signatures and PO tokens), and some
    /// videos only expose a muxed stream, so extracting to an audio-only m4a
    /// keeps playback robust. The temp file is removed after it is read.
    async fn open_audio_stream(&self, page: &str) -> AppResult<AudioStream> {
        let base = std::env::temp_dir().join(format!(
            "yt-dj-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis())
                .unwrap_or_default()
        ));
        let template = format!("{}.%(ext)s", base.display());

        let mut stderr = String::new();
        let mut path = None;
        // Preferred: extract audio-only m4a (needs ffmpeg, bundled with yt-dlp).
        let (success, error) = self
            .run_download(&[
                "-f",
                AUDIO_FORMAT,
                "-x",
                "--audio-format",
                "m4a",
                "--no-playlist",
                "--no-progress",
                "--no-warnings",
                "-o",
                &template,
                page,
            ])
            .await?;
        if success {
            path = find_download(&base);
        } else {
            warn!(stderr = %error.trim(), "yt-dlp -x falló, reintentando sin extracción");
        }
        // Fallback: plain audio-only download (no ffmpeg needed).
        if path.is_none() {
            let (success, error) = self
                .run_download(&[
                    "-f",
                    "bestaudio[ext=m4a]/bestaudio",
                    "--no-playlist",
                    "--no-progress",
                    "--no-warnings",
                    "-o",
                    &template,
                    page,
                ])
                .await?;
            if success {
                path = find_download(&base);
            } else {
                stderr = error;
            }
        }

        let path = path.ok_or_else(|| {
            AppError::Provider(format!(
                "yt-dlp no generó audio: {}",
                first_line(stderr.trim())
            ))
        })?;
        let bytes = tokio::fs::read(&path).await.map_err(|error| {
            AppError::Provider(format!("no se pudo leer {}: {error}", path.display()))
        })?;
        let _ = tokio::fs::remove_file(&path).await;
        if bytes.is_empty() {
            return Err(AppError::Provider("yt-dlp generó un archivo vacío".into()));
        }
        debug!(bytes = bytes.len(), "[STREAM] audio downloaded");
        let (sender, receiver) = tokio::sync::mpsc::channel(8);
        tokio::spawn(async move {
            for chunk in bytes.chunks(64 * 1024) {
                if sender.send(Ok(Some(chunk.to_vec()))).await.is_err() {
                    return;
                }
            }
            let _ = sender.send(Ok(None)).await;
        });
        Ok(AudioStream {
            reader: AudioStreamReader {
                receiver: std::sync::Mutex::new(receiver),
                pending: VecDeque::new(),
                finished: false,
            },
            content_type: Some("audio/mp4".into()),
        })
    }

    async fn run_download(&self, args: &[&str]) -> AppResult<(bool, String)> {
        let mut command = Command::new(&self.executable);
        command.args(self.cookie_args());
        command.args(args);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let output = tokio::time::timeout(Duration::from_secs(180), command.output())
            .await
            .map_err(|_| AppError::Provider("agotado el tiempo descargando el audio".into()))?
            .map_err(|error| AppError::Provider(format!("no se pudo iniciar yt-dlp: {error}")))?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((output.status.success(), stderr))
    }
}

/// Finds the file yt-dlp produced for the given output template base.
fn find_download(base: &std::path::Path) -> Option<PathBuf> {
    let directory = base.parent()?;
    let prefix = base.file_name()?.to_string_lossy().into_owned();
    std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            name.starts_with(&prefix) && !name.ends_with(".part") && name.contains('.')
        })
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

impl MusicProvider for YtDlpProvider {
    fn search(
        &self,
        query: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Vec<Song>>> + Send>> {
        let provider = self.clone();
        Box::pin(async move {
            if let Ok(mut cache) = provider.search_cache.lock() {
                if let Some(cached) = cache.get(&query).cloned() {
                    provider
                        .metrics
                        .cache_hits
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(cached);
                }
                provider
                    .metrics
                    .cache_misses
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            let provider_query = format!("ytsearch10:{query}");
            let output = provider
                .command(&[
                    "--flat-playlist",
                    "--dump-json",
                    "--skip-download",
                    &provider_query,
                ])
                .await?;
            let songs: Vec<Song> = output
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .map(|line| {
                    serde_json::from_slice::<YtDlpSong>(line)
                        .map(Song::from)
                        .map_err(|error| AppError::Provider(format!("respuesta inválida: {error}")))
                })
                .collect::<AppResult<_>>()?;
            if let Ok(mut cache) = provider.search_cache.lock() {
                cache.insert(
                    query.trim_start_matches("ytsearch10:").to_owned(),
                    songs.clone(),
                );
            }
            Ok(songs)
        })
    }

    fn stream_url(
        &self,
        song: Song,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<AudioStream>> + Send>> {
        let provider = self.clone();
        Box::pin(async move {
            let page = song
                .webpage_url
                .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", song.id));
            debug!(
                format = AUDIO_FORMAT,
                container = AUDIO_CONTAINER,
                codec = AUDIO_CODEC,
                "downloading audio with yt-dlp"
            );
            provider.open_audio_stream(&page).await
        })
    }

    fn related(
        &self,
        song: Song,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Vec<Song>>> + Send>> {
        let provider = self.clone();
        Box::pin(async move {
            let mix_url = format!(
                "https://www.youtube.com/watch?v={}&list=RD{}",
                song.id, song.id
            );
            let output = provider
                .command_authed(
                    &[
                        "--flat-playlist",
                        "--dump-json",
                        "--skip-download",
                        &mix_url,
                    ],
                    provider.resolve_timeout,
                )
                .await?;
            output
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .map(|line| {
                    serde_json::from_slice::<YtDlpSong>(line)
                        .map(Song::from)
                        .map_err(|error| AppError::Provider(format!("respuesta inválida: {error}")))
                })
                .filter_map(|result| match result {
                    Ok(item) if item.id != song.id => Some(Ok(item)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .take(10)
                .collect()
        })
    }
}

#[derive(Deserialize)]
struct YtDlpSong {
    id: String,
    title: String,
    #[serde(default)]
    uploader: String,
    duration: Option<f64>,
    thumbnail: Option<String>,
    webpage_url: Option<String>,
}

impl From<YtDlpSong> for Song {
    fn from(song: YtDlpSong) -> Self {
        Self {
            id: song.id,
            title: song.title,
            artist: song.uploader,
            duration: song.duration.map(|value| value as u64),
            thumbnail: song.thumbnail,
            webpage_url: song.webpage_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::AudioStreamReader;

    #[test]
    fn audio_reader_streams_chunks_and_reports_end() {
        let (sender, receiver) = tokio::sync::mpsc::channel(3);
        sender.blocking_send(Ok(Some(b"web".to_vec()))).unwrap();
        sender.blocking_send(Ok(Some(b"m".to_vec()))).unwrap();
        sender.blocking_send(Ok(None)).unwrap();
        let mut reader = AudioStreamReader {
            receiver: std::sync::Mutex::new(receiver),
            pending: Default::default(),
            finished: false,
        };
        let mut output = Vec::new();
        reader.read_to_end(&mut output).unwrap();
        assert_eq!(output, b"webm");
    }
}
