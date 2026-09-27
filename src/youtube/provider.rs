use std::{
    collections::VecDeque,
    io::{self, Read, Seek, SeekFrom},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::StreamExt;
use serde::Deserialize;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::errors::{AppError, AppResult};
use crate::{cache::Cache, config::Settings, metrics::Metrics};

use super::models::Song;

const AUDIO_FORMAT: &str = "bestaudio[ext=m4a]/140";
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
    http: reqwest::Client,
    search_cache: Arc<Mutex<Cache<String, Vec<Song>>>>,
    timeout: Duration,
    retries: u32,
    retry_delay: Duration,
    pub metrics: Arc<Metrics>,
}

impl YtDlpProvider {
    pub fn new(executable: impl Into<String>, settings: &Settings) -> Self {
        let timeout = Duration::from_secs(settings.network.request_timeout_seconds);
        Self {
            executable: executable.into(),
            http: reqwest::Client::builder()
                // A total request timeout would abort a valid long-running audio body.
                .connect_timeout(timeout)
                .http1_only()
                .build()
                .unwrap_or_default(),
            search_cache: Arc::new(Mutex::new(Cache::new(
                settings.cache.memory_capacity,
                Duration::from_secs(settings.cache.search_ttl_seconds),
            ))),
            timeout,
            retries: settings.network.max_retries,
            retry_delay: Duration::from_millis(settings.network.initial_retry_delay_ms),
            metrics: Arc::new(Metrics::default()),
        }
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
        for attempt in 0..=self.retries {
            self.metrics
                .provider_requests
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let result = tokio::time::timeout(self.timeout, async {
                Command::new(&self.executable)
                    .args(args)
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
                    return Err(AppError::Provider(
                        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                    ));
                }
                Ok(Err(error)) if attempt == self.retries => {
                    return Err(AppError::Provider(format!("yt-dlp no disponible: {error}")));
                }
                Err(_) if attempt == self.retries => {
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

    async fn open_audio_stream(&self, url: &str) -> AppResult<AudioStream> {
        let request_url = sanitize_url(url);
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|error| {
                warn!(url = %request_url, error = %error, "audio HTTP request failed");
                AppError::Provider(format!(
                    "audio HTTP request failed for {request_url}: {error}"
                ))
            })?;
        let status = response.status();
        let headers = response.headers().clone();
        debug!(
            url = %request_url,
            status = %status,
            content_type = ?headers.get(reqwest::header::CONTENT_TYPE),
            content_encoding = ?headers.get(reqwest::header::CONTENT_ENCODING),
            content_length = ?headers.get(reqwest::header::CONTENT_LENGTH),
            server = ?headers.get(reqwest::header::SERVER),
            "audio HTTP response"
        );
        if !status.is_success() {
            let body = response.bytes().await.map_err(|error| {
                AppError::Provider(format!(
                    "audio HTTP status {status}; body read failed: {error}"
                ))
            })?;
            return Err(AppError::Provider(format!(
                "audio HTTP status {status} for {request_url}: {}",
                preview_body(&body),
            )));
        }
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let (sender, receiver) = tokio::sync::mpsc::channel(8);
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut chunks = 0u64;
            let mut total_bytes = 0u64;
            while let Some(chunk) = stream.next().await {
                let result = match chunk {
                    Ok(bytes) => {
                        chunks += 1;
                        total_bytes += bytes.len() as u64;
                        debug!(
                            chunk = chunks,
                            bytes = bytes.len(),
                            total_bytes,
                            "[STREAM] chunk received"
                        );
                        Ok(Some(bytes.to_vec()))
                    }
                    Err(error) => {
                        warn!(
                            chunks,
                            total_bytes,
                            error = %error,
                            "[STREAM] body error"
                        );
                        Err(format!("audio stream body error: {error}"))
                    }
                };
                if sender.send(result).await.is_err() {
                    debug!(
                        chunks,
                        total_bytes, "[STREAM] consumer dropped before producer EOF"
                    );
                    break;
                }
            }
            debug!(chunks, total_bytes, "[STREAM] EOF received");
            let _ = sender.send(Ok(None)).await;
        });
        debug!(url = %request_url, "audio stream opened");
        Ok(AudioStream {
            reader: AudioStreamReader {
                receiver: std::sync::Mutex::new(receiver),
                pending: VecDeque::new(),
                finished: false,
            },
            content_type,
        })
    }
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
                "resolving audio format with yt-dlp"
            );
            let output = provider
                .command(&["-f", AUDIO_FORMAT, "-g", "--no-playlist", &page])
                .await?;
            let url =
                String::from_utf8(output).map_err(|error| AppError::Provider(error.to_string()))?;
            for attempt in 0..=provider.retries {
                match provider.open_audio_stream(url.trim()).await {
                    Ok(stream) => return Ok(stream),
                    Err(error) if attempt == provider.retries => return Err(error.into()),
                    Err(_) => tokio::time::sleep(provider.retry_delay * 2u32.pow(attempt)).await,
                }
            }

            Err(AppError::Provider("fallo descargando audio".into()))
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
                .command(&[
                    "--flat-playlist",
                    "--dump-json",
                    "--skip-download",
                    &mix_url,
                ])
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

fn sanitize_url(url: &str) -> String {
    reqwest::Url::parse(url)
        .map(|parsed| {
            format!(
                "{}://{}{}",
                parsed.scheme(),
                parsed.host_str().unwrap_or("<unknown-host>"),
                parsed.path()
            )
        })
        .unwrap_or_else(|_| "<invalid-url>".into())
}

fn preview_body(body: &[u8]) -> String {
    let preview = &body[..body.len().min(1000)];
    match std::str::from_utf8(preview) {
        Ok(text) => text.replace('\r', "\\r").replace('\n', "\\n"),
        Err(_) => preview
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::{AudioStreamReader, preview_body, sanitize_url};

    #[test]
    fn diagnostics_never_expose_signed_url_query() {
        assert_eq!(
            sanitize_url("https://audio.example.test/path?token=secret"),
            "https://audio.example.test/path"
        );
    }

    #[test]
    fn body_preview_handles_text_and_binary_without_invalid_utf8_slicing() {
        assert_eq!(preview_body(b"status\nerror"), "status\\nerror");
        assert_eq!(preview_body(&[0xff, 0x00, 0x01]), "ff 00 01");
    }

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
