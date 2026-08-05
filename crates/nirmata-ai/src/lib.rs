pub mod capabilities;
pub mod contracts;

use bytes::Bytes;
use futures_util::{StreamExt, stream::BoxStream};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{env, error::Error, fmt, future::Future, pin::Pin, time::Duration};

pub use tokio_util::sync::CancellationToken;

#[cfg(windows)]
use keyring::{Entry, Error as KeyringError};

const DEFAULT_CREDENTIAL_SERVICE: &str = "nirmata";
const DEFAULT_CREDENTIAL_ACCOUNT: &str = "azure-foundry-api-key";
const ENV_PROVIDER_API_KEY: &str = "PROVIDER_API_KEY";

type SendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<serde_json::Number>,
    pub max_output_tokens: Option<u32>,
}

impl ChatCompletionRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_output_tokens: None,
        }
    }

    pub fn with_temperature(mut self, value: f64) -> Self {
        self.temperature = serde_json::Number::from_f64(value);
        self
    }

    pub fn with_max_output_tokens(mut self, value: u32) -> Self {
        self.max_output_tokens = Some(value);
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionResponse {
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<ChatCompletionUsage>,
    pub output_text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamDelta {
    pub delta: String,
}

#[derive(Clone, Debug)]
pub struct RequestOptions {
    pub timeout: Duration,
    pub cancellation: Option<CancellationToken>,
}

impl RequestOptions {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            cancellation: None,
        }
    }

    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    Missing,
    SessionEnvironment,
    SessionMemory,
    SystemSecureStore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPersistence {
    None,
    Session,
    SystemSecureStore,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialStatus {
    pub configured: bool,
    pub source: CredentialSource,
    pub persistence: CredentialPersistence,
    pub secure_store_available: bool,
    pub limitation: Option<String>,
}

#[derive(Debug)]
pub enum AiError {
    InvalidBaseUrl(String),
    EmptyProviderApiKey,
    MissingProviderApiKey,
    CredentialStoreClearFailed,
    RequestTimedOut(Duration),
    RequestCancelled,
    Transport(String),
    InvalidHttpStatus { status: u16, message: String },
    InvalidResponse(String),
    StreamInterrupted,
}

impl fmt::Display for AiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBaseUrl(message) => write!(formatter, "{message}"),
            Self::EmptyProviderApiKey => write!(formatter, "provider API key is required"),
            Self::MissingProviderApiKey => write!(formatter, "provider API key is not configured"),
            Self::CredentialStoreClearFailed => write!(
                formatter,
                "the system credential store could not be cleared; the saved provider key may still exist"
            ),
            Self::RequestTimedOut(timeout) => write!(
                formatter,
                "Azure Foundry request timed out after {}s",
                timeout.as_secs()
            ),
            Self::RequestCancelled => write!(formatter, "Azure Foundry request was cancelled"),
            Self::Transport(message) => {
                write!(formatter, "Azure Foundry request failed: {message}")
            }
            Self::InvalidHttpStatus { status, message } => {
                write!(formatter, "Azure Foundry returned HTTP {status}: {message}")
            }
            Self::InvalidResponse(message) => {
                write!(
                    formatter,
                    "Azure Foundry returned an invalid response: {message}"
                )
            }
            Self::StreamInterrupted => write!(
                formatter,
                "Azure Foundry stream ended before the completion marker was received"
            ),
        }
    }
}

impl Error for AiError {}

pub struct ProviderCredentialStore {
    inner: ProviderCredentialStoreInner<SystemCredentialBackend>,
}

impl ProviderCredentialStore {
    pub fn new() -> Self {
        Self {
            inner: ProviderCredentialStoreInner::new(
                SystemCredentialBackend::new(
                    DEFAULT_CREDENTIAL_SERVICE,
                    DEFAULT_CREDENTIAL_ACCOUNT,
                ),
                env::var(ENV_PROVIDER_API_KEY).ok(),
            ),
        }
    }

    pub fn status(&self) -> ProviderCredentialStatus {
        self.inner.status()
    }

    pub fn set_provider_api_key(
        &mut self,
        api_key: String,
    ) -> Result<ProviderCredentialStatus, AiError> {
        self.inner.set_provider_api_key(api_key)
    }

    pub fn clear_provider_api_key(&mut self) -> Result<ProviderCredentialStatus, AiError> {
        self.inner.clear_provider_api_key()
    }

    pub fn clone_api_key(&self) -> Option<String> {
        self.inner.clone_api_key()
    }
}

