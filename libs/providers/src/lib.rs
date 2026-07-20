use async_trait::async_trait;
use errors::ContextraError;
use futures_util::{Stream, StreamExt};
use rand::Rng;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use settings::ProvidersSettings;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

const OPENAI_BASE_URL: &str = "https://api.openai.com";
const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
const DEFAULT_MAX_RETRIES: usize = 3;
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(250);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(4);

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>;

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream, ProviderError>;

    fn supports_function_calling(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(ChatRole::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(ChatRole::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(ChatRole::Assistant, content)
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Tool,
            content: Some(content.into()),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: Vec::new(),
        }
    }

    pub fn new(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatTool {
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    #[serde(rename = "function")]
    Function {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ChatTool>,
    #[serde(default, skip_serializing)]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            user: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub message: ChatMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    pub model: String,
    pub delta: ChatDelta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<ChatRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallDelta>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("missing provider configuration: {0}")]
    MissingConfiguration(String),

    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("invalid provider request: {0}")]
    InvalidRequest(String),

    #[error("provider authentication failed: {0}")]
    Authentication(String),

    #[error("provider rate limit exceeded: {0}")]
    RateLimited(String),

    #[error("provider returned HTTP {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },

    #[error("provider network error: {0}")]
    Network(String),

    #[error("provider timeout: {0}")]
    Timeout(String),

    #[error("failed to decode provider response: {0}")]
    Decode(String),

    #[error("provider stream error: {0}")]
    Stream(String),
}

impl From<ProviderError> for ContextraError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::MissingConfiguration(message)
            | ProviderError::InvalidRequest(message)
            | ProviderError::UnsupportedProvider(message) => Self::Validation(message),
            ProviderError::Authentication(message) => Self::Unauthorized(message),
            ProviderError::RateLimited(message) => Self::RateLimited(message),
            ProviderError::HttpStatus { status, body } if status == StatusCode::UNAUTHORIZED => {
                Self::Unauthorized(body)
            }
            ProviderError::HttpStatus { status, body } if status == StatusCode::FORBIDDEN => {
                Self::Forbidden(body)
            }
            ProviderError::HttpStatus { status, body }
                if status == StatusCode::TOO_MANY_REQUESTS =>
            {
                Self::RateLimited(body)
            }
            other => Self::ProviderError(other.to_string()),
        }
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout(error.to_string())
        } else {
            Self::Network(error.to_string())
        }
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(error: serde_json::Error) -> Self {
        Self::Decode(error.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ProviderFactory {
    settings: ProvidersSettings,
    client: Client,
}

impl ProviderFactory {
    pub fn new(settings: ProvidersSettings) -> Self {
        Self {
            settings,
            client: Client::new(),
        }
    }

    pub fn create_llm_provider(&self, name: &str) -> Result<Arc<dyn LLMProvider>, ProviderError> {
        match normalize_provider_name(name).as_str() {
            "openai" => {
                let api_key = self.settings.openai_api_key.clone().ok_or_else(|| {
                    ProviderError::MissingConfiguration(
                        "providers.openai_api_key is required".to_string(),
                    )
                })?;
                Ok(Arc::new(OpenAIProvider::with_client(
                    api_key,
                    self.client.clone(),
                )))
            }
            provider => Err(ProviderError::UnsupportedProvider(provider.to_string())),
        }
    }

    pub fn create_registry(&self) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        if self.settings.openai_api_key.is_some()
            && let Ok(provider) = self.create_llm_provider("openai")
        {
            registry.register_llm("openai", provider);
        }
        registry
    }
}

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    llm_providers: HashMap<String, Arc<dyn LLMProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_settings(settings: &ProvidersSettings) -> Self {
        ProviderFactory::new(settings.clone()).create_registry()
    }

    pub fn register_llm(
        &mut self,
        name: impl Into<String>,
        provider: Arc<dyn LLMProvider>,
    ) -> Option<Arc<dyn LLMProvider>> {
        self.llm_providers
            .insert(normalize_provider_name(&name.into()), provider)
    }

    pub fn get_llm(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        self.llm_providers
            .get(&normalize_provider_name(name))
            .cloned()
    }

    pub fn contains_llm(&self, name: &str) -> bool {
        self.llm_providers
            .contains_key(&normalize_provider_name(name))
    }

    pub fn llm_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.llm_providers.keys().cloned().collect();
        names.sort();
        names
    }
}

