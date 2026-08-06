
use super::*;

#[test]
fn rejects_inverted_period() {
    assert_eq!(
        Relation::new(
            WorldId::new(),
            EntityId::new(),
            EntityId::new(),
            "allied_with",
            RelationDirection::Undirected,
            Some(10),
            Some(9),
            Certainty::Certain,
            None,
            "{}",
        ),
        Err(DomainError::InvalidPeriod)
    );
}
