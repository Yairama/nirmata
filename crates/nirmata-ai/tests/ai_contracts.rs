use nirmata_ai::contracts::{
    AdvisoryClassification, StructuredOutputErrorKind, parse_advisory_response,
    parse_change_set_draft, parse_critique_report,
};

#[test]
fn parses_valid_advisory_fixture() {
    let response =
        parse_advisory_response(include_str!("fixtures/ai_contracts/advisory_valid.json"))
            .expect("valid advisory fixture");

    assert_eq!(response.items.len(), 1);
    assert_eq!(
        response.items[0].classification,
        AdvisoryClassification::Inference
    );
    assert_eq!(response.items[0].citations.len(), 1);
}

#[test]
fn rejects_advisory_fixture_with_unknown_field() {
    let error = parse_advisory_response(include_str!(
        "fixtures/ai_contracts/advisory_unknown_field.json"
    ))
    .expect_err("unknown field must fail");

    assert_eq!(error.kind(), StructuredOutputErrorKind::InvalidShape);
}

#[test]
fn rejects_advisory_fixture_without_content_references() {
    let error = parse_advisory_response(include_str!(
        "fixtures/ai_contracts/advisory_missing_references.json"
    ))
    .expect_err("missing content references must fail");

    assert_eq!(error.kind(), StructuredOutputErrorKind::InvalidContent);
}

#[test]
fn parses_valid_change_set_fixture() {
    let draft = parse_change_set_draft(include_str!("fixtures/ai_contracts/change_set_valid.json"))
        .expect("valid draft fixture");

    assert_eq!(draft.draft().operations().len(), 1);
    assert_eq!(draft.draft().decisions().len(), 0);
}

#[test]
fn rejects_change_set_fixture_with_unknown_operation() {
    let error = parse_change_set_draft(include_str!(
        "fixtures/ai_contracts/change_set_unknown_operation.json"
    ))
    .expect_err("unknown operation must fail");

    assert_eq!(error.kind(), StructuredOutputErrorKind::InvalidShape);
}

#[test]
fn rejects_truncated_change_set_fixture() {
    let error = parse_change_set_draft(include_str!(
        "fixtures/ai_contracts/change_set_truncated.json"
    ))
    .expect_err("truncated draft must fail");

    assert_eq!(error.kind(), StructuredOutputErrorKind::TruncatedJson);
}

#[test]
fn parses_valid_critique_fixture() {
    let report = parse_critique_report(include_str!("fixtures/ai_contracts/critique_valid.json"))
        .expect("valid critique fixture");

    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].evidence.len(), 1);
}

#[test]
fn rejects_critique_fixture_with_invalid_uri() {
    let error = parse_critique_report(include_str!(
        "fixtures/ai_contracts/critique_invalid_uri.json"
    ))
    .expect_err("invalid uri must fail");

    assert_eq!(error.kind(), StructuredOutputErrorKind::InvalidShape);
}