#[derive(Debug, Clone)]
pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    client: Client,
    max_retries: usize,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl OpenAIProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_client(api_key, Client::new())
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            ..Self::with_client(api_key, Client::new())
        }
    }

    fn with_client(api_key: impl Into<String>, client: Client) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: OPENAI_BASE_URL.to_string(),
            client,
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
        }
    }

    pub fn with_retry_config(
        mut self,
        max_retries: usize,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        self.max_retries = max_retries;
        self.initial_backoff = initial_backoff;
        self.max_backoff = max_backoff;
        self
    }

    fn chat_completions_url(&self) -> String {
        format!("{}{}", self.base_url, CHAT_COMPLETIONS_PATH)
    }

    async fn send_chat_request(
        &self,
        payload: &OpenAIChatRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        for attempt in 0..=self.max_retries {
            let result = self
                .client
                .post(self.chat_completions_url())
                .bearer_auth(&self.api_key)
                .json(payload)
                .send()
                .await;

            match result {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => {
                    let status = response.status();
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|error| format!("failed to read error body: {error}"));

                    let provider_error = map_status_error(status, body);
                    if !is_retryable_status(status) || attempt == self.max_retries {
                        return Err(provider_error);
                    }
                }
                Err(error) => {
                    let provider_error = ProviderError::from(error);
                    if !is_retryable_request_error(&provider_error) || attempt == self.max_retries {
                        return Err(provider_error);
                    }
                }
            }

            tokio::time::sleep(self.retry_delay(attempt)).await;
        }

        Err(ProviderError::Network(
            "retry loop exited unexpectedly".to_string(),
        ))
    }

    fn retry_delay(&self, attempt: usize) -> Duration {
        let multiplier = 2_u32.saturating_pow(attempt.min(16) as u32);
        let base = self.initial_backoff.saturating_mul(multiplier);
        let capped = base.min(self.max_backoff);
        let jitter = rand::thread_rng().gen_range(0.5..=1.5);
        Duration::from_secs_f64(capped.as_secs_f64() * jitter)
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let payload = OpenAIChatRequest::from_chat_request(request, false);
        let response = self.send_chat_request(&payload).await?;
        let completion = response
            .json::<OpenAIChatResponse>()
            .await
            .map_err(ProviderError::from)?;

        completion.try_into_chat_response()
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream, ProviderError> {
        let payload = OpenAIChatRequest::from_chat_request(request, true);
        let response = self.send_chat_request(&payload).await?;
        let byte_stream = response.bytes_stream();

        Ok(Box::pin(async_stream::try_stream! {
            let mut stream = byte_stream;
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                let bytes = chunk.map_err(ProviderError::from)?;
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(event_end) = buffer.find("\n\n") {
                    let event = buffer[..event_end].to_string();
                    buffer = buffer[event_end + 2..].to_string();

                    if let Some(data) = parse_sse_data(&event) {
                        if data == "[DONE]" {
                            return;
                        }

                        let openai_chunk = serde_json::from_str::<OpenAIStreamResponse>(&data)
                            .map_err(ProviderError::from)?;

                        for chunk in openai_chunk.into_chat_chunks() {
                            yield chunk;
                        }
                    }
                }
            }

            if !buffer.trim().is_empty() {
                Err(ProviderError::Stream(
                    "stream ended with an incomplete SSE event".to_string(),
                ))?;
            }
        }))
    }

    fn supports_function_calling(&self) -> bool {
        true
    }
}

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAITool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

