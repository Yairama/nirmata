
use super::*;

#[test]
fn distinguishes_semantic_and_coded_rules() {
    let semantic = Rule::new(
        WorldId::new(),
        RuleKind::Institutional,
        "Every oath has a price.",
        "kingdom",
        RuleSeverity::Advisory,
        None,
        None,
        r#"{"review":"semantic"}"#,
        1,
    )
    .expect("semantic rule");
    assert!(!semantic.can_produce_hard_error());

    let coded = Rule::new(
        WorldId::new(),
        RuleKind::Constitutive,
        "The dead do not return.",
        "world",
        RuleSeverity::Hard,
        None,
        Some(RuleValidatorKind::NoResurrection),
        "{}",
        1,
    )
    .expect("coded rule");
    assert!(coded.can_produce_hard_error());
}

#[test]
fn rejects_unknown_enums_and_invalid_validator_parameters() {
    assert!(serde_json::from_str::<RuleKind>(r#""physical""#).is_err());
    assert!(serde_json::from_str::<RuleSeverity>(r#""fatal""#).is_err());

    assert_eq!(
        Rule::new(
            WorldId::new(),
            RuleKind::Constitutive,
            "The dead do not return.",
            "world",
            RuleSeverity::Hard,
            None,
            Some(RuleValidatorKind::NoResurrection),
            r#"{"days":1}"#,
            1,
        ),
        Err(DomainError::InvalidRuleValidatorParameters {
            validator: "no_resurrection"
        })
    );
}
