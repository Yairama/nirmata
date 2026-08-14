use nirmata_app::{
    AppError, ContextBudget, ContextBundleRequest, ContextIntent, DraftOperationInput,
    EmptySearchClassification, ExportSnapshotInput, ImportSnapshotInput, ManualReviewActionRequest,
    ManualReviewFreshnessStatus, ManualReviewInput, NirmataApp, PendingReviewOrigin,
    RelatedContextRequest, SearchWorldRequest,
};
use nirmata_core::{
    EntityId, World,
    change_set::RetconKind,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
};
use nirmata_store::{CanonSnapshot, StructuredSearchKind, StructuredSearchQuery, WorldStore};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    project: PathBuf,
    parent: PathBuf,
    actor: Entity,
    disposable: Entity,
    document: DocumentAggregate,
}

fn fixture(label: &str) -> Fixture {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&root).expect("create test root");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let parent = root.join(format!(
        "snapshot-import-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&parent).expect("create fixture parent");
    let project = parent.join("world.nirmata");
    let world = World::new("Import World", "Original premise.", "Bell", 1).expect("world");
    let mut store = WorldStore::create(&project, &world).expect("create store");
    let actor = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Archivist",
        "Original entity body.",
        "{}",
        vec![],
        1,
    )
    .expect("actor");
    let disposable = Entity::new(
        world.id(),
        EntityKind::Concept,
        "Old Note",
        "old-note",
        "Unreferenced",
        "Can be deleted.",
        "{}",
        vec![],
        1,
    )
    .expect("disposable");
    store.insert_entity(&actor).expect("insert actor");
    store.insert_entity(&disposable).expect("insert disposable");
    let document = Document::new(
        world.id(),
        "Chronicle",
        "chronicle",
        Some(actor.id()),
        Some(actor.id()),
        DocumentCanonStatus::Canonical,
        "Original document body.",
        1,
    )
    .expect("document");
    let document_id = document.id();
    let document = DocumentAggregate::new(
        document,
        vec![ContentReference::new(
            ObjectRef::Document(document_id),
            ObjectRef::Entity(actor.id()),
            0,
        )],
    );
    store.insert_document(&document).expect("insert document");
    drop(store);
    Fixture {
        project,
        parent,
        actor,
        disposable,
        document,
    }
}

fn open_and_export(fixture: &Fixture, name: &str) -> (NirmataApp, PathBuf) {
    let mut app = NirmataApp::default();
    app.open_world(fixture.project.clone())
        .expect("open fixture");
    let result = app
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory: fixture.parent.clone(),
            snapshot_name: name.to_owned(),
        })
        .expect("export snapshot");
    (app, result.path)
}

fn manifest(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path.join("manifest.json")).expect("read manifest"))
        .expect("parse manifest")
}

