use super::*;
use bytes::Bytes;
use futures_util::{FutureExt, StreamExt, stream, stream::BoxStream};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::TransportError;

type SendFuture<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type TransportHandler = dyn Fn(
        crate::TransportRequest,
    ) -> SendFuture<'static, Result<crate::TransportResponse, TransportError>>
    + Send
    + Sync;

#[derive(Clone)]
struct SimulatedTransport {
    handler: Arc<TransportHandler>,
}

impl SimulatedTransport {
    fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(crate::TransportRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<crate::TransportResponse, TransportError>> + Send + 'static,
    {
        Self {
            handler: Arc::new(move |request| Box::pin(handler(request))),
        }
    }
}

impl Transport for SimulatedTransport {
    fn send(
        &self,
        request: crate::TransportRequest,
    ) -> SendFuture<'_, Result<crate::TransportResponse, TransportError>> {
        (self.handler)(request)
    }
}

#[derive(Serialize)]
struct TestPayload {
    mode: &'static str,
    base_revision: &'static str,
    context_object_ids: Vec<&'static str>,
}

fn test_client(transport: SimulatedTransport) -> CapabilityClientInner<SimulatedTransport> {
    CapabilityClientInner::with_client(
        AzureFoundryClientInner::new(
            crate::normalize_base_url("https://example.services.ai.azure.com/")
                .expect("test base url"),
            transport,
        ),
        "super-secret-key",
        "gpt-5.6-terra",
    )
}

fn json_response(body: Value) -> crate::TransportResponse {
    crate::TransportResponse {
        status: 200,
        headers: vec![("x-request-id".to_owned(), "req-cap".to_owned())],
        body: body_stream(body),
    }
}

fn body_stream(body: Value) -> BoxStream<'static, Result<Bytes, TransportError>> {
    stream::iter([Ok(Bytes::from(body.to_string()))]).boxed()
}

#[tokio::test]
async fn query_rejects_change_set_output() {
    let client = test_client(SimulatedTransport::new(|_| async {
        Ok(json_response(json!({
            "model": "gpt-5.6-terra",
            "usage": { "prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30 },
            "choices": [{
                "finish_reason": "stop",
                "message": {
                    "content": include_str!("../fixtures/ai_contracts/change_set_valid.json")
                }
            }]
        })))
    }));

    let error = client
        .query(
            &TestPayload {
                mode: "query",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/1"],
            },
            vec!["nirmata://entity/1".to_owned()],
            RequestOptions::new(Duration::from_secs(1)),
        )
        .await
        .expect_err("query should reject change sets");

    let CapabilityError::StructuredOutput(error) = error else {
        panic!("unexpected error: {error}");
    };
    assert_eq!(
        error.kind(),
        crate::contracts::StructuredOutputErrorKind::InvalidShape
    );
}

#[tokio::test]
async fn query_streaming_aggregates_deltas_and_parses_output() {
    let client = test_client(SimulatedTransport::new(|request| async move {
        assert!(request.body["stream"].as_bool().expect("stream flag"));
        let second_content = r#"{"itemId":"item-1","classification":"fact","answer":{"markdown":"Mara guarda la puerta.","contentReferences":["nirmata://entity/11111111-1111-1111-1111-111111111111"]},"citations":[{"sourceUri":"nirmata://entity/11111111-1111-1111-1111-111111111111","quoteMd":"Mara guarda la puerta."}]}"#;
        Ok(crate::TransportResponse {
            status: 200,
            headers: vec![("x-request-id".to_owned(), "req-stream".to_owned())],
            body: stream::iter([
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "model": "gpt-5.6-terra",
                        "choices": [{
                            "delta": { "content": "{\"items\":[" },
                            "finish_reason": null
                        }]
                    })
                ))),
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "choices": [{
                            "delta": { "content": second_content },
                            "finish_reason": null
                        }]
                    })
                ))),
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "choices": [{
                            "delta": { "content": "]}" },
                            "finish_reason": "stop"
                        }]
                    })
                ))),
                Ok(Bytes::from("data: [DONE]\n\n")),
            ])
            .boxed(),
        })
    }));

    let mut deltas = Vec::new();
    let result = client
        .query_streaming(
            &TestPayload {
                mode: "query",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/1"],
            },
            vec!["nirmata://entity/1".to_owned()],
            RequestOptions::new(Duration::from_secs(1)),
            |delta| deltas.push(delta.delta),
        )
        .await
        .expect("streaming query succeeds");

    assert_eq!(result.output.items.len(), 1);
    assert_eq!(
        result.output.items[0].classification,
        crate::contracts::AdvisoryClassification::Fact
    );
    assert_eq!(result.metadata.status, InvocationStatus::Completed);
    assert!(result.metadata.usage.is_none());
    assert_eq!(deltas.len(), 3);
    assert!(deltas.join("").contains("\"itemId\":\"item-1\""));
}

