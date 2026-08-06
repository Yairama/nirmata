use super::*;

#[test]
fn requires_desired_state_and_ordered_period() {
    assert_eq!(
        Goal::new(
            WorldId::new(),
            EntityId::new(),
            " ",
            1,
            GoalStatus::Active,
            None,
            GoalVisibility::Secret,
            None,
        ),
        Err(DomainError::EmptyField {
            field: "desired_state_md"
        })
    );
    assert_eq!(
        Period::new(Some(2), Some(1)),
        Err(DomainError::InvalidPeriod)
    );
}
