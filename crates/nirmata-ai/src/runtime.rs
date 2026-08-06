use crate::{RequestOptions, ResponseRequest, ResponseResult, ResponseUsage, StreamDelta};
use bytes::Bytes;
use futures_util::{StreamExt, stream::BoxStream};
use reqwest::Url;
use serde::Serialize;
use serde_json::{Value, json};
use std::{env, error::Error, fmt, future::Future, pin::Pin, time::Duration};

#[cfg(test)]
use tokio_util::sync::CancellationToken;

#[cfg(windows)]
use keyring::{Entry, Error as KeyringError};

const DEFAULT_CREDENTIAL_SERVICE: &str = "nirmata";
const DEFAULT_CREDENTIAL_ACCOUNT: &str = "azure-foundry-api-key";
const ENV_PROVIDER_API_KEY: &str = "PROVIDER_API_KEY";

type SendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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

    pub async fn create_response(
        &self,
        api_key: &str,
        request: ResponseRequest,
        options: RequestOptions,
    ) -> Result<ResponseResult, AiError> {
        self.inner.create_response(api_key, request, options).await
    }

    pub async fn stream_response<F>(
        &self,
        api_key: &str,
        request: ResponseRequest,
        options: RequestOptions,
        on_delta: F,
    ) -> Result<ResponseResult, AiError>
    where
        F: FnMut(StreamDelta) + Send,
    {
        self.inner
            .stream_response(api_key, request, options, on_delta)
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

pub(crate) struct AzureFoundryClientInner<T> {
    base_url: Url,
    transport: T,
}

impl<T> AzureFoundryClientInner<T>
where
    T: Transport,
{
    pub(crate) fn new(base_url: Url, transport: T) -> Self {
        Self {
            base_url,
            transport,
        }
    }

    pub(crate) async fn create_response(
        &self,
        api_key: &str,
        request: ResponseRequest,
        options: RequestOptions,
    ) -> Result<ResponseResult, AiError> {
        validate_request(api_key, &request)?;
        let api_key = api_key.to_owned();
        run_with_request_controls(options, async {
            let response = self
                .transport
                .send(TransportRequest {
                    url: self.endpoint("responses")?,
                    api_key: api_key.clone(),
                    body: response_request_body(&request, false),
                    stream: false,
                })
                .await
                .map_err(|error| AiError::Transport(redact_secret(&error.message, &api_key)))?;

            let request_id = request_id(&response.headers);
            if response.status >= 400 {
                return Err(http_status_error(response, &api_key).await);
            }

            let body = read_body(response.body, &api_key).await?;
            parse_response_result(&body, request_id, &api_key)
        })
        .await
    }

    pub(crate) async fn stream_response<F>(
        &self,
        api_key: &str,
        request: ResponseRequest,
        options: RequestOptions,
        mut on_delta: F,
    ) -> Result<ResponseResult, AiError>
    where
        F: FnMut(StreamDelta) + Send,
    {
        validate_request(api_key, &request)?;
        let api_key = api_key.to_owned();
        run_with_request_controls(options, async {
            let response = self
                .transport
                .send(TransportRequest {
                    url: self.endpoint("responses")?,
                    api_key: api_key.clone(),
                    body: response_request_body(&request, true),
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
            let mut status = None;
            let mut incomplete_reason = None;
            let mut usage = None;
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
                    match value.get("type").and_then(Value::as_str) {
                        Some("response.output_text.delta") => {
                            let delta = value
                                .get("delta")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if !delta.is_empty() {
                                aggregated.push_str(delta);
                                on_delta(StreamDelta {
                                    delta: delta.to_owned(),
                                });
                            }
                        }
                        Some("response.completed") | Some("response.incomplete") => {
                            if let Some(final_response) = value.get("response") {
                                model = final_response
                                    .get("model")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned);
                                status = final_response
                                    .get("status")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned);
                                incomplete_reason = parse_incomplete_reason(final_response);
                                usage = parse_usage(final_response.get("usage"));
                            }
                            saw_done = true;
                            break;
                        }
                        Some("error") => {
                            let message = value
                                .get("error")
                                .and_then(|error| error.get("message"))
                                .and_then(Value::as_str)
                                .unwrap_or("the response stream failed");
                            return Err(AiError::InvalidResponse(redact_secret(message, &api_key)));
                        }
                        _ => {}
                    }
                }

                if saw_done {
                    break;
                }
            }

            if !saw_done {
                return Err(AiError::StreamInterrupted);
            }

            Ok(ResponseResult {
                request_id,
                model,
                status,
                incomplete_reason,
                usage,
                output_text: aggregated,
            })
        })
        .await
    }

    fn endpoint(&self, suffix: &str) -> Result<Url, AiError> {
        self.base_url.join(suffix).map_err(|_| {
            AiError::InvalidBaseUrl(
                "BASE_URL could not be combined with the Azure Foundry responses endpoint."
                    .to_owned(),
            )
        })
    }
}

