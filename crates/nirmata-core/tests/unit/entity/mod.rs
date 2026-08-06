use super::*;

#[test]
fn normalizes_aliases_and_preserves_identity_when_renamed() {
    let mut entity = Entity::new(
        WorldId::new(),
        EntityKind::Person,
        "Mara",
        "mara",
        "",
        "",
        "{}",
        vec!["  The Cartographer  ".to_owned()],
        1,
    )
    .expect("valid entity");
    let id = entity.id();

    assert_eq!(entity.aliases(), ["The Cartographer"]);
    entity.rename("Mara Vale", "mara-vale", 2).expect("rename");
    assert_eq!(entity.id(), id);
    assert_eq!(entity.name(), "Mara Vale");
    assert_eq!(entity.version(), 2);
}

#[test]
fn rejects_empty_duplicate_aliases_and_invalid_json() {
    let duplicate = Entity::new(
        WorldId::new(),
        EntityKind::Person,
        "Mara",
        "mara",
        "",
        "",
        "{}",
        vec!["Witness".to_owned(), " witness ".to_owned()],
        1,
    );
    assert!(matches!(duplicate, Err(DomainError::DuplicateAlias(_))));

    let empty = Entity::new(
        WorldId::new(),
        EntityKind::Person,
        "",
        "mara",
        "",
        "",
        "{}",
        vec![],
        1,
    );
    assert_eq!(empty, Err(DomainError::EmptyField { field: "name" }));

    let invalid_json = Entity::new(
        WorldId::new(),
        EntityKind::Person,
        "Mara",
        "mara",
        "",
        "",
        "[]",
        vec![],
        1,
    );
    assert_eq!(
        invalid_json,
        Err(DomainError::InvalidJsonObject {
            field: "attributes_json"
        })
    );
}