fn object_index(manifest: &Value, uri: &str) -> usize {
    manifest["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .position(|object| object["uri"] == uri)
        .expect("object entry")
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn hash_value(value: &Value) -> String {
    hash_bytes(&serde_json::to_vec(value).expect("serialize canonical JSON value"))
}

fn rehash(path: &Path, value: &mut Value) {
    for object in value["objects"].as_array_mut().expect("objects") {
        let relative = object["path"].as_str().expect("path");
        object["content_hash"] = Value::String(hash_bytes(
            &fs::read(path.join(relative)).expect("read object content"),
        ));
        object["metadata_hash"] = Value::String(hash_value(&object["metadata"]));
    }
    let mut logical = value.clone();
    logical
        .as_object_mut()
        .expect("manifest object")
        .remove("logical_hash");
    value["logical_hash"] = Value::String(hash_value(&logical));
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize manifest");
    bytes.push(b'\n');
    fs::write(path.join("manifest.json"), bytes).expect("write manifest");
}

fn relogical(path: &Path, value: &mut Value) {
    let mut logical = value.clone();
    logical
        .as_object_mut()
        .expect("manifest object")
        .remove("logical_hash");
    value["logical_hash"] = Value::String(hash_value(&logical));
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize manifest");
    bytes.push(b'\n');
    fs::write(path.join("manifest.json"), bytes).expect("write manifest");
}

fn replace_prose(path: &Path, value: &Value, uri: &str, prose: &str) {
    let index = object_index(value, uri);
    let object = &value["objects"][index];
    let file = path.join(object["path"].as_str().expect("path"));
    let bytes = fs::read(&file).expect("read Markdown");
    let start = object["content_start_byte"]
        .as_u64()
        .expect("content start") as usize;
    let mut edited = bytes[..start].to_vec();
    edited.extend_from_slice(prose.as_bytes());
    fs::write(file, edited).expect("write edited Markdown");
}

fn import(app: &mut NirmataApp, path: &Path) -> nirmata_app::ImportSnapshotResult {
    app.import_vfs_snapshot(ImportSnapshotInput {
        snapshot_directory: path.to_path_buf(),
    })
    .expect("import snapshot")
}

fn cleanup(mut app: NirmataApp, fixture: &Fixture) {
    app.close_world().expect("close fixture");
    let mut last_error = None;
    for _ in 0..40 {
        match fs::remove_dir_all(&fixture.parent) {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
    panic!("remove fixture: {}", last_error.expect("cleanup error"));
}

#[test]
fn calendar_metadata_imports_as_reviewed_world_update_and_undoes() {
    let fixture = fixture("calendar");
    let (mut app, snapshot) = open_and_export(&fixture, "calendar-edit");
    let session = app.get_current_world().expect("session").expect("world");
    let mut value = manifest(&snapshot);
    let world_uri = ObjectRef::World(session.world_id).to_string();
    let index = object_index(&value, &world_uri);
    value["objects"][index]["metadata"]["calendar"] = json!({
        "name": "Imperial",
        "epoch_tick": 100,
        "ticks_per_day": 10,
        "weekday_names": ["First", "Second", "Third"],
        "months": [
            { "name": "Ash", "days": 2 },
            { "name": "Rain", "days": 3 }
        ]
    });
    rehash(&snapshot, &mut value);

    let imported = import(&mut app, &snapshot);
    assert_eq!(imported.created_count, 0);
    assert_eq!(imported.updated_count, 1);
    assert_eq!(imported.deleted_count, 0);
    assert!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .world
            .calendar()
            .is_none()
    );
    app.confirm_stored_manual_review(&imported.review.review_key)
        .expect("commit calendar import");
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .world
            .calendar()
            .expect("calendar")
            .name(),
        "Imperial"
    );
    app.undo_last_commit().expect("undo calendar import");
    assert!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .world
            .calendar()
            .is_none()
    );
    cleanup(app, &fixture);
}

#[test]
fn schema_nine_snapshot_without_calendar_remains_importable() {
    let fixture = fixture("schema-nine-calendar-compat");
    let (mut app, snapshot) = open_and_export(&fixture, "schema-nine");
    let mut value = manifest(&snapshot);
    value["canon_schema_version"] = Value::from(9);
    let world_index = value["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .position(|object| object["object_type"] == "world")
        .expect("world");
    assert!(
        value["objects"][world_index]["metadata"]
            .get("calendar")
            .is_none()
    );
    replace_prose(
        &snapshot,
        &value,
        &ObjectRef::Entity(fixture.actor.id()).to_string(),
        "Edited by a schema nine snapshot.",
    );
    rehash(&snapshot, &mut value);

    let imported = import(&mut app, &snapshot);
    assert_eq!(imported.updated_count, 1);
    assert!(imported.review.ready_to_confirm);
    cleanup(app, &fixture);
}

fn entity_body(project: &Path, id: EntityId) -> String {
    WorldStore::open(project)
        .expect("open entity store")
        .get_entity(id)
        .expect("read entity")
        .expect("entity exists")
        .body_md()
        .to_owned()
}

fn document_body(project: &Path, id: nirmata_core::DocumentId) -> String {
    WorldStore::open(project)
        .expect("open document store")
        .get_document(id)
        .expect("read document")
        .expect("document exists")
        .object()
        .body_md()
        .to_owned()
}

fn logical_canon(snapshot: &CanonSnapshot) -> Value {
    let mut value = json!({
        "world": snapshot.world(),
        "entities": snapshot.entities(),
        "relations": snapshot.relations(),
        "goals": snapshot.goals(),
        "events": snapshot.events(),
        "claims": snapshot.claims(),
        "rules": snapshot.rules(),
        "documents": snapshot.documents(),
        "references": snapshot.content_references(),
    });
    strip_editorial_fields(&mut value);
    value
}

fn strip_editorial_fields(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(strip_editorial_fields),
        Value::Object(values) => {
            values.remove("current_revision");
            values.remove("version");
            values.remove("created_at_ms");
            values.remove("updated_at_ms");
            values.values_mut().for_each(strip_editorial_fields);
        }
        _ => {}
    }
}

fn assert_cited_hit(hit: &nirmata_app::SearchResult, expected: ObjectRef, expected_stage: &str) {
    assert_eq!(hit.object_ref, expected);
    assert_eq!(hit.uri, expected.to_string());
    assert_eq!(hit.stage, expected_stage);
    assert!(!hit.provenance.is_empty());
    assert!(hit.score > 0);
    assert!(hit.rank > 0);
    assert!(!hit.score_explanation.is_empty());
}

#[test]
fn nir_058_hybrid_retrieval_and_snapshot_round_trip_preserve_authority_and_human_selection() {
    let fixture = fixture("nir-058-e2e");
    let mut store = WorldStore::open(&fixture.project).expect("open NIR-058 fixture");
    let world = store.load_world().expect("load NIR-058 world");
    let semantic_document = Document::new(
        world.id(),
        "Senate Record",
        "minutes",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "The senate refuses the border pact.",
        1,
    )
    .expect("semantic source");
    store
        .insert_document(&DocumentAggregate::new(semantic_document.clone(), vec![]))
        .expect("insert semantic source");
    let positive_claim = Claim::new(
        world.id(),
        fixture.actor.id(),
        "Mara believes the archive gate is open.",
        Some("archive.gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(fixture.actor.id()),
        Some(ClaimModality::Belief),
        Some("testimony".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.8),
        None,
        world.current_revision(),
    )
    .expect("positive perspective");
    let negative_claim = Claim::new(
        world.id(),
        fixture.actor.id(),
        "The old note records that the archive gate is not open.",
        Some("archive.gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Negative,
        ClaimAuthentication::Attributed,
        Some(fixture.disposable.id()),
        Some(ClaimModality::Belief),
        Some("testimony".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.7),
        None,
        world.current_revision(),
    )
    .expect("negative perspective");
    store
        .insert_claim(&positive_claim)
        .expect("insert positive perspective");
    store
        .insert_claim(&negative_claim)
        .expect("insert negative perspective");
    let prior_snapshot = store
        .read_canon_snapshot()
        .expect("read prior canon snapshot");
    let prior_logical_canon = logical_canon(&prior_snapshot);
    drop(store);

    let (mut app, edited_path) = open_and_export(&fixture, "edited-e2e");
    let source_ref = ObjectRef::Document(semantic_document.id());
    let exact_request = SearchWorldRequest::new(StructuredSearchQuery {
        kinds: vec![StructuredSearchKind::Document],
        text: Some("senate refuses pact".to_owned()),
        limit: 5,
        ..Default::default()
    });
    let semantic_request = SearchWorldRequest::new(StructuredSearchQuery {
        kinds: vec![StructuredSearchKind::Document],
        text: Some("council rejects treaty".to_owned()),
        limit: 5,
        ..Default::default()
    });
    let exact_before = app.search_world(&exact_request).expect("active FTS search");
    let semantic_before = app
        .search_world(&semantic_request)
        .expect("active semantic search");
    assert_eq!(exact_before.hits.len(), 1);
    assert_eq!(semantic_before.hits.len(), 1);
    assert_cited_hit(&exact_before.hits[0], source_ref, "fts5");
    assert_cited_hit(&semantic_before.hits[0], source_ref, "semantic");
    assert!(
        semantic_before.hits[0]
            .provenance
            .contains("wordnet-en-offline:v1")
    );

    let contradiction_request = RelatedContextRequest {
        bundle: ContextBundleRequest {
            intent: ContextIntent::ContradictionCheck,
            anchors: vec![ObjectRef::Entity(fixture.actor.id())],
            query_text: None,
            temporal: None,
            temporal_radius: None,
            perspective_entity_ids: vec![],
            include_perspectives: true,
            relation_limit: 0,
            budget: ContextBudget {
                max_objects: 12,
                max_chars: 2_000,
            },
        },
        kinds: vec![StructuredSearchKind::Claim],
        empty: EmptySearchClassification::NoEvidence,
    };
    let contradiction_refs = app
        .get_related_context(&contradiction_request)
        .expect("contradiction context")
        .all_entries()
        .into_iter()
        .map(|entry| {
            assert_eq!(entry.result.uri, entry.result.object_ref.to_string());
            assert_eq!(entry.result.stage, "relation");
            assert!(!entry.result.provenance.is_empty());
            entry.result.object_ref
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        contradiction_refs,
        std::collections::BTreeSet::from([
            ObjectRef::Claim(positive_claim.id()),
            ObjectRef::Claim(negative_claim.id()),
        ])
    );

    let connection = rusqlite::Connection::open(&fixture.project).expect("open derived index");
    connection
        .execute("DELETE FROM canon_fts", [])
        .expect("delete derived FTS state");
    drop(connection);
    let mut rebuild_store = WorldStore::open(&fixture.project).expect("open rebuild store");
    assert!(
        rebuild_store
            .search_structured_fts(&exact_request.query)
            .expect("search without FTS state")
            .is_empty()
    );
    let semantic_without_fts = app
        .search_world(&semantic_request)
        .expect("semantic reads canon without FTS state");
    assert_cited_hit(&semantic_without_fts.hits[0], source_ref, "semantic");
    assert_eq!(
        logical_canon(
            &rebuild_store
                .read_canon_snapshot()
                .expect("canon after derived deletion")
        ),
        prior_logical_canon
    );
    rebuild_store
        .rebuild_canon_text_index()
        .expect("rebuild FTS from canon");
    assert_eq!(
        rebuild_store
            .search_structured_fts(&exact_request.query)
            .expect("FTS after rebuild")[0]
            .object,
        source_ref
    );
    drop(rebuild_store);

    let tampered = app
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory: fixture.parent.clone(),
            snapshot_name: "tampered-e2e".to_owned(),
        })
        .expect("export tamper candidate");
    let mut tampered_manifest = manifest(&tampered.path);
    tampered_manifest["logical_hash"] = Value::String("sha256:tampered".to_owned());
    let mut tampered_bytes =
        serde_json::to_vec_pretty(&tampered_manifest).expect("serialize tampered manifest");
    tampered_bytes.push(b'\n');
    fs::write(tampered.path.join("manifest.json"), tampered_bytes)
        .expect("write tampered manifest");
    assert!(matches!(
        app.import_vfs_snapshot(ImportSnapshotInput {
            snapshot_directory: tampered.path,
        }),
        Err(AppError::InvalidSnapshotImport { .. })
    ));
    assert_eq!(
        logical_canon(
            &WorldStore::open(&fixture.project)
                .expect("open canon after tamper rejection")
                .read_canon_snapshot()
                .expect("canon after tamper rejection")
        ),
        prior_logical_canon
    );

    let actor_uri = ObjectRef::Entity(fixture.actor.id()).to_string();
    let source_uri = source_ref.to_string();
    let mut edited_manifest = manifest(&edited_path);
    replace_prose(
        &edited_path,
        &edited_manifest,
        &actor_uri,
        "This renamed entity edit must be rejected.",
    );
    let actor_index = object_index(&edited_manifest, &actor_uri);
    edited_manifest["objects"][actor_index]["metadata"]["name"] =
        Value::String("Mara Renamed Externally".to_owned());
    edited_manifest["objects"][actor_index]["metadata"]["slug"] =
        Value::String("mara-renamed-externally".to_owned());
    let accepted_markdown = "<script>throw new Error('inert')</script>\n\nThe senate refuses the border pact. Moonledger remains cited.";
    replace_prose(
        &edited_path,
        &edited_manifest,
        &source_uri,
        accepted_markdown,
    );
    rehash(&edited_path, &mut edited_manifest);

    let imported = import(&mut app, &edited_path);
    assert_eq!(
        (
            imported.created_count,
            imported.updated_count,
            imported.deleted_count
        ),
        (0, 2, 0)
    );
    let rejected_operation = imported
        .review
        .operations
        .iter()
        .find(|operation| operation.target_uri == actor_uri)
        .expect("renamed actor operation")
        .operation_id
        .clone();
    let reviewed = app
        .apply_stored_manual_review_action(
            &imported.review.review_key,
            ManualReviewActionRequest::Reject {
                operation_id: rejected_operation,
            },
        )
        .expect("reject renamed actor operation");
    assert!(reviewed.ready_to_confirm);
    assert_eq!(
        reviewed
            .operations
            .iter()
            .filter(|operation| operation.selected)
            .map(|operation| operation.target_uri.as_str())
            .collect::<Vec<_>>(),
        vec![source_uri.as_str()]
    );
    app.confirm_stored_manual_review(&imported.review.review_key)
        .expect("commit approved snapshot operation");

    let committed_snapshot = WorldStore::open(&fixture.project)
        .expect("open committed canon")
        .read_canon_snapshot()
        .expect("read committed canon");
    let mut expected_committed_canon = prior_logical_canon.clone();
    let expected_document = expected_committed_canon["documents"]
        .as_array_mut()
        .expect("logical documents")
        .iter_mut()
        .find(|document| {
            document["object"]["id"] == Value::String(semantic_document.id().to_string())
        })
        .expect("approved logical document");
    expected_document["object"]["body_md"] = Value::String(accepted_markdown.to_owned());
    assert_eq!(
        logical_canon(&committed_snapshot),
        expected_committed_canon,
        "commit must change exactly the operation retained by human review"
    );
    assert_eq!(
        committed_snapshot
            .entities()
            .iter()
            .find(|entity| entity.id() == fixture.actor.id())
            .expect("actor remains")
            .name(),
        "Mara"
    );
    assert_eq!(
        document_body(&fixture.project, semantic_document.id()),
        accepted_markdown
    );
    let history = app.list_revision_history().expect("approved import audit");
    assert_eq!(history.revisions[0].operations.len(), 1);
    assert_eq!(history.revisions[0].operations[0].target_uri, source_uri);
    assert_eq!(history.revisions[0].operations[0].decision, "accept");

    let semantic_after = app
        .search_world(&semantic_request)
        .expect("semantic after approved import");
    assert_cited_hit(&semantic_after.hits[0], source_ref, "semantic");
    assert_eq!(semantic_after.hits[0].uri, semantic_before.hits[0].uri);
    let exact_after = app
        .search_world(&SearchWorldRequest::new(StructuredSearchQuery {
            kinds: vec![StructuredSearchKind::Document],
            text: Some("Moonledger".to_owned()),
            limit: 5,
            ..Default::default()
        }))
        .expect("FTS after approved import");
    assert_cited_hit(&exact_after.hits[0], source_ref, "fts5");
    let contradictions_after = app
        .get_related_context(&contradiction_request)
        .expect("contradictions after import")
        .all_entries()
        .into_iter()
        .map(|entry| entry.result.object_ref)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(contradictions_after, contradiction_refs);

    let approved_export = app
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory: fixture.parent.clone(),
            snapshot_name: "approved-e2e".to_owned(),
        })
        .expect("export resulting canon");
    assert!(matches!(
        app.import_vfs_snapshot(ImportSnapshotInput {
            snapshot_directory: approved_export.path,
        }),
        Err(AppError::SnapshotHasNoChanges)
    ));

    app.undo_last_commit()
        .expect("undo approved snapshot import");
    let restored_snapshot = WorldStore::open(&fixture.project)
        .expect("open restored canon")
        .read_canon_snapshot()
        .expect("read restored canon");
    assert_eq!(logical_canon(&restored_snapshot), prior_logical_canon);
    cleanup(app, &fixture);
}