impl Default for ProviderCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AzureFoundryClient {
    inner: AzureFoundryClientInner<ReqwestTransport>,
}

impl AzureFoundryClient {
    pub fn new(base_url: &str) -> Result<Self, AiError> {
        Ok(Self {
            inner: AzureFoundryClientInner::new(
                normalize_base_url(base_url)?,
                ReqwestTransport::default(),
            ),
        })
    }

    pub async fn complete_chat(
        &self,
        api_key: &str,
        request: ChatCompletionRequest,
        options: RequestOptions,
    ) -> Result<ChatCompletionResponse, AiError> {
        self.inner.complete_chat(api_key, request, options).await
    }

    pub async fn stream_chat<F>(
        &self,
        api_key: &str,
        request: ChatCompletionRequest,
        options: RequestOptions,
        on_delta: F,
    ) -> Result<ChatCompletionResponse, AiError>
    where
        F: FnMut(StreamDelta) + Send,
    {
        self.inner
            .stream_chat(api_key, request, options, on_delta)
            .await
    }
}

struct ProviderCredentialStoreInner<B> {
    backend: B,
    persisted_key: Option<String>,
    session_key: Option<SessionCredential>,
    secure_store_available: bool,
    secure_store_limitation: Option<String>,
}

impl<B> ProviderCredentialStoreInner<B>
where
    B: CredentialBackend,
{
    fn new(backend: B, env_api_key: Option<String>) -> Self {
        let mut store = Self {
            backend,
            persisted_key: None,
            session_key: None,
            secure_store_available: true,
            secure_store_limitation: None,
        };
        store.load_persisted_key();
        if store.persisted_key.is_none() {
            if let Some(api_key) = normalize_api_key(env_api_key.as_deref()) {
                store.session_key = Some(SessionCredential {
                    value: api_key,
                    source: CredentialSource::SessionEnvironment,
                });
            }
        }
        store
    }

    fn status(&self) -> ProviderCredentialStatus {
        let resolved = self.resolve_key();
        let source = resolved
            .as_ref()
            .map_or(CredentialSource::Missing, |value| value.source);
        let limitation = match source {
            CredentialSource::SessionEnvironment => Some(
                "Loaded from PROVIDER_API_KEY for this app session only; it was not persisted."
                    .to_owned(),
            ),
            CredentialSource::SessionMemory => Some(
                self.secure_store_limitation.clone().unwrap_or_else(|| {
                    "The provider key is available only for this app session.".to_owned()
                }),
            ),
            CredentialSource::SystemSecureStore => None,
            CredentialSource::Missing => self
                .secure_store_limitation
                .as_ref()
                .map(|_| {
                    "The system credential store is unavailable; any key set now will only live for this app session."
                        .to_owned()
                }),
        };

        ProviderCredentialStatus {
            configured: resolved.is_some(),
            source,
            persistence: match source {
                CredentialSource::SystemSecureStore => CredentialPersistence::SystemSecureStore,
                CredentialSource::SessionEnvironment | CredentialSource::SessionMemory => {
                    CredentialPersistence::Session
                }
                CredentialSource::Missing => CredentialPersistence::None,
            },
            secure_store_available: self.secure_store_available,
            limitation,
        }
    }

    fn set_provider_api_key(
        &mut self,
        api_key: String,
    ) -> Result<ProviderCredentialStatus, AiError> {
        let api_key =
            normalize_api_key(Some(api_key.as_str())).ok_or(AiError::EmptyProviderApiKey)?;
        self.session_key = None;

        if self.secure_store_available {
            match self.backend.set(&api_key) {
                Ok(()) => {
                    self.persisted_key = Some(api_key);
                    self.secure_store_limitation = None;
                    return Ok(self.status());
                }
                Err(_) => self.note_secure_store_issue(
                    "The system credential store rejected the key, so it will only live for this app session.",
                ),
            }
        }

        self.session_key = Some(SessionCredential {
            value: api_key,
            source: CredentialSource::SessionMemory,
        });
        Ok(self.status())
    }

    fn clear_provider_api_key(&mut self) -> Result<ProviderCredentialStatus, AiError> {
        self.session_key = None;
        if self.persisted_key.is_none() {
            return Ok(self.status());
        }

        if !self.secure_store_available {
            return Err(AiError::CredentialStoreClearFailed);
        }

        match self.backend.clear() {
            Ok(()) | Err(CredentialBackendError::NotFound) => {
                self.persisted_key = None;
                Ok(self.status())
            }
            Err(_) => {
                self.note_secure_store_issue(
                    "The system credential store could not be cleared in this session.",
                );
                Err(AiError::CredentialStoreClearFailed)
            }
        }
    }

    fn clone_api_key(&self) -> Option<String> {
        self.resolve_key().map(|value| value.value.to_owned())
    }

    fn load_persisted_key(&mut self) {
        match self.backend.check_available() {
            Ok(()) => match self.backend.get() {
                Ok(value) => self.persisted_key = value,
                Err(CredentialBackendError::NotFound) => {}
                Err(_) => self.note_secure_store_issue(
                    "The system credential store could not be read in this session.",
                ),
            },
            Err(message) => self.note_secure_store_issue(message),
        }
    }

    fn resolve_key(&self) -> Option<ResolvedCredential<'_>> {
        self.session_key
            .as_ref()
            .map(|value| ResolvedCredential {
                value: value.value.as_str(),
                source: value.source,
            })
            .or_else(|| {
                self.persisted_key
                    .as_deref()
                    .map(|value| ResolvedCredential {
                        value,
                        source: CredentialSource::SystemSecureStore,
                    })
            })
    }

    fn note_secure_store_issue(&mut self, message: &'static str) {
        self.secure_store_available = false;
        self.secure_store_limitation = Some(message.to_owned());
    }
}