#[tokio::test]
async fn propose_request_omits_write_capabilities() {
    let captured = Arc::new(Mutex::new(None::<Value>));
    let sink = captured.clone();
    let client = test_client(SimulatedTransport::new(move |request| {
        let sink = sink.clone();
        async move {
            *sink.lock().expect("capture lock") = Some(request.body.clone());
            Ok(json_response(json!({
                "model": "gpt-5.6-terra",
                "usage": { "prompt_tokens": 12, "completion_tokens": 24, "total_tokens": 36 },
                "choices": [{
                    "finish_reason": "stop",
                    "message": {
                        "content": include_str!("../fixtures/ai_contracts/change_set_valid.json")
                    }
                }]
            })))
        }
        .boxed()
    }));

    let result = client
        .propose(
            &TestPayload {
                mode: "propose",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/1"],
            },
            vec!["nirmata://entity/1".to_owned()],
            RequestOptions::new(Duration::from_secs(1)),
        )
        .await
        .expect("proposal succeeds");

    let body = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured request body");
    assert!(body.get("tools").is_none());
    assert!(body.get("functions").is_none());
    let messages = body["messages"].as_array().expect("messages array");
    assert_eq!(messages.len(), 2);
    assert!(
        messages[0]["content"]
            .as_str()
            .expect("system prompt")
            .contains("change_set_draft")
    );
    assert!(
        !messages[0]["content"]
            .as_str()
            .expect("system prompt")
            .contains("critique_report")
    );
    assert_eq!(result.metadata.prompt_version, PROPOSE_PROMPT_VERSION);
    assert_eq!(result.metadata.status, InvocationStatus::Completed);
    assert_eq!(
        result.metadata.usage,
        Some(ChatCompletionUsage {
            prompt_tokens: Some(12),
            completion_tokens: Some(24),
            total_tokens: Some(36),
        })
    );
}

#[tokio::test]
async fn critic_uses_a_dedicated_prompt() {
    let captured = Arc::new(Mutex::new(None::<Value>));
    let sink = captured.clone();
    let client = test_client(SimulatedTransport::new(move |request| {
        let sink = sink.clone();
        async move {
            *sink.lock().expect("capture lock") = Some(request.body.clone());
            Ok(json_response(json!({
                "model": "gpt-5.6-terra",
                "choices": [{
                    "finish_reason": "stop",
                    "message": {
                        "content": include_str!("../fixtures/ai_contracts/critique_valid.json")
                    }
                }]
            })))
        }
        .boxed()
    }));

    let result = client
        .critic(
            &TestPayload {
                mode: "critic",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/1"],
            },
            vec!["nirmata://entity/1".to_owned()],
            RequestOptions::new(Duration::from_secs(1)),
        )
        .await
        .expect("critique succeeds");

    let body = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured request body");
    let messages = body["messages"].as_array().expect("messages array");
    assert!(
        messages[0]["content"]
            .as_str()
            .expect("system prompt")
            .contains("critique_report")
    );
    assert!(
        !messages[0]["content"]
            .as_str()
            .expect("system prompt")
            .contains("change_set_draft")
    );
    assert_eq!(result.metadata.prompt_version, CRITIC_PROMPT_VERSION);
    assert_eq!(result.output.issues.len(), 1);
}