#[test]
fn edited_entity_and_document_round_trip_through_review_commit_undo_and_reopen() {
    let fixture = fixture("round-trip");
    let (mut app, path) = open_and_export(&fixture, "edited");
    let actor_uri = ObjectRef::Entity(fixture.actor.id()).to_string();
    let document_uri = ObjectRef::Document(fixture.document.object().id()).to_string();
    let hostile = "<script>throw new Error('must stay inert')</script>\n[file](file:///tmp/escape)";
    let mut value = manifest(&path);
    replace_prose(&path, &value, &actor_uri, "Edited entity body.");
    replace_prose(&path, &value, &document_uri, hostile);
    let document_index = object_index(&value, &document_uri);
    value["objects"][document_index]["references"] = json!([{
        "source_uri": document_uri,
        "target_uri": ObjectRef::Entity(fixture.disposable.id()).to_string(),
        "ordinal": 0
    }]);
    rehash(&path, &mut value);

    let imported = import(&mut app, &path);
    assert_eq!(imported.created_count, 0);
    assert_eq!(imported.updated_count, 2);
    assert_eq!(imported.deleted_count, 0);
    assert_eq!(
        imported.review.freshness.status,
        ManualReviewFreshnessStatus::Current
    );
    assert!(imported.review.ready_to_confirm);
    let entity_diff = imported
        .review
        .operations
        .iter()
        .find(|operation| operation.target_uri == actor_uri)
        .expect("entity diff");
    assert!(
        entity_diff
            .before
            .as_ref()
            .expect("before")
            .lines
            .iter()
            .any(|line| line.value == "Original entity body.")
    );
    assert!(
        entity_diff
            .after
            .as_ref()
            .expect("after")
            .lines
            .iter()
            .any(|line| line.value == "Edited entity body.")
    );
    let document_diff = imported
        .review
        .operations
        .iter()
        .find(|operation| operation.target_uri == document_uri)
        .expect("document diff");
    assert!(
        document_diff
            .after
            .as_ref()
            .expect("after")
            .lines
            .iter()
            .any(|line| line
                .value
                .contains(&ObjectRef::Entity(fixture.disposable.id()).to_string()))
    );

    app.close_world().expect("close pending snapshot review");
    app.open_world(fixture.project.clone())
        .expect("reopen pending snapshot review");
    let pending = app
        .list_pending_reviews()
        .expect("recovered snapshot review");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].origin, PendingReviewOrigin::Snapshot);

    let rejected_operation = entity_diff.operation_id.clone();
    app.apply_stored_manual_review_action(
        &imported.review.review_key,
        ManualReviewActionRequest::Reject {
            operation_id: rejected_operation,
        },
    )
    .expect("reject imported operation");
    assert_eq!(
        entity_body(&fixture.project, fixture.actor.id()),
        "Original entity body."
    );
    app.discard_stored_manual_review(&imported.review.review_key)
        .expect("discard review");
    assert_eq!(
        document_body(&fixture.project, fixture.document.object().id()),
        "Original document body."
    );

    let imported = import(&mut app, &path);
    let committed = app
        .confirm_stored_manual_review(&imported.review.review_key)
        .expect("confirm imported review");
    assert_eq!(
        entity_body(&fixture.project, fixture.actor.id()),
        "Edited entity body."
    );
    assert_eq!(
        document_body(&fixture.project, fixture.document.object().id()),
        hostile
    );
    let history = app.list_revision_history().expect("history");
    assert_eq!(
        history.current_head_revision_id,
        committed.current_revision.to_string()
    );
    assert!(
        history.revisions[0]
            .operations
            .iter()
            .any(|operation| operation.target_uri == document_uri
                && operation.before.is_some()
                && operation.after.is_some())
    );

    app.close_world().expect("close after commit");
    app.open_world(fixture.project.clone())
        .expect("reopen committed world");
    assert_eq!(
        document_body(&fixture.project, fixture.document.object().id()),
        hostile
    );
    let undone = app.undo_last_commit().expect("undo snapshot commit");
    app.close_world().expect("close after undo");
    let reopened = app
        .open_world(fixture.project.clone())
        .expect("reopen undone world");
    assert_eq!(reopened.current_revision, undone.current_revision);
    assert_eq!(
        entity_body(&fixture.project, fixture.actor.id()),
        "Original entity body."
    );
    assert_eq!(
        document_body(&fixture.project, fixture.document.object().id()),
        "Original document body."
    );
    cleanup(app, &fixture);
}