struct SessionCredential {
    value: String,
    source: CredentialSource,
}

struct ResolvedCredential<'a> {
    value: &'a str,
    source: CredentialSource,
}

trait CredentialBackend {
    fn check_available(&self) -> Result<(), &'static str>;
    fn get(&self) -> Result<Option<String>, CredentialBackendError>;
    fn set(&self, api_key: &str) -> Result<(), CredentialBackendError>;
    fn clear(&self) -> Result<(), CredentialBackendError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialBackendError {
    Unavailable,
    NotFound,
    OperationFailed,
}

#[cfg(windows)]
struct SystemCredentialBackend {
    entry: Option<Entry>,
}

#[cfg(windows)]
impl SystemCredentialBackend {
    fn new(service: &str, account: &str) -> Self {
        let entry = Entry::new(service, account).ok();
        Self { entry }
    }
}

#[cfg(windows)]
impl CredentialBackend for SystemCredentialBackend {
    fn check_available(&self) -> Result<(), &'static str> {
        if self.entry.is_some() {
            Ok(())
        } else {
            Err("The system credential store is unavailable in this session.")
        }
    }

    fn get(&self) -> Result<Option<String>, CredentialBackendError> {
        let Some(entry) = &self.entry else {
            return Err(CredentialBackendError::Unavailable);
        };
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Err(CredentialBackendError::OperationFailed),
        }
    }

    fn set(&self, api_key: &str) -> Result<(), CredentialBackendError> {
        let Some(entry) = &self.entry else {
            return Err(CredentialBackendError::Unavailable);
        };
        entry
            .set_password(api_key)
            .map_err(|_| CredentialBackendError::OperationFailed)
    }

    fn clear(&self) -> Result<(), CredentialBackendError> {
        let Some(entry) = &self.entry else {
            return Err(CredentialBackendError::Unavailable);
        };
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(KeyringError::NoEntry) => Err(CredentialBackendError::NotFound),
            Err(_) => Err(CredentialBackendError::OperationFailed),
        }
    }
}

#[cfg(not(windows))]
struct SystemCredentialBackend;

#[cfg(not(windows))]
impl SystemCredentialBackend {
    fn new(_service: &str, _account: &str) -> Self {
        Self
    }
}

#[cfg(not(windows))]
impl CredentialBackend for SystemCredentialBackend {
    fn check_available(&self) -> Result<(), &'static str> {
        Err("The system credential store is unavailable in this session.")
    }

    fn get(&self) -> Result<Option<String>, CredentialBackendError> {
        Err(CredentialBackendError::Unavailable)
    }

    fn set(&self, _api_key: &str) -> Result<(), CredentialBackendError> {
        Err(CredentialBackendError::Unavailable)
    }

    fn clear(&self) -> Result<(), CredentialBackendError> {
        Err(CredentialBackendError::Unavailable)
    }
}

struct AzureFoundryClientInner<T> {
    base_url: Url,
    transport: T,
}