pub(crate) trait Transport: Send + Sync {
    fn send(
        &self,
        request: TransportRequest,
    ) -> SendFuture<'_, Result<TransportResponse, TransportError>>;
}

pub(crate) struct TransportRequest {
    pub(crate) url: Url,
    pub(crate) api_key: String,
    pub(crate) body: Value,
    pub(crate) stream: bool,
}

pub(crate) struct TransportResponse {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: BoxStream<'static, Result<Bytes, TransportError>>,
}

#[derive(Debug)]
pub(crate) struct TransportError {
    pub(crate) message: String,
}

impl TransportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ReqwestTransport {
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
                .bearer_auth(request.api_key)
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

pub(crate) fn normalize_base_url(base_url: &str) -> Result<Url, AiError> {
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

fn validate_request(api_key: &str, request: &ResponseRequest) -> Result<(), AiError> {
    if normalize_api_key(Some(api_key)).is_none() {
        return Err(AiError::MissingProviderApiKey);
    }
    if request.model.trim().is_empty() {
        return Err(AiError::InvalidResponse(
            "the responses request requires a model or deployment name".to_owned(),
        ));
    }
    if request.input.trim().is_empty() {
        return Err(AiError::InvalidResponse(
            "the responses request requires non-empty input".to_owned(),
        ));
    }
    Ok(())
}

fn response_request_body(request: &ResponseRequest, stream: bool) -> Value {
    let mut body = json!({
        "model": request.model,
        "instructions": request.instructions,
        "input": request.input,
        "stream": stream,
        "store": false,
    });
    if let Some(max_output_tokens) = request.max_output_tokens {
        body["max_output_tokens"] = Value::Number(max_output_tokens.into());
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

fn parse_response_result(
    body: &str,
    request_id: Option<String>,
    api_key: &str,
) -> Result<ResponseResult, AiError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| AiError::InvalidResponse(redact_secret(&error.to_string(), api_key)))?;
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let incomplete_reason = parse_incomplete_reason(&value);
    let usage = parse_usage(value.get("usage"));
    let output_text = value
        .get("output_text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| extract_response_output_text(&value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AiError::InvalidResponse("the response did not include output text".to_owned())
        })?;

    Ok(ResponseResult {
        request_id,
        model,
        status,
        incomplete_reason,
        usage,
        output_text,
    })
}

fn parse_usage(value: Option<&Value>) -> Option<ResponseUsage> {
    let value = value?;
    Some(ResponseUsage {
        input_tokens: parse_usage_field(value.get("input_tokens")),
        output_tokens: parse_usage_field(value.get("output_tokens")),
        total_tokens: parse_usage_field(value.get("total_tokens")),
    })
}

fn parse_incomplete_reason(value: &Value) -> Option<String> {
    value
        .get("incomplete_details")
        .and_then(|details| details.get("reason"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn extract_response_output_text(value: &Value) -> Option<String> {
    let text = value
        .get("output")?
        .as_array()?
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|content| content.get("type").and_then(Value::as_str) == Some("output_text"))
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

fn parse_usage_field(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
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
#[path = "../tests/runtime/mod.rs"]
mod tests;
