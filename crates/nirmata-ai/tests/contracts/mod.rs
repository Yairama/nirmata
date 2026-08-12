use super::*;

#[test]
fn rejects_free_text_mutation_payloads() {
    let error = parse_change_set_draft("Add the queen back to the throne.")
        .expect_err("free text should fail");

    assert_eq!(error.kind(), StructuredOutputErrorKind::FreeTextMutation);
    assert_eq!(error.diagnostic().starts_with, Some('A'));
    assert_eq!(error.diagnostic().ends_with, Some('.'));
    assert!(!error.diagnostic().looks_like_json_object);
}

#[test]
fn accepts_no_evidence_without_content_references() {
    let response = parse_advisory_response(
        r#"{
                "items": [
                    {
                        "itemId": "no-evidence-1",
                        "classification": "no_evidence",
                        "answer": {
                            "markdown": "No hay evidencia recuperada en este contexto.",
                            "contentReferences": []
                        },
                        "citations": []
                    }
                ]
            }"#,
    )
    .expect("no_evidence without content references should remain valid");

    assert!(response.items[0].answer.content_references.is_empty());
    assert!(response.items[0].citations.is_empty());
}

fn valid_specialist_report_json() -> serde_json::Value {
    serde_json::json!({
        "specialist": "economist",
        "sources": ["nirmata://entity/11111111-1111-1111-1111-111111111111"],
        "findings": [{
            "findingId": "resource-shortage",
            "summary": {
                "markdown": "La escasez reduce el comercio.",
                "contentReferences": ["nirmata://entity/11111111-1111-1111-1111-111111111111"]
            },
            "affectedObjectUris": ["nirmata://entity/11111111-1111-1111-1111-111111111111"],
            "candidateConsequences": [{
                "markdown": "Los precios locales aumentan.",
                "contentReferences": ["nirmata://entity/11111111-1111-1111-1111-111111111111"]
            }],
            "assumptions": ["La ruta alternativa sigue cerrada."],
            "evidence": [{
                "sourceUri": "nirmata://entity/11111111-1111-1111-1111-111111111111",
                "excerptMd": "La ciudad depende de una sola mina."
            }],
            "confidence": 0.8,
            "unresolvedQuestions": ["¿Existe una reserva estratégica?"],
            "decisionPosition": null
        }]
    })
}

#[test]
fn specialist_report_is_strict_grounded_and_round_trips() {
    let value = valid_specialist_report_json();
    let report = parse_specialist_report(&value.to_string()).expect("valid specialist report");
    let round_trip = serde_json::from_value::<SpecialistReport>(
        serde_json::to_value(&report).expect("serialize specialist report"),
    )
    .expect("deserialize specialist report");
    assert_eq!(round_trip, report);

    let mut unknown = value.clone();
    unknown["operations"] = serde_json::json!([]);
    assert_eq!(
        parse_specialist_report(&unknown.to_string())
            .expect_err("specialists cannot return operations")
            .kind(),
        StructuredOutputErrorKind::InvalidShape
    );

    let mut missing_evidence = value;
    missing_evidence["findings"][0]["evidence"] = serde_json::json!([]);
    assert_eq!(
        parse_specialist_report(&missing_evidence.to_string())
            .expect_err("evidence is mandatory")
            .kind(),
        StructuredOutputErrorKind::InvalidContent
    );
}

#[test]
fn deep_synthesis_round_trips_and_requires_origins_for_every_operation() {
    let draft: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/ai_contracts/change_set_valid.json"
    ))
    .expect("valid draft fixture");
    let operation_id = draft["operations"][0]["create_entity"]["operation_id"].clone();
    let value = serde_json::json!({
        "draft": draft,
        "operationOrigins": [{
            "operationId": operation_id,
            "findingIds": ["resource-shortage"]
        }],
        "decisionOrigins": []
    });
    let synthesis = parse_deep_synthesis(&value.to_string()).expect("valid synthesis");
    let round_trip = serde_json::from_value::<DeepSynthesis>(
        serde_json::to_value(&synthesis).expect("serialize synthesis"),
    )
    .expect("deserialize synthesis");
    assert_eq!(round_trip, synthesis);

    let mut without_origin = value;
    without_origin["operationOrigins"] = serde_json::json!([]);
    assert_eq!(
        parse_deep_synthesis(&without_origin.to_string())
            .expect_err("every operation needs a finding origin")
            .kind(),
        StructuredOutputErrorKind::InvalidContent
    );
}

#[test]
fn standard_contract_still_rejects_deep_profile_fields() {
    let error = parse_advisory_response(&valid_specialist_report_json().to_string())
        .expect_err("standard query contract must not accept specialist reports");
    assert_eq!(error.kind(), StructuredOutputErrorKind::InvalidShape);
}

#[test]
fn import_extraction_requires_literal_hash_bound_chunk_citations_and_keeps_opposition() {
    let hash = format!("sha256:{}", "a".repeat(64));
    let value = serde_json::json!({
        "candidates": [
            {
                "kind": "claim",
                "candidateId": "gate-open",
                "subjectName": "Mara",
                "contentMd": "The gate is open.",
                "predicateKey": "gate.open",
                "objectScalar": "true",
                "polarity": "positive",
                "authentication": "canonical",
                "contradictionKey": "gate-state",
                "citations": [{
                    "chunkId": "chunk-a",
                    "sourceId": "source-a",
                    "sourceHash": hash,
                    "excerpt": "gate is open"
                }],
                "technicalConfidence": 0.8
            },
            {
                "kind": "claim",
                "candidateId": "gate-closed",
                "subjectName": "Mara",
                "contentMd": "The gate is not open.",
                "predicateKey": "gate.open",
                "objectScalar": "true",
                "polarity": "negative",
                "authentication": "canonical",
                "contradictionKey": "gate-state",
                "citations": [{
                    "chunkId": "chunk-b",
                    "sourceId": "source-a",
                    "sourceHash": format!("sha256:{}", "a".repeat(64)),
                    "excerpt": "gate is not open"
                }],
                "technicalConfidence": 0.7
            }
        ]
    });
    let extraction = parse_import_extraction(&value.to_string()).expect("valid import extraction");
    assert_eq!(extraction.candidates.len(), 2);
    assert_eq!(
        extraction.candidates[0].contradiction_key(),
        extraction.candidates[1].contradiction_key()
    );

    let mut missing_citation = value.clone();
    missing_citation["candidates"][0]["citations"] = serde_json::json!([]);
    assert_eq!(
        parse_import_extraction(&missing_citation.to_string())
            .expect_err("every candidate must cite a chunk")
            .kind(),
        StructuredOutputErrorKind::InvalidContent
    );

    let mut operation = value;
    operation["candidates"][0]["operations"] = serde_json::json!([]);
    assert_eq!(
        parse_import_extraction(&operation.to_string())
            .expect_err("import extraction cannot emit operations")
            .kind(),
        StructuredOutputErrorKind::InvalidShape
    );
}