impl<T> AzureFoundryClientInner<T>
where
    T: Transport,
{
    fn new(base_url: Url, transport: T) -> Self {
        Self {
            base_url,
            transport,
        }
    }

    async fn complete_chat(
        &self,
        api_key: &str,
        request: ChatCompletionRequest,
        options: RequestOptions,
    ) -> Result<ChatCompletionResponse, AiError> {
        validate_request(api_key, &request)?;
        let api_key = api_key.to_owned();
        run_with_request_controls(options, async {
            let response = self
                .transport
                .send(TransportRequest {
                    url: self.endpoint("chat/completions")?,
                    api_key: api_key.clone(),
                    body: chat_request_body(&request, false),
                    stream: false,
                })
                .await
                .map_err(|error| AiError::Transport(redact_secret(&error.message, &api_key)))?;

            let request_id = request_id(&response.headers);
            if response.status >= 400 {
                return Err(http_status_error(response, &api_key).await);
            }

            let body = read_body(response.body, &api_key).await?;
            parse_chat_completion_response(&body, request_id, &api_key)
        })
        .await
    }

    async fn stream_chat<F>(
        &self,
        api_key: &str,
        request: ChatCompletionRequest,
        options: RequestOptions,
        mut on_delta: F,
    ) -> Result<ChatCompletionResponse, AiError>
    where
        F: FnMut(StreamDelta) + Send,
    {
        validate_request(api_key, &request)?;
        let api_key = api_key.to_owned();
        run_with_request_controls(options, async {
            let response = self
                .transport
                .send(TransportRequest {
                    url: self.endpoint("chat/completions")?,
                    api_key: api_key.clone(),
                    body: chat_request_body(&request, true),
                    stream: true,
                })
                .await
                .map_err(|error| AiError::Transport(redact_secret(&error.message, &api_key)))?;

            let request_id = request_id(&response.headers);
            if response.status >= 400 {
                return Err(http_status_error(response, &api_key).await);
            }

            let mut body = response.body;
            let mut buffer = String::new();
            let mut aggregated = String::new();
            let mut model = None;
            let mut finish_reason = None;
            let mut saw_done = false;

            while let Some(chunk) = body.next().await {
                let chunk = chunk
                    .map_err(|error| AiError::Transport(redact_secret(&error.message, &api_key)))?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line_end) = buffer.find('\n') {
                    let mut line = buffer.drain(..=line_end).collect::<String>();
                    if line.ends_with('\n') {
                        line.pop();
                    }
                    if line.ends_with('\r') {
                        line.pop();
                    }
                    if line.is_empty() {
                        continue;
                    }
                    let Some(payload) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let payload = payload.trim_start();
                    if payload == "[DONE]" {
                        saw_done = true;
                        break;
                    }

                    let value: Value = serde_json::from_str(payload).map_err(|error| {
                        AiError::InvalidResponse(redact_secret(&error.to_string(), &api_key))
                    })?;
                    if model.is_none() {
                        model = value
                            .get("model")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                    }
                    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
                        for choice in choices {
                            if finish_reason.is_none() {
                                finish_reason = choice
                                    .get("finish_reason")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned);
                            }
                            let delta = choice
                                .get("delta")
                                .and_then(|delta| delta.get("content"))
                                .map(extract_text)
                                .unwrap_or_default();
                            if !delta.is_empty() {
                                aggregated.push_str(&delta);
                                on_delta(StreamDelta {
                                    delta: delta.clone(),
                                });
                            }
                        }
                    }
                }

                if saw_done {
                    break;
                }
            }

            if !saw_done {
                return Err(AiError::StreamInterrupted);
            }

            Ok(ChatCompletionResponse {
                request_id,
                model,
                finish_reason,
                usage: None,
                output_text: aggregated,
            })
        })
        .await
    }

    fn endpoint(&self, suffix: &str) -> Result<Url, AiError> {
        self.base_url.join(suffix).map_err(|_| {
            AiError::InvalidBaseUrl(
                "BASE_URL could not be combined with the Azure Foundry chat endpoint.".to_owned(),
            )
        })
    }
}

trait Transport: Send + Sync {
    fn send(
        &self,
        request: TransportRequest,
    ) -> SendFuture<'_, Result<TransportResponse, TransportError>>;
}

struct TransportRequest {
    url: Url,
    api_key: String,
    body: Value,
    stream: bool,
}

struct TransportResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: BoxStream<'static, Result<Bytes, TransportError>>,
}

#[derive(Debug)]
struct TransportError {
    message: String,
}

