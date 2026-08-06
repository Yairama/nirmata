use super::*;

#[test]
fn creates_valid_world() {
    let world = World::new("Arcadia", "A world of floating cities.", "First Dawn", 42)
        .expect("valid world");

    assert_eq!(world.name(), "Arcadia");
    assert_eq!(world.created_at_ms(), 42);
    assert_eq!(world.updated_at_ms(), 42);
}

#[test]
fn rejects_empty_world_name() {
    assert_eq!(
        World::new(" \n ", "", "", 42),
        Err(DomainError::EmptyWorldName)
    );
}

#[test]
fn serializes_round_trip() {
    let world = World::new("Arcadia", "# Premise", "First Dawn", 42).expect("valid world");
    let json = serde_json::to_string(&world).expect("serialize world");
    let restored: World = serde_json::from_str(&json).expect("deserialize world");

    assert_eq!(restored, world);
}