#[test]
fn snapshot_diff_supports_typed_create_and_delete_without_writing_canon() {
    let fixture = fixture("create-delete");
    let (mut app, path) = open_and_export(&fixture, "edited");
    let mut value = manifest(&path);
    let disposable_uri = ObjectRef::Entity(fixture.disposable.id()).to_string();
    let old_index = object_index(&value, &disposable_uri);
    let mut created = value["objects"][old_index].clone();
    let old_path = path.join(created["path"].as_str().expect("old path"));
    fs::remove_file(old_path).expect("remove deleted object file");
    value["objects"]
        .as_array_mut()
        .expect("objects")
        .remove(old_index);

    let new_id = EntityId::new().to_string();
    let new_uri = format!("nirmata://entity/{new_id}");
    let relative = format!("entities/{new_id}.md");
    created["id"] = Value::String(new_id.clone());
    created["uri"] = Value::String(new_uri.clone());
    created["path"] = Value::String(relative.clone());
    created["metadata"]["id"] = Value::String(new_id.clone());
    created["metadata"]["name"] = Value::String("New Note".to_owned());
    created["metadata"]["slug"] = Value::String("new-note".to_owned());
    created["metadata"]["version"] = json!(1);
    let prefix = format!(
        "# Nirmata entity {new_id}\n\n- URI: `{new_uri}`\n- World ID: `{}`\n- Variant: `main`\n- Base revision: `{}`\n\n## Content\n\n",
        value["world_id"].as_str().expect("world"),
        value["base_revision"].as_str().expect("revision")
    );
    created["content_start_byte"] = json!(prefix.len());
    fs::write(path.join(&relative), format!("{prefix}Created externally."))
        .expect("write created object");
    value["objects"]
        .as_array_mut()
        .expect("objects")
        .push(created);
    rehash(&path, &mut value);

    let before = WorldStore::open(&fixture.project)
        .expect("open before import")
        .read_canon_snapshot()
        .expect("snapshot before import");
    let imported = import(&mut app, &path);
    assert_eq!(imported.created_count, 1);
    assert_eq!(imported.deleted_count, 1);
    assert!(
        imported
            .review
            .operations
            .iter()
            .any(|operation| operation.target_uri == new_uri
                && operation.before.is_none()
                && operation.after.is_some())
    );
    assert!(
        imported
            .review
            .operations
            .iter()
            .any(|operation| operation.target_uri == disposable_uri
                && operation.before.is_some()
                && operation.after.is_none())
    );
    app.discard_stored_manual_review(&imported.review.review_key)
        .expect("discard create/delete review");
    let after = WorldStore::open(&fixture.project)
        .expect("open after discard")
        .read_canon_snapshot()
        .expect("snapshot after discard");
    assert_eq!(before, after, "import and discard must not mutate canon");
    cleanup(app, &fixture);
}