impl TransportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone)]
struct ReqwestTransport {
    client: reqwest::Client,
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Transport for ReqwestTransport {
    fn send(
        &self,
        request: TransportRequest,
    ) -> SendFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            let mut builder = self
                .client
                .post(request.url)
                .header("api-key", request.api_key)
                .header("content-type", "application/json");
            if request.stream {
                builder = builder.header("accept", "text/event-stream");
            }
            let response = builder
                .json(&request.body)
                .send()
                .await
                .map_err(|error| TransportError::new(error.to_string()))?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        value.to_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            let body = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(|error| TransportError::new(error.to_string())))
                .boxed();
            Ok(TransportResponse {
                status,
                headers,
                body,
            })
        })
    }
}

fn normalize_base_url(base_url: &str) -> Result<Url, AiError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(AiError::InvalidBaseUrl(
            "BASE_URL must be a non-empty absolute URL.".to_owned(),
        ));
    }
    let mut url = Url::parse(trimmed).map_err(|_| {
        AiError::InvalidBaseUrl("BASE_URL must be a valid absolute URL.".to_owned())
    })?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(AiError::InvalidBaseUrl(
            "BASE_URL must use http or https.".to_owned(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AiError::InvalidBaseUrl(
            "BASE_URL must not include query parameters or fragments.".to_owned(),
        ));
    }

    let path = url.path().trim_end_matches('/');
    let normalized = if path.is_empty() || path == "/" {
        "/openai/v1".to_owned()
    } else if path.ends_with("/openai/v1") {
        path.to_owned()
    } else {
        format!("{path}/openai/v1")
    };

    url.set_path(&format!("{normalized}/"));
    Ok(url)
}

fn normalize_api_key(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn validate_request(api_key: &str, request: &ChatCompletionRequest) -> Result<(), AiError> {
    if normalize_api_key(Some(api_key)).is_none() {
        return Err(AiError::MissingProviderApiKey);
    }
    if request.model.trim().is_empty() {
        return Err(AiError::InvalidResponse(
            "the chat request requires a deployment name".to_owned(),
        ));
    }
    if request.messages.is_empty() {
        return Err(AiError::InvalidResponse(
            "the chat request requires at least one message".to_owned(),
        ));
    }
    Ok(())
}

fn chat_request_body(request: &ChatCompletionRequest, stream: bool) -> Value {
    let mut body = json!({
        "model": request.model,
        "messages": request.messages,
        "stream": stream,
    });
    if let Some(temperature) = request.temperature.clone() {
        body["temperature"] = Value::Number(temperature);
    }
    if let Some(max_output_tokens) = request.max_output_tokens {
        body["max_completion_tokens"] = Value::Number(max_output_tokens.into());
    }
    body
}

async fn run_with_request_controls<F, T>(options: RequestOptions, future: F) -> Result<T, AiError>
where
    F: Future<Output = Result<T, AiError>> + Send,
    T: Send,
{
    let cancellation = options.cancellation.clone();
    let operation = async move {
        if let Some(token) = cancellation {
            tokio::select! {
                _ = token.cancelled() => Err(AiError::RequestCancelled),
                result = future => result,
            }
        } else {
            future.await
        }
    };

    match tokio::time::timeout(options.timeout, operation).await {
        Ok(result) => result,
        Err(_) => Err(AiError::RequestTimedOut(options.timeout)),
    }
}

async fn read_body(
    mut body: BoxStream<'static, Result<Bytes, TransportError>>,
    api_key: &str,
) -> Result<String, AiError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk =
            chunk.map_err(|error| AiError::Transport(redact_secret(&error.message, api_key)))?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn http_status_error(response: TransportResponse, api_key: &str) -> AiError {
    let body = read_body(response.body, api_key)
        .await
        .unwrap_or_else(|error| error.to_string());
    AiError::InvalidHttpStatus {
        status: response.status,
        message: redact_secret(&extract_http_error_message(&body), api_key),
    }
}

fn extract_http_error_message(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "empty error body".to_owned();
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return value
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .or_else(|| value.get("message").and_then(Value::as_str))
            .or_else(|| value.get("detail").and_then(Value::as_str))
            .unwrap_or("the service rejected the request")
            .to_owned();
    }
    trimmed.to_owned()
}

