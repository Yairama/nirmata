use super::*;

#[test]
fn separates_canon_rumor_belief_and_hypothesis() {
    let world_id = WorldId::new();
    let subject = EntityId::new();
    let holder = EntityId::new();
    let revision = RevisionId::new();

    let canon = Claim::new(
        world_id,
        subject,
        "The gate is closed.",
        Some("gate.closed".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        revision,
    )
    .expect("canonical claim");
    assert_eq!(canon.authentication(), ClaimAuthentication::Canonical);

    for (modality, register) in [
        (ClaimModality::Belief, "rumor"),
        (ClaimModality::Belief, "myth"),
        (ClaimModality::Hypothesis, "testimony"),
    ] {
        let claim = Claim::new(
            world_id,
            subject,
            "The gate may be closed.",
            None,
            None,
            ClaimPolarity::Positive,
            ClaimAuthentication::Attributed,
            Some(holder),
            Some(modality),
            Some(register.to_owned()),
            None,
            None,
            None,
            None,
            Some(0.7),
            None,
            revision,
        )
        .expect("perspectival claim");
        assert_eq!(claim.holder_entity_id(), Some(holder));
    }
}

#[test]
fn rejects_invalid_authentication_and_self_provenance() {
    let claim_id = ClaimId::new();
    assert_eq!(
        Claim::restore(
            claim_id,
            WorldId::new(),
            EntityId::new(),
            "",
            None,
            None,
            ClaimPolarity::Negative,
            ClaimAuthentication::Attributed,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(claim_id),
            None,
            None,
            RevisionId::new(),
            None,
            1,
        ),
        Err(DomainError::InvalidClaimContext(
            "attributed claims require a holder and modality"
        ))
    );

    assert!(
        Claim::restore(
            claim_id,
            WorldId::new(),
            EntityId::new(),
            "",
            None,
            None,
            ClaimPolarity::Negative,
            ClaimAuthentication::Disputed,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(claim_id),
            None,
            None,
            RevisionId::new(),
            None,
            1,
        )
        .is_err()
    );
}
