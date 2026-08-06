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

fn test_request() -> ResponseRequest {
    ResponseRequest::new("deployment-name", "You are concise.", "Say hello.")
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
    let mut store = ProviderCredentialStoreInner::new(TestCredentialBackend::unavailable(), None);

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
        .create_response("", test_request(), RequestOptions::default())
        .await
        .expect_err("missing key must fail");
    assert!(matches!(error, AiError::MissingProviderApiKey));
}

#[tokio::test]
async fn creates_response_successfully() {
    let client = test_client(SimulatedTransport::new(|request| async move {
        assert!(!request.stream);
        assert!(request.url.as_str().ends_with("/openai/v1/responses"));
        assert_eq!(request.body["instructions"], "You are concise.");
        assert_eq!(request.body["input"], "Say hello.");
        assert_eq!(request.body["max_output_tokens"], 32);
        assert_eq!(request.body["store"], false);
        Ok(json_response(
            200,
            json!({
                "model": "gpt-5.6-terra",
                "status": "completed",
                "usage": { "input_tokens": 7, "output_tokens": 5, "total_tokens": 12 },
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "Hello from Azure Foundry."
                    }]
                }]
            }),
        ))
    }));

    let response = client
        .create_response(
            "super-secret-key",
            test_request(),
            RequestOptions::new(Duration::from_secs(1)),
        )
        .await
        .expect("response succeeds");

    assert_eq!(response.request_id.as_deref(), Some("req-123"));
    assert_eq!(response.model.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(response.status.as_deref(), Some("completed"));
    assert_eq!(response.output_text, "Hello from Azure Foundry.");
    assert_eq!(
        response.usage,
        Some(ResponseUsage {
            input_tokens: Some(7),
            output_tokens: Some(5),
            total_tokens: Some(12),
        })
    );
}

#[tokio::test]
async fn streams_response_successfully() {
    let client = test_client(SimulatedTransport::new(|request| async move {
        assert!(request.stream);
        Ok(sse_response(vec![
            Ok("data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\n"),
            Ok("data: {\"type\":\"response.output_text.delta\",\"delta\":\" world\"}\n\n"),
            Ok(
                "data: {\"type\":\"response.completed\",\"response\":{\"model\":\"gpt-5.6-terra\",\"status\":\"completed\",\"usage\":{\"input_tokens\":7,\"output_tokens\":2,\"total_tokens\":9}}}\n\n",
            ),
        ]))
    }));

    let mut deltas = Vec::new();
    let response = client
        .stream_response(
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
    assert_eq!(response.status.as_deref(), Some("completed"));
    assert_eq!(response.usage.expect("stream usage").total_tokens, Some(9));
}

#[tokio::test]
async fn times_out_requests() {
    let client = test_client(SimulatedTransport::new(|_| async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(json_response(200, json!({ "output": [] })))
    }));

    let error = client
        .create_response(
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
        Ok(json_response(200, json!({ "output": [] })))
    }));

    let error = client
        .create_response(
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
        .create_response(secret, test_request(), RequestOptions::default())
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
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
        )]))
    }));

    let error = client
        .stream_response(secret, test_request(), RequestOptions::default(), |_| {})
        .await
        .expect_err("interrupted stream must fail");
    assert!(matches!(error, AiError::StreamInterrupted));
    assert_secret_redacted(&error, secret);
}

#[tokio::test]
#[ignore = "requires BASE_URL, PROVIDER_API_KEY and a model environment variable"]
async fn live_smoke_test() {
    let base_url = env::var("BASE_URL").expect("BASE_URL");
    let api_key = env::var("PROVIDER_API_KEY").expect("PROVIDER_API_KEY");
    let model = env::var("AZURE_FOUNDRY_MODEL")
        .or_else(|_| env::var("GPT-5.6-SOL"))
        .expect("AZURE_FOUNDRY_MODEL or GPT-5.6-SOL");

    let client = AzureFoundryClient::new(&base_url).expect("create live client");
    let response = client
        .create_response(
            &api_key,
            ResponseRequest::new(
                model,
                "Reply in one short sentence.",
                "Say hello in English.",
            )
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

    let mut deltas = Vec::new();
    let streamed = client
        .stream_response(
            &api_key,
            ResponseRequest::new(
                response.model.as_deref().unwrap_or("gpt-5.6-sol"),
                "Reply with exactly one short word.",
                "Say hello in English.",
            )
            .with_max_output_tokens(32),
            RequestOptions::new(Duration::from_secs(60)),
            |delta| deltas.push(delta.delta),
        )
        .await
        .expect("live stream succeeds");
    assert!(!streamed.output_text.trim().is_empty());
    assert_eq!(deltas.concat(), streamed.output_text);
}