fn parse_chat_completion_response(
    body: &str,
    request_id: Option<String>,
    api_key: &str,
) -> Result<ChatCompletionResponse, AiError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| AiError::InvalidResponse(redact_secret(&error.to_string(), api_key)))?;
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
    else {
        return Err(AiError::InvalidResponse(
            "the response did not include any choices".to_owned(),
        ));
    };

    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let usage = parse_usage(value.get("usage"));
    let output_text = choice
        .get("message")
        .and_then(|message| message.get("content"))
        .map(extract_text)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AiError::InvalidResponse(
                "the response did not include assistant message content".to_owned(),
            )
        })?;

    Ok(ChatCompletionResponse {
        request_id,
        model,
        finish_reason,
        usage,
        output_text,
    })
}

fn parse_usage(value: Option<&Value>) -> Option<ChatCompletionUsage> {
    let value = value?;
    Some(ChatCompletionUsage {
        prompt_tokens: parse_usage_field(value.get("prompt_tokens")),
        completion_tokens: parse_usage_field(value.get("completion_tokens")),
        total_tokens: parse_usage_field(value.get("total_tokens")),
    })
}

fn parse_usage_field(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn extract_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("content").and_then(Value::as_str))
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn request_id(headers: &[(String, String)]) -> Option<String> {
    headers.iter().find_map(|(name, value)| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "x-request-id" | "request-id" | "x-ms-request-id"
        )
        .then(|| value.clone())
    })
}

