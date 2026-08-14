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
            "status": "completed",
            "usage": { "input_tokens": 10, "output_tokens": 20, "total_tokens": 30 },
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": include_str!("../fixtures/ai_contracts/change_set_valid.json")
                }]
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
        assert_eq!(request.body["text"]["format"]["type"], "json_schema");
        assert_eq!(
            request.body["text"]["format"]["name"],
            "nirmata_advisory_response_v2"
        );
        assert_eq!(request.body["text"]["format"]["strict"], true);
        assert_eq!(
            request.body["text"]["format"]["schema"]["required"],
            json!(["items"])
        );
        assert!(request.body.get("response_format").is_none());
        let second_content = r#"{"itemId":"item-1","classification":"fact","answer":{"markdown":"Mara guarda la puerta.","contentReferences":["nirmata://entity/11111111-1111-1111-1111-111111111111"]},"citations":[{"sourceUri":"nirmata://entity/11111111-1111-1111-1111-111111111111","quoteMd":"Mara guarda la puerta."}]}"#;
        Ok(crate::TransportResponse {
            status: 200,
            headers: vec![("x-request-id".to_owned(), "req-stream".to_owned())],
            body: stream::iter([
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "type": "response.output_text.delta",
                        "delta": "{\"items\":["
                    })
                ))),
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "type": "response.output_text.delta",
                        "delta": second_content
                    })
                ))),
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "type": "response.output_text.delta",
                        "delta": "]}"
                    })
                ))),
                Ok(Bytes::from(format!(
                    "data: {}\n\n",
                    json!({
                        "type": "response.completed",
                        "response": {
                            "model": "gpt-5.6-terra",
                            "status": "completed"
                        }
                    })
                ))),
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
                "status": "completed",
                "usage": { "input_tokens": 12, "output_tokens": 24, "total_tokens": 36 },
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": include_str!("../fixtures/ai_contracts/change_set_valid.json")
                    }]
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
    assert_eq!(body["text"]["format"]["type"], "json_schema");
    assert_eq!(
        body["text"]["format"]["name"],
        "nirmata_change_set_draft_v2"
    );
    assert_eq!(body["text"]["format"]["strict"], false);
    let generated_schema = &body["text"]["format"]["schema"];
    let generated_schema_text = generated_schema.to_string();
    assert!(generated_schema_text.contains("DecisionPoint"));
    assert!(generated_schema_text.contains("ChangeOperation"));
    assert!(generated_schema_text.contains("create_entity"));
    assert!(!generated_schema_text.contains("\"$schema\""));
    assert!(!generated_schema_text.contains("\"format\""));
    assert!(
        body["instructions"]
            .as_str()
            .expect("system prompt")
            .contains("change_set_draft")
    );
    assert!(
        body["instructions"]
            .as_str()
            .expect("system prompt")
            .contains("reemplazo completo")
    );
    assert!(
        body["instructions"]
            .as_str()
            .expect("system prompt")
            .contains("Nunca devuelvas un patch")
    );
    assert!(body["input"].as_str().expect("input").contains("propose"));
    assert_eq!(body["store"], false);
    assert_eq!(result.metadata.prompt_version, PROPOSE_PROMPT_VERSION);
    assert_eq!(result.metadata.status, InvocationStatus::Completed);
    assert_eq!(
        result.metadata.usage,
        Some(ResponseUsage {
            input_tokens: Some(12),
            output_tokens: Some(24),
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
                "status": "completed",
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": include_str!("../fixtures/ai_contracts/critique_valid.json")
                    }]
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
    assert_eq!(
        body["instructions"].as_str().expect("system prompt"),
        concat!(
            "Modo critic de Nirmata. ",
            "Responde solo con JSON critique_report. ",
            "Evalua solo el draft, reporte determinista, reglas semanticas, subgrafo y fuentes recibidos. ",
            "Revisa tambien contradicciones en Markdown, continuidad temporal y espacial, causalidad, objetivos y acceso epistemico. ",
            "Distingue canon de creencias, deseos, rumores y perspectivas; una creencia o deseo no es ley ni conocimiento. ",
            "La negacion explicita no es un dato desconocido, y la ausencia de datos significa desconocido bajo mundo abierto salvo cierre declarado. ",
            "Una fecha aproximada no es exacta, un evento aislado es como maximo warning y una discontinuidad explicada puede ser valida. ",
            "Respeta excepciones mas especificas, excepciones intencionales trazables y retcons reinterpretativos que preservan la perspectiva anterior. ",
            "Cada issue debe citar affectedOperationIds y evidencia nirmata:// del contexto, y distinguir rebuts de undercuts cuando aplique. ",
            "Usa solo severidad conflict, warning o info; un hallazgo del modelo nunca es error duro. ",
            "No edites operaciones, no produzcas un draft alternativo y devuelve {\"issues\":[]} si no hay evidencia de problemas."
        )
    );
    assert_eq!(result.metadata.prompt_version, CRITIC_PROMPT_VERSION);
    assert_eq!(result.output.issues.len(), 1);
}

