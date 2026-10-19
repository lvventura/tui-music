use std::{collections::VecDeque, time::Duration};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Play,
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
    Search,
    QueueAdd,
    QueueClear,
    Favorite,
    Unfavorite,
    Recommend,
    EnableAutoDj,
    DisableAutoDj,
    SetDjMode,
    SetVolume,
    SetShuffle,
    SetRepeat,
    Seek,
    NowPlaying,
    ShowHistory,
    ShowFavorites,
    ExplainRecommendation,
    Unknown,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Parameters {
    pub query: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub energy: Option<String>,
    pub volume: Option<u8>,
    pub enabled: Option<bool>,
    pub mode: Option<String>,
    pub repeat: Option<String>,
    pub seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Command {
    pub intent: Intent,
    #[serde(default)]
    pub parameters: Parameters,
    #[serde(default)]
    pub requires_confirmation: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum AIError {
    #[error("AI provider unavailable")]
    ProviderUnavailable,
    #[error("AI request timed out")]
    Timeout,
    #[error("AI returned invalid JSON")]
    InvalidResponse,
    #[error("invalid AI command: {0}")]
    InvalidCommand(String),
    #[error("AI configuration error: {0}")]
    Configuration(String),
    #[error("AI request failed: {0}")]
    Request(String),
}

pub trait AIProvider: Send + Sync {
    fn complete(
        &self,
        prompt: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Command, AIError>> + Send>>;
}

pub struct Assistant {
    provider: Option<Box<dyn AIProvider>>,
    pub conversation: Conversation,
}

impl Assistant {
    pub fn local(max_turns: usize) -> Self {
        Self {
            provider: None,
            conversation: Conversation::new(max_turns),
        }
    }

    pub fn with_provider(provider: Box<dyn AIProvider>, max_turns: usize) -> Self {
        Self {
            provider: Some(provider),
            conversation: Conversation::new(max_turns),
        }
    }

    pub async fn interpret(&mut self, input: &str) -> Result<Command, AIError> {
        let command = if let Some(provider) = &self.provider {
            provider
                .complete(format!("{}\n\nUsuario: {}", system_prompt(), input))
                .await?
        } else {
            parse_local(input)
        };
        validate(&command)?;
        self.conversation
            .push(input, format!("{:?}", command.intent));
        Ok(command)
    }
}

#[derive(Clone)]
pub struct OllamaProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    timeout: Duration,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            model: model.into(),
            timeout,
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    format: &'static str,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl AIProvider for OllamaProvider {
    fn complete(
        &self,
        prompt: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Command, AIError>> + Send>> {
        let provider = self.clone();
        Box::pin(async move {
            let response = tokio::time::timeout(
                provider.timeout,
                provider
                    .client
                    .post(format!(
                        "{}/api/generate",
                        provider.base_url.trim_end_matches('/')
                    ))
                    .json(&OllamaRequest {
                        model: &provider.model,
                        prompt: &prompt,
                        stream: false,
                        format: "json",
                    })
                    .send(),
            )
            .await
            .map_err(|_| AIError::Timeout)?
            .map_err(|error| AIError::Request(error.to_string()))?;
            if !response.status().is_success() {
                return Err(AIError::ProviderUnavailable);
            }
            let body = response
                .json::<OllamaResponse>()
                .await
                .map_err(|error| AIError::Request(error.to_string()))?;
            serde_json::from_str(&body.response).map_err(|_| AIError::InvalidResponse)
        })
    }
}

pub fn parse_local(input: &str) -> Command {
    let text = input.trim().to_lowercase();
    let parameters = Parameters::default();
    if text.contains("pausa") || text == "pause" {
        return Command {
            intent: Intent::Pause,
            parameters,
            requires_confirmation: false,
        };
    }
    if text.contains("reanuda") || text == "resume" {
        return Command {
            intent: Intent::Resume,
            parameters,
            requires_confirmation: false,
        };
    }
    if text.contains("siguiente") || text == "next" {
        return Command {
            intent: Intent::Next,
            parameters,
            requires_confirmation: false,
        };
    }
    if text.contains("anterior") || text == "previous" {
        return Command {
            intent: Intent::Previous,
            parameters,
            requires_confirmation: false,
        };
    }
    if text.contains("auto dj") {
        return Command {
            intent: if text.contains("desactiva") || text.contains("apaga") {
                Intent::DisableAutoDj
            } else {
                Intent::EnableAutoDj
            },
            parameters,
            requires_confirmation: false,
        };
    }
    if let Some(value) = extract_percent(&text) {
        if text.contains("volumen") || text.contains("volume") {
            return Command {
                intent: Intent::SetVolume,
                parameters: Parameters {
                    volume: Some(value),
                    ..parameters
                },
                requires_confirmation: false,
            };
        }
    }
    if text.contains("favoritos") {
        return Command {
            intent: Intent::ShowFavorites,
            parameters,
            requires_confirmation: false,
        };
    }
    if text.contains("historial") {
        return Command {
            intent: Intent::ShowHistory,
            parameters,
            requires_confirmation: false,
        };
    }
    if text.contains("recom") || text.contains("parecido") || text.contains("música") {
        let artist = text
            .strip_prefix("pon algo parecido a ")
            .or_else(|| text.strip_prefix("algo parecido a "))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        return Command {
            intent: Intent::Recommend,
            parameters: Parameters {
                artist,
                ..parameters
            },
            requires_confirmation: false,
        };
    }
    Command {
        intent: Intent::Unknown,
        parameters,
        requires_confirmation: false,
    }
}

fn extract_percent(text: &str) -> Option<u8> {
    text.split_whitespace()
        .find_map(|word| word.trim_end_matches('%').parse::<u8>().ok())
}

pub fn validate(command: &Command) -> Result<(), AIError> {
    if let Some(volume) = command.parameters.volume {
        if volume > 100 {
            return Err(AIError::InvalidCommand(
                "volume must be between 0 and 100".into(),
            ));
        }
    }
    if let Some(repeat) = &command.parameters.repeat {
        if !matches!(repeat.as_str(), "off" | "queue" | "song") {
            return Err(AIError::InvalidCommand("invalid repeat mode".into()));
        }
    }
    if let Some(seconds) = command.parameters.seconds {
        if !(-3600..=3600).contains(&seconds) {
            return Err(AIError::InvalidCommand("seek is out of range".into()));
        }
    }
    if matches!(command.intent, Intent::Unknown) {
        return Err(AIError::InvalidCommand("unknown intent".into()));
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct Conversation {
    turns: VecDeque<(String, String)>,
    max_turns: usize,
}

impl Conversation {
    pub fn new(max_turns: usize) -> Self {
        Self {
            turns: VecDeque::new(),
            max_turns: max_turns.max(1),
        }
    }

    pub fn push(&mut self, user: impl Into<String>, assistant: impl Into<String>) {
        self.turns.push_back((user.into(), assistant.into()));
        while self.turns.len() > self.max_turns {
            self.turns.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }
}

pub fn system_prompt() -> &'static str {
    "Eres el asistente de Lite|Music. Devuelve exclusivamente JSON con intent, parameters y requires_confirmation. Solo puedes controlar reproducción, cola, favoritos, recomendaciones y configuración musical. Nunca devuelvas shell, SQL, rutas, archivos ni código."
}

pub fn app_error(error: AIError) -> AppError {
    AppError::Provider(format!("AI: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_parser_creates_safe_structured_commands() {
        let command = parse_local("sube el volumen al 70%");
        assert_eq!(command.intent, Intent::SetVolume);
        assert_eq!(command.parameters.volume, Some(70));
        assert!(validate(&command).is_ok());
    }

    #[test]
    fn validator_rejects_invalid_volume() {
        let command = Command {
            intent: Intent::SetVolume,
            parameters: Parameters {
                volume: Some(101),
                ..Default::default()
            },
            requires_confirmation: false,
        };
        assert!(validate(&command).is_err());
    }

    #[test]
    fn conversation_is_bounded() {
        let mut conversation = Conversation::new(2);
        conversation.push("one", "one");
        conversation.push("two", "two");
        conversation.push("three", "three");
        assert_eq!(conversation.len(), 2);
    }

    #[tokio::test]
    async fn local_assistant_never_needs_an_ai_server() {
        let mut assistant = Assistant::local(2);
        let command = assistant.interpret("pausa").await.unwrap();
        assert_eq!(command.intent, Intent::Pause);
    }
}