impl OpenAIChatRequest {
    fn from_chat_request(request: ChatRequest, stream: bool) -> Self {
        Self {
            model: request.model,
            messages: request
                .messages
                .into_iter()
                .map(OpenAIMessage::from)
                .collect(),
            stream,
            temperature: request.temperature,
            top_p: request.top_p,
            max_tokens: request.max_tokens,
            stop: request.stop,
            tools: request.tools.into_iter().map(OpenAITool::from).collect(),
            tool_choice: request.tool_choice.map(tool_choice_to_openai_value),
            user: request.user,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: ChatRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OpenAIToolCall>,
}

impl From<ChatMessage> for OpenAIMessage {
    fn from(message: ChatMessage) -> Self {
        Self {
            role: message.role,
            content: message.content,
            name: message.name,
            tool_call_id: message.tool_call_id,
            tool_calls: message
                .tool_calls
                .into_iter()
                .map(OpenAIToolCall::from)
                .collect(),
        }
    }
}

impl From<OpenAIMessage> for ChatMessage {
    fn from(message: OpenAIMessage) -> Self {
        Self {
            role: message.role,
            content: message.content,
            name: message.name,
            tool_call_id: message.tool_call_id,
            tool_calls: message.tool_calls.into_iter().map(ToolCall::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDefinition,
}

impl From<ChatTool> for OpenAITool {
    fn from(tool: ChatTool) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: tool.function,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionCall,
}

impl From<ToolCall> for OpenAIToolCall {
    fn from(tool_call: ToolCall) -> Self {
        Self {
            id: tool_call.id,
            tool_type: "function".to_string(),
            function: tool_call.function,
        }
    }
}

impl From<OpenAIToolCall> for ToolCall {
    fn from(tool_call: OpenAIToolCall) -> Self {
        Self {
            id: tool_call.id,
            function: tool_call.function,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: Option<TokenUsage>,
}

impl OpenAIChatResponse {
    fn try_into_chat_response(self) -> Result<ChatResponse, ProviderError> {
        let choice = self.choices.into_iter().next().ok_or_else(|| {
            ProviderError::Decode("OpenAI response did not include any choices".to_string())
        })?;

        Ok(ChatResponse {
            id: self.id,
            model: self.model,
            message: choice.message.into(),
            finish_reason: choice.finish_reason,
            usage: self.usage,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIStreamChoice>,
    usage: Option<TokenUsage>,
}

impl OpenAIStreamResponse {
    fn into_chat_chunks(self) -> Vec<ChatChunk> {
        self.choices
            .into_iter()
            .map(|choice| ChatChunk {
                id: self.id.clone(),
                model: self.model.clone(),
                delta: choice.delta.into(),
                finish_reason: choice.finish_reason,
                usage: self.usage.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAIStreamDelta {
    role: Option<ChatRole>,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAIToolCallDelta>,
}

impl From<OpenAIStreamDelta> for ChatDelta {
    fn from(delta: OpenAIStreamDelta) -> Self {
        Self {
            role: delta.role,
            content: delta.content,
            tool_calls: delta
                .tool_calls
                .into_iter()
                .map(ToolCallDelta::from)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCallDelta {
    index: u32,
    id: Option<String>,
    function: Option<FunctionCallDelta>,
}

impl From<OpenAIToolCallDelta> for ToolCallDelta {
    fn from(delta: OpenAIToolCallDelta) -> Self {
        Self {
            index: delta.index,
            id: delta.id,
            function: delta.function,
        }
    }
}

fn normalize_provider_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn tool_choice_to_openai_value(choice: ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => Value::String("auto".to_string()),
        ToolChoice::None => Value::String("none".to_string()),
        ToolChoice::Required => Value::String("required".to_string()),
        ToolChoice::Function { name } => serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
            },
        }),
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn is_retryable_request_error(error: &ProviderError) -> bool {
    matches!(error, ProviderError::Timeout(_) | ProviderError::Network(_))
}

fn map_status_error(status: StatusCode, body: String) -> ProviderError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::Authentication(body),
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited(body),
        _ => ProviderError::HttpStatus { status, body },
    }
}

fn parse_sse_data(event: &str) -> Option<String> {
    let data_lines: Vec<&str> = event
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect();

    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

// Tests
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode as AxumStatusCode};
    use axum::response::{IntoResponse, Response};
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::json;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    #[derive(Clone)]
    struct MockServerState {
        calls: Arc<AtomicUsize>,
        mode: MockMode,
    }

    #[derive(Clone)]
    enum MockMode {
        Chat,
        Stream,
        Retry,
    }

    #[tokio::test]
    async fn openai_chat_completion_maps_response() -> Result<(), Box<dyn std::error::Error>> {
        let server = TestServer::start(MockMode::Chat).await?;
        let provider = OpenAIProvider::with_base_url("test-key", server.base_url())
            .with_retry_config(0, Duration::from_millis(1), Duration::from_millis(1));

        let response = provider
            .chat(ChatRequest::new(
                "gpt-test",
                vec![ChatMessage::user("Hello")],
            ))
            .await?;

        assert_eq!(response.id, "chatcmpl-test");
        assert_eq!(response.model, "gpt-test");
        assert_eq!(response.message.role, ChatRole::Assistant);
        assert_eq!(
            response.message.content.as_deref(),
            Some("Hello from OpenAI")
        );
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(response.usage.map(|usage| usage.total_tokens), Some(8));
        assert_eq!(server.calls(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn openai_chat_stream_parses_sse_chunks() -> Result<(), Box<dyn std::error::Error>> {
        let server = TestServer::start(MockMode::Stream).await?;
        let provider = OpenAIProvider::with_base_url("test-key", server.base_url())
            .with_retry_config(0, Duration::from_millis(1), Duration::from_millis(1));

        let mut stream = provider
            .chat_stream(ChatRequest::new(
                "gpt-test",
                vec![ChatMessage::user("Hello")],
            ))
            .await?;

        let first = stream.next().await.ok_or("missing first stream chunk")??;
        let second = stream.next().await.ok_or("missing second stream chunk")??;

        assert_eq!(first.delta.role, Some(ChatRole::Assistant));
        assert_eq!(first.delta.content.as_deref(), Some("Hel"));
        assert_eq!(second.delta.content.as_deref(), Some("lo"));
        assert_eq!(second.finish_reason.as_deref(), Some("stop"));
        assert!(stream.next().await.is_none());
        assert_eq!(server.calls(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn openai_retries_429_and_5xx() -> Result<(), Box<dyn std::error::Error>> {
        let server = TestServer::start(MockMode::Retry).await?;
        let provider = OpenAIProvider::with_base_url("test-key", server.base_url())
            .with_retry_config(3, Duration::from_millis(1), Duration::from_millis(1));

        let response = provider
            .chat(ChatRequest::new(
                "gpt-test",
                vec![ChatMessage::user("Hello")],
            ))
            .await?;

        assert_eq!(response.message.content.as_deref(), Some("Recovered"));
        assert_eq!(server.calls(), 3);

        Ok(())
    }

    #[test]
    fn factory_and_registry_create_openai_from_settings() -> Result<(), Box<dyn std::error::Error>>
    {
        let settings = ProvidersSettings {
            openai_api_key: Some("test-key".to_string()),
            anthropic_api_key: None,
        };

        let factory = ProviderFactory::new(settings.clone());
        let provider = factory.create_llm_provider("OpenAI")?;
        assert!(provider.supports_function_calling());

        let registry = ProviderRegistry::from_settings(&settings);
        assert!(registry.contains_llm("openai"));
        assert_eq!(registry.llm_names(), vec!["openai".to_string()]);

        Ok(())
    }

    #[test]
    fn provider_error_maps_into_context_error() {
        let context_error =
            ContextraError::from(ProviderError::RateLimited("slow down".to_string()));
        assert!(matches!(context_error, ContextraError::RateLimited(_)));

        let context_error =
            ContextraError::from(ProviderError::Authentication("bad key".to_string()));
        assert!(matches!(context_error, ContextraError::Unauthorized(_)));
    }

    struct TestServer {
        address: SocketAddr,
        calls: Arc<AtomicUsize>,
        shutdown: Option<oneshot::Sender<()>>,
    }

    impl TestServer {
        async fn start(mode: MockMode) -> Result<Self, Box<dyn std::error::Error>> {
            let calls = Arc::new(AtomicUsize::new(0));
            let state = MockServerState {
                calls: Arc::clone(&calls),
                mode,
            };

            let app = Router::new()
                .route(CHAT_COMPLETIONS_PATH, post(mock_chat_completions))
                .with_state(state);
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let (shutdown_sender, shutdown_receiver) = oneshot::channel();

            tokio::spawn(async move {
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = shutdown_receiver.await;
                    })
                    .await;
            });

            Ok(Self {
                address,
                calls,
                shutdown: Some(shutdown_sender),
            })
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.address)
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }

    async fn mock_chat_completions(
        State(state): State<MockServerState>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Response {
        let call_number = state.calls.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer test-key")
        );
        assert_eq!(body.get("model").and_then(Value::as_str), Some("gpt-test"));
        assert_eq!(
            body.get("stream").and_then(Value::as_bool),
            Some(matches!(state.mode, MockMode::Stream))
        );

        match state.mode {
            MockMode::Chat => Json(chat_response("Hello from OpenAI")).into_response(),
            MockMode::Stream => Response::builder()
                .status(AxumStatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from(stream_response()))
                .unwrap_or_else(|error| {
                    (
                        AxumStatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to build response: {error}"),
                    )
                        .into_response()
                }),
            MockMode::Retry if call_number == 1 => {
                (AxumStatusCode::TOO_MANY_REQUESTS, "rate limited").into_response()
            }
            MockMode::Retry if call_number == 2 => {
                (AxumStatusCode::INTERNAL_SERVER_ERROR, "temporary failure").into_response()
            }
            MockMode::Retry => Json(chat_response("Recovered")).into_response(),
        }
    }

    fn chat_response(content: &str) -> Value {
        json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1,
            "model": "gpt-test",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 3,
                "completion_tokens": 5,
                "total_tokens": 8
            }
        })
    }

    fn stream_response() -> String {
        [
            r#"data: {"id":"chatcmpl-test","model":"gpt-test","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}"#,
            "",
            r#"data: {"id":"chatcmpl-test","model":"gpt-test","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":"stop"}]}"#,
            "",
            "data: [DONE]",
            "",
            "",
        ]
        .join("\n")
    }
}