#[test]
fn stale_snapshot_is_visible_and_cannot_be_confirmed_or_rebased() {
    let fixture = fixture("stale");
    let (mut app, path) = open_and_export(&fixture, "edited");
    let actor_uri = ObjectRef::Entity(fixture.actor.id()).to_string();
    let mut value = manifest(&path);
    replace_prose(&path, &value, &actor_uri, "Edit from stale snapshot.");
    rehash(&path, &mut value);

    let current = app.open_uri(&actor_uri).expect("current actor").result;
    let before = match WorldStore::open(&fixture.project)
        .expect("open actor store")
        .get_entity(fixture.actor.id())
        .expect("get actor")
    {
        Some(value) => value,
        None => panic!("actor missing"),
    };
    let after = Entity::restore(
        before.id(),
        before.world_id(),
        before.kind(),
        before.name(),
        before.slug(),
        "Committed after export",
        before.body_md(),
        before.attributes_json().as_str(),
        before.aliases().to_vec(),
        before.version() + 1,
        before.created_at_ms(),
        2,
    )
    .expect("updated actor");
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "advance revision".to_owned(),
            sources: vec![current.object_ref],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Reinterpretive,
                before,
                after,
            }],
        })
        .expect("start advancing review");
    app.confirm_manual_review(&review).expect("advance head");

    let imported = import(&mut app, &path);
    assert_eq!(
        imported.review.freshness.status,
        ManualReviewFreshnessStatus::Stale
    );
    assert!(!imported.review.freshness.can_revalidate);
    assert!(!imported.review.ready_to_confirm);
    assert!(matches!(
        app.confirm_stored_manual_review(&imported.review.review_key),
        Err(AppError::ManualReviewStale { .. })
    ));
    assert!(matches!(
        app.revalidate_stored_manual_review(&imported.review.review_key),
        Err(AppError::ManualReviewRevalidationFailed)
    ));
    cleanup(app, &fixture);
}