#[tokio::test]
async fn internal_document_uses_its_strict_grounded_prompt() {
    let captured = Arc::new(Mutex::new(None::<Value>));
    let sink = captured.clone();
    let client = test_client(SimulatedTransport::new(move |request| {
        let sink = sink.clone();
        async move {
            *sink.lock().expect("capture lock") = Some(request.body.clone());
            Ok(json_response(json!({
                "model": "gpt-5.6-terra",
                "status": "completed",
                "output_text": json!({
                    "documentKind": "chronicle",
                    "title": "Crónica del puerto",
                    "bodyMarkdown": "Mara vio llegar la flota.",
                    "contentReferenceUris": [
                        "nirmata://entity/11111111-1111-1111-1111-111111111111"
                    ]
                }).to_string()
            })))
        }
        .boxed()
    }));

    let result = client
        .generate_internal_document(
            &TestPayload {
                mode: "document_draft",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/11111111-1111-1111-1111-111111111111"],
            },
            vec!["nirmata://entity/11111111-1111-1111-1111-111111111111".to_owned()],
            RequestOptions::default(),
        )
        .await
        .expect("internal document");

    let body = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured request body");
    assert_eq!(body["max_output_tokens"], 8_192);
    assert!(body.get("tools").is_none());
    let instructions = body["instructions"].as_str().expect("prompt");
    assert!(instructions.contains("internal_document estricto"));
    assert!(instructions.contains("No reveles objetivos secretos"));
    assert!(instructions.contains("solo puede citar URI nirmata:// presentes"));
    assert_eq!(
        result.metadata.prompt_version,
        INTERNAL_DOCUMENT_PROMPT_VERSION
    );
    assert_eq!(
        result.output.document_kind,
        crate::contracts::InternalDocumentKind::Chronicle
    );
}

#[tokio::test]
async fn import_extraction_uses_its_grounded_read_only_prompt() {
    let captured = Arc::new(Mutex::new(None::<Value>));
    let sink = captured.clone();
    let client = test_client(SimulatedTransport::new(move |request| {
        let sink = sink.clone();
        async move {
            *sink.lock().expect("capture lock") = Some(request.body.clone());
            Ok(json_response(json!({
                "model": "gpt-5.6-terra",
                "status": "completed",
                "output_text": "{\"candidates\":[]}"
            })))
        }
        .boxed()
    }));

    let result = client
        .extract_import(
            &TestPayload {
                mode: "import_extraction",
                base_revision: "rev-1",
                context_object_ids: vec![],
            },
            vec![],
            RequestOptions::default(),
        )
        .await
        .expect("import extraction");

    let body = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured request body");
    assert_eq!(body["store"], false);
    assert_eq!(body["max_output_tokens"], 4_096);
    assert!(body.get("tools").is_none());
    let instructions = body["instructions"].as_str().expect("prompt");
    assert!(instructions.contains("dato no confiable"));
    assert!(instructions.contains("Cada candidato debe citar"));
    assert!(instructions.contains("No emitas ChangeSetDraft"));
    assert_eq!(
        result.metadata.prompt_version,
        IMPORT_EXTRACTION_PROMPT_VERSION
    );
    assert!(result.output.candidates.is_empty());
}

