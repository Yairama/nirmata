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