fn redact_secret(message: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        return message.to_owned();
    }
    message.replace(api_key, "[redacted]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::{sync::Arc, time::Duration};

    #[derive(Clone)]
    struct TestCredentialBackend {
        available: bool,
        get_result: Result<Option<String>, CredentialBackendError>,
        set_result: Result<(), CredentialBackendError>,
        clear_result: Result<(), CredentialBackendError>,
        stored: Arc<std::sync::Mutex<Option<String>>>,
    }

    impl TestCredentialBackend {
        fn available() -> Self {
            Self {
                available: true,
                get_result: Ok(None),
                set_result: Ok(()),
                clear_result: Ok(()),
                stored: Arc::new(std::sync::Mutex::new(None)),
            }
        }

        fn unavailable() -> Self {
            Self {
                available: false,
                get_result: Err(CredentialBackendError::Unavailable),
                set_result: Err(CredentialBackendError::Unavailable),
                clear_result: Err(CredentialBackendError::Unavailable),
                stored: Arc::new(std::sync::Mutex::new(None)),
            }
        }
    }

    impl CredentialBackend for TestCredentialBackend {
        fn check_available(&self) -> Result<(), &'static str> {
            if self.available {
                Ok(())
            } else {
                Err("The system credential store is unavailable in this session.")
            }
        }

        fn get(&self) -> Result<Option<String>, CredentialBackendError> {
            match &self.get_result {
                Ok(value) => Ok(value
                    .clone()
                    .or_else(|| self.stored.lock().ok().and_then(|value| value.clone()))),
                Err(error) => Err(*error),
            }
        }

        fn set(&self, api_key: &str) -> Result<(), CredentialBackendError> {
            self.set_result?;
            *self.stored.lock().expect("store lock") = Some(api_key.to_owned());
            Ok(())
        }

        fn clear(&self) -> Result<(), CredentialBackendError> {
            self.clear_result?;
            *self.stored.lock().expect("store lock") = None;
            Ok(())
        }
    }

    type TransportHandler = dyn Fn(TransportRequest) -> SendFuture<'static, Result<TransportResponse, TransportError>>
        + Send
        + Sync;

    #[derive(Clone)]
    struct SimulatedTransport {
        handler: Arc<TransportHandler>,
    }

    impl SimulatedTransport {
        fn new<F, Fut>(handler: F) -> Self
        where
            F: Fn(TransportRequest) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Result<TransportResponse, TransportError>> + Send + 'static,
        {
            Self {
                handler: Arc::new(move |request| Box::pin(handler(request))),
            }
        }
    }

    impl Transport for SimulatedTransport {
        fn send(
            &self,
            request: TransportRequest,
        ) -> SendFuture<'_, Result<TransportResponse, TransportError>> {
            (self.handler)(request)
        }
    }

    fn test_request() -> ChatCompletionRequest {
        ChatCompletionRequest::new(
            "deployment-name",
            vec![
                ChatMessage::system("You are concise."),
                ChatMessage::user("Say hello."),
            ],
        )
        .with_max_output_tokens(32)
    }

    fn test_client(transport: SimulatedTransport) -> AzureFoundryClientInner<SimulatedTransport> {
        AzureFoundryClientInner::new(
            normalize_base_url("https://example.services.ai.azure.com/").expect("test base URL"),
            transport,
        )
    }

    fn json_response(status: u16, value: Value) -> TransportResponse {
        TransportResponse {
            status,
            headers: vec![("x-request-id".to_owned(), "req-123".to_owned())],
            body: stream::iter([Ok(Bytes::from(value.to_string()))]).boxed(),
        }
    }

    fn sse_response(lines: Vec<Result<&'static str, TransportError>>) -> TransportResponse {
        TransportResponse {
            status: 200,
            headers: vec![("x-request-id".to_owned(), "req-stream".to_owned())],
            body: stream::iter(
                lines
                    .into_iter()
                    .map(|line| line.map(|value| Bytes::from(value.to_owned()))),
            )
            .boxed(),
        }
    }

    fn assert_secret_redacted(error: &AiError, secret: &str) {
        assert!(
            !error.to_string().contains(secret),
            "error leaked provider key: {error}"
        );
    }

    #[test]
    fn credential_status_reports_missing_key() {
        let store = ProviderCredentialStoreInner::new(TestCredentialBackend::available(), None);
        assert_eq!(
            store.status(),
            ProviderCredentialStatus {
                configured: false,
                source: CredentialSource::Missing,
                persistence: CredentialPersistence::None,
                secure_store_available: true,
                limitation: None,
            }
        );
    }

    #[test]
    fn credential_store_sets_reads_and_clears_keys() {
        let mut store = ProviderCredentialStoreInner::new(TestCredentialBackend::available(), None);

        let status = store
            .set_provider_api_key("super-secret".to_owned())
            .expect("set provider key");
        assert_eq!(status.source, CredentialSource::SystemSecureStore);
        assert_eq!(store.clone_api_key().as_deref(), Some("super-secret"));

        let cleared = store.clear_provider_api_key().expect("clear provider key");
        assert!(!cleared.configured);
        assert_eq!(cleared.source, CredentialSource::Missing);
    }

    #[test]
    fn unavailable_secure_store_falls_back_to_session_memory_with_limitation() {
        let mut store =
            ProviderCredentialStoreInner::new(TestCredentialBackend::unavailable(), None);

        let status = store
            .set_provider_api_key("session-only".to_owned())
            .expect("fallback to session memory");
        assert_eq!(status.source, CredentialSource::SessionMemory);
        assert_eq!(status.persistence, CredentialPersistence::Session);
        assert!(!status.secure_store_available);
        assert!(status.limitation.is_some());
    }

    #[test]
    fn provider_key_bootstraps_from_environment_without_persisting() {
        let store = ProviderCredentialStoreInner::new(
            TestCredentialBackend::available(),
            Some("bootstrapped".to_owned()),
        );
        let status = store.status();
        assert_eq!(status.source, CredentialSource::SessionEnvironment);
        assert_eq!(status.persistence, CredentialPersistence::Session);
        assert_eq!(store.clone_api_key().as_deref(), Some("bootstrapped"));
    }

    #[tokio::test]
    async fn missing_provider_key_short_circuits_before_transport() {
        let client = test_client(SimulatedTransport::new(|_| async {
            panic!("transport must not run when the key is missing");
        }));

        let error = client
            .complete_chat("", test_request(), RequestOptions::default())
            .await
            .expect_err("missing key must fail");
        assert!(matches!(error, AiError::MissingProviderApiKey));
    }

    #[tokio::test]
    async fn completes_chat_successfully() {
        let client = test_client(SimulatedTransport::new(|request| async move {
            assert!(!request.stream);
            assert!(
                request
                    .url
                    .as_str()
                    .ends_with("/openai/v1/chat/completions")
            );
            Ok(json_response(
                200,
                json!({
                    "model": "gpt-5.6-terra",
                    "choices": [
                        {
                            "finish_reason": "stop",
                            "message": { "content": "Hello from Azure Foundry." }
                        }
                    ]
                }),
            ))
        }));

        let response = client
            .complete_chat(
                "super-secret-key",
                test_request(),
                RequestOptions::new(Duration::from_secs(1)),
            )
            .await
            .expect("chat completion succeeds");

        assert_eq!(response.request_id.as_deref(), Some("req-123"));
        assert_eq!(response.model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert_eq!(response.output_text, "Hello from Azure Foundry.");
    }

    #[tokio::test]
    async fn streams_chat_successfully() {
        let client = test_client(SimulatedTransport::new(|request| async move {
            assert!(request.stream);
            Ok(sse_response(vec![
                Ok(
                    "data: {\"model\":\"gpt-5.6-terra\",\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
                ),
                Ok(
                    "data: {\"choices\":[{\"delta\":{\"content\":\" world\"},\"finish_reason\":\"stop\"}]}\n\n",
                ),
                Ok("data: [DONE]\n\n"),
            ]))
        }));

        let mut deltas = Vec::new();
        let response = client
            .stream_chat(
                "super-secret-key",
                test_request(),
                RequestOptions::new(Duration::from_secs(1)),
                |delta| deltas.push(delta.delta),
            )
            .await
            .expect("stream succeeds");

        assert_eq!(deltas, vec!["Hello".to_owned(), " world".to_owned()]);
        assert_eq!(response.output_text, "Hello world");
        assert_eq!(response.model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    }

    #[tokio::test]
    async fn times_out_requests() {
        let client = test_client(SimulatedTransport::new(|_| async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(json_response(200, json!({ "choices": [] })))
        }));

        let error = client
            .complete_chat(
                "super-secret-key",
                test_request(),
                RequestOptions::new(Duration::from_millis(10)),
            )
            .await
            .expect_err("request must time out");
        assert!(matches!(error, AiError::RequestTimedOut(_)));
        assert_secret_redacted(&error, "super-secret-key");
    }

    #[tokio::test]
    async fn cancels_requests_explicitly() {
        let cancellation = CancellationToken::new();
        let cancel_after = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            cancel_after.cancel();
        });

        let client = test_client(SimulatedTransport::new(|_| async {
            tokio::time::sleep(Duration::from_millis(60)).await;
            Ok(json_response(200, json!({ "choices": [] })))
        }));

        let error = client
            .complete_chat(
                "super-secret-key",
                test_request(),
                RequestOptions::new(Duration::from_secs(1)).with_cancellation(cancellation),
            )
            .await
            .expect_err("request must be cancelled");
        assert!(matches!(error, AiError::RequestCancelled));
        assert_secret_redacted(&error, "super-secret-key");
    }

    #[tokio::test]
    async fn invalid_http_errors_are_sanitized() {
        let secret = "super-secret-key";
        let client = test_client(SimulatedTransport::new(move |_| {
            let leaked = secret.to_owned();
            async move {
                Ok(json_response(
                    422,
                    json!({ "message": format!("invalid request for {leaked}") }),
                ))
            }
        }));

        let error = client
            .complete_chat(secret, test_request(), RequestOptions::default())
            .await
            .expect_err("HTTP error must fail");
        assert!(matches!(
            error,
            AiError::InvalidHttpStatus { status: 422, .. }
        ));
        assert_secret_redacted(&error, secret);
    }

    #[tokio::test]
    async fn interrupted_streams_report_an_error() {
        let secret = "super-secret-key";
        let client = test_client(SimulatedTransport::new(|_| async {
            Ok(sse_response(vec![Ok(
                "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n",
            )]))
        }));

        let error = client
            .stream_chat(secret, test_request(), RequestOptions::default(), |_| {})
            .await
            .expect_err("interrupted stream must fail");
        assert!(matches!(error, AiError::StreamInterrupted));
        assert_secret_redacted(&error, secret);
    }

    #[tokio::test]
    #[ignore = "requires BASE_URL, PROVIDER_API_KEY and AZURE_FOUNDRY_MODEL"]
    async fn live_smoke_test() {
        let base_url = env::var("BASE_URL").expect("BASE_URL");
        let api_key = env::var("PROVIDER_API_KEY").expect("PROVIDER_API_KEY");
        let model = env::var("AZURE_FOUNDRY_MODEL").expect("AZURE_FOUNDRY_MODEL");

        let client = AzureFoundryClient::new(&base_url).expect("create live client");
        let response = client
            .complete_chat(
                &api_key,
                ChatCompletionRequest::new(
                    model,
                    vec![
                        ChatMessage::system("Reply in one short sentence."),
                        ChatMessage::user("Say hello in English."),
                    ],
                )
                .with_temperature(0.0)
                .with_max_output_tokens(32),
                RequestOptions::new(Duration::from_secs(60)),
            )
            .await
            .expect("live call succeeds");

        println!(
            "smoke success model={}",
            response.model.as_deref().unwrap_or("unknown")
        );
        assert!(!response.output_text.trim().is_empty());
    }
}