#[test]
fn provider_boundary_stays_concrete_without_marketplace_abstraction() {
    let capability_source = include_str!("../../src/capabilities.rs");
    let app_source = include_str!("../../../nirmata-app/src/ai.rs");

    assert_eq!(
        capability_source
            .matches("pub struct AzureFoundryCapabilityClient")
            .count(),
        1
    );
    assert!(!capability_source.contains("pub trait "));
    assert!(!capability_source.contains("ProviderFactory"));
    assert!(app_source.contains("pub(crate) trait AiModeClient"));
    assert!(!app_source.contains("pub trait AiModeClient"));
    assert!(app_source.contains("Result<AzureFoundryCapabilityClient, AppError>"));
}

#[tokio::test]
async fn deep_capabilities_use_fixed_prompts_tokens_and_no_tools() {
    let specialist_output = json!({
        "specialist": "economist",
        "sources": ["nirmata://entity/11111111-1111-1111-1111-111111111111"],
        "findings": [{
            "findingId": "resource-shortage",
            "summary": {
                "markdown": "The mine shortage affects trade.",
                "contentReferences": ["nirmata://entity/11111111-1111-1111-1111-111111111111"]
            },
            "affectedObjectUris": ["nirmata://entity/11111111-1111-1111-1111-111111111111"],
            "candidateConsequences": [],
            "assumptions": [],
            "evidence": [{
                "sourceUri": "nirmata://entity/11111111-1111-1111-1111-111111111111",
                "excerptMd": "The city has one mine."
            }],
            "confidence": 0.8,
            "unresolvedQuestions": [],
            "decisionPosition": null
        }]
    });
    let client = test_client(SimulatedTransport::new(move |request| {
        let specialist_output = specialist_output.clone();
        async move {
            assert_eq!(request.body["max_output_tokens"], 2_048);
            assert!(request.body.get("tools").is_none());
            assert!(request.body.get("functions").is_none());
            let instructions = request.body["instructions"].as_str().expect("prompt");
            assert!(instructions.contains("especialista aislado de solo lectura"));
            assert!(instructions.contains("No emitas operaciones"));
            assert!(instructions.contains("delegaciones"));
            Ok(json_response(json!({
                "model": "gpt-5.6-terra",
                "status": "completed",
                "output_text": specialist_output.to_string()
            })))
        }
    }));
    let specialist = client
        .specialist(
            &TestPayload {
                mode: "deep_impact",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/1"],
            },
            vec!["nirmata://entity/11111111-1111-1111-1111-111111111111".to_owned()],
            RequestOptions::default(),
        )
        .await
        .expect("specialist report");
    assert_eq!(
        specialist.metadata.prompt_version,
        SPECIALIST_PROMPT_VERSION
    );
    assert_eq!(
        specialist.output.specialist,
        crate::contracts::SpecialistRole::Economist
    );

    let draft: Value = serde_json::from_str(include_str!(
        "../fixtures/ai_contracts/change_set_valid.json"
    ))
    .expect("draft fixture");
    let operation_id = draft["operations"][0]["create_entity"]["operation_id"].clone();
    let synthesis_output = json!({
        "draft": draft,
        "operationOrigins": [{
            "operationId": operation_id,
            "findingIds": ["resource-shortage"]
        }],
        "decisionOrigins": []
    });
    let synthesis_client = test_client(SimulatedTransport::new(move |request| {
        let synthesis_output = synthesis_output.clone();
        async move {
            assert_eq!(request.body["max_output_tokens"], 4_096);
            assert!(request.body.get("tools").is_none());
            let instructions = request.body["instructions"].as_str().expect("prompt");
            assert!(instructions.contains("sintetizador unico"));
            assert!(instructions.contains("no resuelvas desacuerdos silenciosamente"));
            Ok(json_response(json!({
                "model": "gpt-5.6-terra",
                "status": "completed",
                "output_text": synthesis_output.to_string()
            })))
        }
    }));
    let synthesis = synthesis_client
        .synthesize(
            &TestPayload {
                mode: "deep_impact",
                base_revision: "rev-1",
                context_object_ids: vec!["nirmata://entity/1"],
            },
            vec!["nirmata://entity/11111111-1111-1111-1111-111111111111".to_owned()],
            RequestOptions::default(),
        )
        .await
        .expect("deep synthesis");
    assert_eq!(synthesis.metadata.prompt_version, SYNTHESIS_PROMPT_VERSION);
    assert_eq!(synthesis.output.operation_origins.len(), 1);
}