#[test]
fn rejects_hash_path_id_reference_unknown_and_binary_tampering() {
    for case in [
        "hash",
        "path",
        "id",
        "reference",
        "unknown",
        "binary",
        "type",
    ] {
        let fixture = fixture(case);
        let (mut app, path) = open_and_export(&fixture, "tampered");
        let actor_uri = ObjectRef::Entity(fixture.actor.id()).to_string();
        let document_uri = ObjectRef::Document(fixture.document.object().id()).to_string();
        let mut value = manifest(&path);
        match case {
            "hash" => replace_prose(&path, &value, &actor_uri, "changed without hashes"),
            "path" => {
                let index = object_index(&value, &actor_uri);
                value["objects"][index]["path"] = Value::String("../escape.md".to_owned());
                relogical(&path, &mut value);
            }
            "id" => {
                let index = object_index(&value, &actor_uri);
                value["objects"][index]["id"] = Value::String(EntityId::new().to_string());
                relogical(&path, &mut value);
            }
            "reference" => {
                let index = object_index(&value, &document_uri);
                value["objects"][index]["references"][0]["target_uri"] =
                    Value::String(format!("nirmata://entity/{}", EntityId::new()));
                relogical(&path, &mut value);
            }
            "unknown" => {
                fs::write(path.join("entities/unknown.bin"), b"unknown").expect("extra file")
            }
            "binary" => {
                let index = object_index(&value, &actor_uri);
                let relative = value["objects"][index]["path"].as_str().expect("path");
                fs::write(path.join(relative), [0xff, 0xfe, 0xfd]).expect("binary object");
                relogical(&path, &mut value);
            }
            "type" => {
                let index = object_index(&value, &actor_uri);
                value["objects"][index]["object_type"] = Value::String("plugin".to_owned());
                rehash(&path, &mut value);
            }
            _ => unreachable!(),
        }
        let error = app
            .import_vfs_snapshot(ImportSnapshotInput {
                snapshot_directory: path,
            })
            .expect_err("tampering must be rejected");
        assert!(
            matches!(error, AppError::InvalidSnapshotImport { .. }),
            "case {case}: {error}"
        );
        assert_eq!(
            entity_body(&fixture.project, fixture.actor.id()),
            "Original entity body."
        );
        cleanup(app, &fixture);
    }
}
