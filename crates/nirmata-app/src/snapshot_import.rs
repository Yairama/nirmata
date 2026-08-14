use crate::{
    AppError, DraftOperationInput, ManualReviewInput, ManualReviewSession, ManualReviewSnapshot,
    NirmataApp,
    app::StoredManualReview,
    snapshot_export::{
        LogicalSnapshot, SNAPSHOT_FORMAT, SNAPSHOT_FORMAT_VERSION, SnapshotManifest,
        SnapshotObject, TYPE_DIRECTORIES, hash_bytes, hash_json,
        remove_prose_and_embedded_references, type_directory,
    },
};
use nirmata_core::{
    RevisionId, World,
    change_set::RetconKind,
    claim::Claim,
    document::{ContentReference, Document, DocumentAggregate, ObjectRef},
    entity::Entity,
    event::{Event, EventAggregate},
    goal::Goal,
    relation::Relation,
    rule::Rule,
};
use nirmata_store::CanonSnapshot;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: u64 = 2 * 1024 * 1024;
const MAX_OBJECTS: usize = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportSnapshotInput {
    pub snapshot_directory: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSnapshotResult {
    pub path: PathBuf,
    pub world_id: String,
    pub variant_id: String,
    pub variant: String,
    pub base_revision: String,
    pub logical_hash: String,
    pub object_count: usize,
    pub created_count: usize,
    pub updated_count: usize,
    pub deleted_count: usize,
    pub review: ManualReviewSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
enum SnapshotValue {
    World(World),
    Entity(Entity),
    Relation(Relation),
    Event(EventAggregate),
    Claim(Claim),
    Rule(Rule),
    Goal(Goal),
    Document(DocumentAggregate),
}

impl NirmataApp {
    pub fn import_vfs_snapshot(
        &mut self,
        input: ImportSnapshotInput,
    ) -> Result<ImportSnapshotResult, AppError> {
        let root = validate_snapshot_directory(&input.snapshot_directory)?;
        let manifest_path = root.join("manifest.json");
        let manifest_bytes = read_regular_file(&manifest_path, MAX_MANIFEST_BYTES)?;
        let manifest: SnapshotManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| invalid(&manifest_path, format!("invalid manifest JSON: {error}")))?;

        let (snapshot, current_revision, world_id, schema_version, active_variant) = {
            let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
            crate::app::ensure_active_write_scope(active)?;
            let snapshot = active.store.read_canon_snapshot()?;
            let current_revision = snapshot.world().current_revision();
            let world_id = snapshot.world().id();
            let schema_version = snapshot.schema_version();
            let active_variant = active.store.active_variant()?;
            (
                snapshot,
                current_revision,
                world_id,
                schema_version,
                active_variant,
            )
        };
        validate_manifest_header(
            &manifest,
            world_id.to_string(),
            schema_version,
            &active_variant,
            &manifest_path,
        )?;
        let base_revision = RevisionId::from_str(&manifest.base_revision).map_err(|_| {
            invalid(
                &manifest_path,
                "base_revision must be a canonical UUID".to_owned(),
            )
        })?;
        {
            let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
            if active.store.get_revision(base_revision)?.is_none() {
                return Err(invalid(
                    &manifest_path,
                    "base_revision does not exist in the active world".to_owned(),
                ));
            }
            active
                .store
                .resolve_scope(nirmata_store::ReadScope::historical(
                    active_variant.id,
                    base_revision,
                ))
                .map_err(|_| {
                    invalid(
                        &manifest_path,
                        "base_revision is not in the snapshot variant history".to_owned(),
                    )
                })?;
        }
        validate_logical_hash(&manifest, &manifest_path)?;
        validate_snapshot_tree(&root, &manifest)?;

        let current = current_values(&snapshot);
        let edited = read_edited_values(&root, &manifest, world_id, &current)?;
        let now_ms = current_time_ms()?;
        let (operations, created_count, updated_count, deleted_count, sources) =
            diff_values(&current, &edited, now_ms)?;
        if operations.is_empty() {
            return Err(AppError::SnapshotHasNoChanges);
        }

        let review_key = ObjectRef::World(world_id).to_string();
        if self.manual_reviews.contains_key(&review_key) {
            return Err(AppError::ReviewSessionConflict(review_key));
        }
        let review = {
            let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
            ManualReviewSession::create(
                active.session.active_variant.id,
                world_id,
                base_revision,
                ManualReviewInput {
                    objective: "Import edited VFS snapshot".to_owned(),
                    sources,
                    assumptions: vec![format!("snapshot logical hash: {}", manifest.logical_hash)],
                    operations,
                },
                &active.store,
            )?
        };
        let mut stored = StoredManualReview::from_snapshot_import(review);
        stored.sync_with_revision(current_revision);
        let review = stored.snapshot(&review_key);
        self.insert_pending_review(review_key, stored)?;

        Ok(ImportSnapshotResult {
            path: root,
            world_id: manifest.world_id,
            variant_id: active_variant.id.to_string(),
            variant: active_variant.name,
            base_revision: manifest.base_revision,
            logical_hash: manifest.logical_hash,
            object_count: manifest.objects.len(),
            created_count,
            updated_count,
            deleted_count,
            review,
        })
    }
}

fn validate_snapshot_directory(path: &Path) -> Result<PathBuf, AppError> {
    if path.as_os_str().is_empty() {
        return Err(invalid(path, "snapshot directory is empty".to_owned()));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| invalid(path, format!("cannot inspect snapshot directory: {error}")))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid(
            path,
            "snapshot root must be a real directory, not a symlink".to_owned(),
        ));
    }
    fs::canonicalize(path).map_err(|error| invalid(path, format!("cannot resolve path: {error}")))
}

fn validate_manifest_header(
    manifest: &SnapshotManifest,
    world_id: String,
    schema_version: i64,
    active_variant: &nirmata_store::Variant,
    path: &Path,
) -> Result<(), AppError> {
    if manifest.format != SNAPSHOT_FORMAT
        || manifest.format_version != SNAPSHOT_FORMAT_VERSION
        || manifest.hash_algorithm != "sha256"
    {
        return Err(invalid(path, "unsupported snapshot format".to_owned()));
    }
    if manifest.world_id != world_id {
        return Err(invalid(
            path,
            "snapshot belongs to another world".to_owned(),
        ));
    }
    let variant_matches = match manifest.variant_id.as_deref() {
        Some(id) => id == active_variant.id.to_string(),
        None => active_variant.name.eq_ignore_ascii_case("main"),
    };
    if !variant_matches || manifest.variant != active_variant.name {
        return Err(invalid(
            path,
            "snapshot belongs to another variant".to_owned(),
        ));
    }
    let schema_supported = manifest.canon_schema_version == schema_version
        || (schema_version == 11 && matches!(manifest.canon_schema_version, 9 | 10));
    if !schema_supported {
        return Err(invalid(
            path,
            format!(
                "canon schema {} is not supported by current schema {schema_version}",
                manifest.canon_schema_version
            ),
        ));
    }
    if manifest.canon_schema_version == 9
        && manifest.objects.iter().any(|object| {
            object.object_type == "world" && object.metadata.get("calendar").is_some()
        })
    {
        return Err(invalid(
            path,
            "canon schema 9 snapshots cannot define a world calendar".to_owned(),
        ));
    }
    if manifest.objects.is_empty() || manifest.objects.len() > MAX_OBJECTS {
        return Err(invalid(path, "invalid snapshot object count".to_owned()));
    }
    Ok(())
}

fn validate_logical_hash(manifest: &SnapshotManifest, path: &Path) -> Result<(), AppError> {
    let logical = LogicalSnapshot {
        format: SNAPSHOT_FORMAT,
        format_version: SNAPSHOT_FORMAT_VERSION,
        hash_algorithm: "sha256",
        world_id: manifest.world_id.clone(),
        variant: &manifest.variant,
        variant_id: manifest.variant_id.as_deref(),
        base_revision: manifest.base_revision.clone(),
        canon_schema_version: manifest.canon_schema_version,
        objects: &manifest.objects,
    };
    let expected = hash_json(&logical)?;
    if manifest.logical_hash != expected {
        return Err(invalid(
            path,
            "logical_hash does not match manifest".to_owned(),
        ));
    }
    Ok(())
}

fn validate_snapshot_tree(root: &Path, manifest: &SnapshotManifest) -> Result<(), AppError> {
    let mut expected_files = HashSet::with_capacity(manifest.objects.len());
    let mut identities = HashSet::with_capacity(manifest.objects.len());
    for object in &manifest.objects {
        validate_object_identity(object, root)?;
        if !expected_files.insert(object.path.clone()) {
            return Err(invalid(root, format!("duplicate path {}", object.path)));
        }
        if !identities.insert((object.object_type.clone(), object.id.clone())) {
            return Err(invalid(root, format!("duplicate object {}", object.uri)));
        }
    }

    let root_entries = read_directory(root)?;
    let expected_root: HashSet<_> = TYPE_DIRECTORIES
        .iter()
        .map(|value| (*value).to_owned())
        .chain(std::iter::once("manifest.json".to_owned()))
        .collect();
    let actual_root: HashSet<_> = root_entries
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    if actual_root != expected_root {
        return Err(invalid(
            root,
            "snapshot contains unknown or missing root entries".to_owned(),
        ));
    }

    let mut actual_files = HashSet::with_capacity(expected_files.len());
    for directory in TYPE_DIRECTORIES {
        let directory_path = root.join(directory);
        let metadata = fs::symlink_metadata(&directory_path).map_err(|error| {
            invalid(
                &directory_path,
                format!("cannot inspect directory: {error}"),
            )
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid(
                &directory_path,
                "type entry must be a real directory".to_owned(),
            ));
        }
        for entry in read_directory(&directory_path)? {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| invalid(&path, format!("cannot inspect object file: {error}")))?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(invalid(
                    &path,
                    "object entry must be a regular file".to_owned(),
                ));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid(&path, "object filename must be UTF-8".to_owned()))?;
            actual_files.insert(format!("{directory}/{name}"));
        }
    }
    if actual_files != expected_files {
        return Err(invalid(
            root,
            "manifest files do not exactly match snapshot files".to_owned(),
        ));
    }
    Ok(())
}

fn validate_object_identity(object: &SnapshotObject, root: &Path) -> Result<(), AppError> {
    let object_ref = ObjectRef::from_str(&object.uri)
        .map_err(|_| invalid(root, format!("invalid object URI {}", object.uri)))?;
    if object_ref.kind() != object.object_type || object.uri != object_ref.to_string() {
        return Err(invalid(
            root,
            format!("non-canonical object URI {}", object.uri),
        ));
    }
    let canonical_id = object.uri.rsplit('/').next().expect("validated URI");
    if object.id != canonical_id {
        return Err(invalid(
            root,
            format!("ID and URI disagree for {}", object.uri),
        ));
    }
    let expected = format!("{}/{}.md", type_directory(&object.object_type), object.id);
    if object.path != expected
        || Path::new(&object.path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(root, format!("unsafe object path {}", object.path)));
    }
    if !object.content_hash.starts_with("sha256:") || !object.metadata_hash.starts_with("sha256:") {
        return Err(invalid(root, format!("invalid hash for {}", object.uri)));
    }
    Ok(())
}

fn read_edited_values(
    root: &Path,
    manifest: &SnapshotManifest,
    world_id: nirmata_core::WorldId,
    current: &BTreeMap<ObjectRef, SnapshotValue>,
) -> Result<BTreeMap<ObjectRef, SnapshotValue>, AppError> {
    let resulting_refs = manifest
        .objects
        .iter()
        .map(|object| ObjectRef::from_str(&object.uri).expect("identity was validated"))
        .collect::<HashSet<_>>();
    let based_on_current = current
        .values()
        .find_map(|value| match value {
            SnapshotValue::World(world) => Some(world.current_revision().to_string()),
            _ => None,
        })
        .is_some_and(|revision| revision == manifest.base_revision);
    let mut values = BTreeMap::new();
    for object in &manifest.objects {
        let object_ref = ObjectRef::from_str(&object.uri).expect("identity was validated");
        let bytes = read_regular_file(&root.join(&object.path), MAX_MARKDOWN_BYTES)?;
        if object.content_hash != hash_bytes(&bytes) {
            return Err(invalid(
                &root.join(&object.path),
                "content_hash does not match file".to_owned(),
            ));
        }
        if object.metadata_hash != hash_json(&object.metadata)? {
            return Err(invalid(
                &root.join("manifest.json"),
                format!("metadata_hash does not match {}", object.uri),
            ));
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            invalid(
                &root.join(&object.path),
                "Markdown must be UTF-8".to_owned(),
            )
        })?;
        if text.contains('\0') {
            return Err(invalid(
                &root.join(&object.path),
                "binary Markdown is not supported".to_owned(),
            ));
        }
        let prefix = expected_prefix(object, manifest);
        if object.content_start_byte != prefix.len() || !bytes.starts_with(prefix.as_bytes()) {
            return Err(invalid(
                &root.join(&object.path),
                "generated Markdown header was altered".to_owned(),
            ));
        }
        let prose = &text[object.content_start_byte..];
        let references = parse_references(object, &resulting_refs, root)?;
        let value = parse_value(object, prose, references, root)?;
        if value.object_ref() != object_ref || value.world_id() != world_id {
            return Err(invalid(
                &root.join("manifest.json"),
                format!("metadata identity does not match {}", object.uri),
            ));
        }
        if current.contains_key(&object_ref) {
            value.validate_export_controls(
                manifest,
                current.get(&object_ref).expect("present"),
                based_on_current,
                root,
            )?;
        } else {
            value.validate_new_controls(manifest, root)?;
        }
        values.insert(object_ref, value);
    }
    if !values.contains_key(&ObjectRef::World(world_id))
        || values
            .keys()
            .filter(|value| matches!(value, ObjectRef::World(_)))
            .count()
            != 1
    {
        return Err(invalid(
            root,
            "snapshot must contain exactly the active world".to_owned(),
        ));
    }
    Ok(values)
}

fn parse_references(
    object: &SnapshotObject,
    resulting: &HashSet<ObjectRef>,
    root: &Path,
) -> Result<Vec<ContentReference>, AppError> {
    let source = ObjectRef::from_str(&object.uri).expect("identity was validated");
    let mut ordinals = HashSet::new();
    let mut references = Vec::with_capacity(object.references.len());
    for reference in &object.references {
        let found_source = ObjectRef::from_str(&reference.source_uri)
            .map_err(|_| invalid(root, "invalid content reference source".to_owned()))?;
        let target = ObjectRef::from_str(&reference.target_uri)
            .map_err(|_| invalid(root, "invalid content reference target".to_owned()))?;
        if found_source != source
            || reference.source_uri != source.to_string()
            || reference.target_uri != target.to_string()
            || !resulting.contains(&target)
            || !ordinals.insert(reference.ordinal)
        {
            return Err(invalid(
                root,
                format!("broken or duplicate content reference on {}", object.uri),
            ));
        }
        references.push(ContentReference::new(source, target, reference.ordinal));
    }
    references.sort_by_key(ContentReference::ordinal);
    if object.object_type != "document" && !references.is_empty() {
        return Err(invalid(
            root,
            "content reference edits are supported only for documents".to_owned(),
        ));
    }
    Ok(references)
}

fn parse_value(
    object: &SnapshotObject,
    prose: &str,
    references: Vec<ContentReference>,
    root: &Path,
) -> Result<SnapshotValue, AppError> {
    let parsed = match object.object_type.as_str() {
        "world" => SnapshotValue::World(parse_with_prose::<World>(
            &object.metadata,
            "premise_md",
            prose,
            object,
            root,
        )?),
        "entity" => SnapshotValue::Entity(parse_with_prose::<Entity>(
            &object.metadata,
            "body_md",
            prose,
            object,
            root,
        )?),
        "relation" => {
            if !prose.is_empty() {
                return Err(invalid(
                    root,
                    "relations cannot contain Markdown prose".to_owned(),
                ));
            }
            SnapshotValue::Relation(parse_metadata(&object.metadata, object, root)?)
        }
        "event" => SnapshotValue::Event(parse_nested_with_prose::<EventAggregate>(
            &object.metadata,
            "event",
            "body_md",
            prose,
            object,
            root,
        )?),
        "claim" => SnapshotValue::Claim(parse_with_prose::<Claim>(
            &object.metadata,
            "content_md",
            prose,
            object,
            root,
        )?),
        "rule" => SnapshotValue::Rule(parse_with_prose::<Rule>(
            &object.metadata,
            "statement_md",
            prose,
            object,
            root,
        )?),
        "goal" => SnapshotValue::Goal(parse_with_prose::<Goal>(
            &object.metadata,
            "desired_state_md",
            prose,
            object,
            root,
        )?),
        "document" => {
            let document: Document = parse_nested_object_with_prose(
                &object.metadata,
                "object",
                "body_md",
                prose,
                object,
                root,
            )?;
            SnapshotValue::Document(DocumentAggregate::new(document, references))
        }
        _ => {
            return Err(invalid(
                root,
                format!("unsupported object type {}", object.object_type),
            ));
        }
    };
    let mut canonical = parsed.metadata_value()?;
    remove_prose_and_embedded_references(&object.object_type, &mut canonical);
    if canonical != object.metadata {
        return Err(invalid(
            root,
            format!("unknown or non-canonical metadata on {}", object.uri),
        ));
    }
    parsed.revalidate_domain(root)
}

fn parse_with_prose<T: serde::de::DeserializeOwned>(
    metadata: &Value,
    field: &str,
    prose: &str,
    object: &SnapshotObject,
    root: &Path,
) -> Result<T, AppError> {
    let mut value = metadata.clone();
    value
        .as_object_mut()
        .ok_or_else(|| {
            invalid(
                root,
                format!("metadata for {} must be an object", object.uri),
            )
        })?
        .insert(field.to_owned(), Value::String(prose.to_owned()));
    serde_json::from_value(value).map_err(|error| {
        invalid(
            root,
            format!("invalid metadata for {}: {error}", object.uri),
        )
    })
}

fn parse_nested_with_prose<T: serde::de::DeserializeOwned>(
    metadata: &Value,
    nested: &str,
    field: &str,
    prose: &str,
    object: &SnapshotObject,
    root: &Path,
) -> Result<T, AppError> {
    let mut value = metadata.clone();
    value
        .get_mut(nested)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid(root, format!("metadata for {} lacks {nested}", object.uri)))?
        .insert(field.to_owned(), Value::String(prose.to_owned()));
    serde_json::from_value(value).map_err(|error| {
        invalid(
            root,
            format!("invalid metadata for {}: {error}", object.uri),
        )
    })
}

fn parse_nested_object_with_prose<T: serde::de::DeserializeOwned>(
    metadata: &Value,
    nested: &str,
    field: &str,
    prose: &str,
    object: &SnapshotObject,
    root: &Path,
) -> Result<T, AppError> {
    let mut value = metadata
        .get(nested)
        .cloned()
        .ok_or_else(|| invalid(root, format!("metadata for {} lacks {nested}", object.uri)))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid(root, format!("metadata field {nested} must be an object")))?
        .insert(field.to_owned(), Value::String(prose.to_owned()));
    serde_json::from_value(value).map_err(|error| {
        invalid(
            root,
            format!("invalid metadata for {}: {error}", object.uri),
        )
    })
}

fn parse_metadata<T: serde::de::DeserializeOwned>(
    metadata: &Value,
    object: &SnapshotObject,
    root: &Path,
) -> Result<T, AppError> {
    serde_json::from_value(metadata.clone()).map_err(|error| {
        invalid(
            root,
            format!("invalid metadata for {}: {error}", object.uri),
        )
    })
}

fn current_values(snapshot: &CanonSnapshot) -> BTreeMap<ObjectRef, SnapshotValue> {
    let mut values = BTreeMap::new();
    values.insert(
        ObjectRef::World(snapshot.world().id()),
        SnapshotValue::World(snapshot.world().clone()),
    );
    for value in snapshot.entities() {
        values.insert(
            ObjectRef::Entity(value.id()),
            SnapshotValue::Entity(value.clone()),
        );
    }
    for value in snapshot.relations() {
        values.insert(
            ObjectRef::Relation(value.id()),
            SnapshotValue::Relation(value.clone()),
        );
    }
    for value in snapshot.events() {
        values.insert(
            ObjectRef::Event(value.event().id()),
            SnapshotValue::Event(value.clone()),
        );
    }
    for value in snapshot.claims() {
        values.insert(
            ObjectRef::Claim(value.id()),
            SnapshotValue::Claim(value.clone()),
        );
    }
    for value in snapshot.rules() {
        values.insert(
            ObjectRef::Rule(value.id()),
            SnapshotValue::Rule(value.clone()),
        );
    }
    for value in snapshot.goals() {
        values.insert(
            ObjectRef::Goal(value.id()),
            SnapshotValue::Goal(value.clone()),
        );
    }
    for value in snapshot.documents() {
        values.insert(
            ObjectRef::Document(value.object().id()),
            SnapshotValue::Document(value.clone()),
        );
    }
    values
}

fn diff_values(
    current: &BTreeMap<ObjectRef, SnapshotValue>,
    edited: &BTreeMap<ObjectRef, SnapshotValue>,
    now_ms: i64,
) -> Result<
    (
        Vec<DraftOperationInput>,
        usize,
        usize,
        usize,
        Vec<ObjectRef>,
    ),
    AppError,
> {
    let mut creates_and_updates = Vec::new();
    let mut deletes = Vec::new();
    let mut sources = BTreeSet::new();
    let world = current
        .keys()
        .find(|value| matches!(value, ObjectRef::World(_)))
        .copied()
        .expect("canon snapshot contains world");
    sources.insert(world);
    let mut created = 0;
    let mut updated = 0;
    let mut deleted = 0;

    for (object_ref, after) in edited {
        match current.get(object_ref) {
            Some(before) if before.same_editable_content(after)? => {}
            Some(before) => {
                sources.insert(*object_ref);
                creates_and_updates.push((
                    operation_rank(*object_ref),
                    update_operation(before, after, now_ms)?,
                ));
                updated += 1;
            }
            None => {
                creates_and_updates.push((
                    operation_rank(*object_ref),
                    create_operation(after, now_ms)?,
                ));
                created += 1;
            }
        }
    }
    for (object_ref, before) in current {
        if !edited.contains_key(object_ref) {
            if matches!(object_ref, ObjectRef::World(_)) {
                return Err(AppError::InvalidSnapshotImport {
                    path: PathBuf::from("manifest.json"),
                    reason: "the world object cannot be deleted".to_owned(),
                });
            }
            sources.insert(*object_ref);
            deletes.push((delete_rank(*object_ref), delete_operation(before)?));
            deleted += 1;
        }
    }
    creates_and_updates.sort_by_key(|(rank, _)| *rank);
    deletes.sort_by_key(|(rank, _)| *rank);
    let operations = creates_and_updates
        .into_iter()
        .chain(deletes)
        .map(|(_, operation)| operation)
        .collect();
    Ok((
        operations,
        created,
        updated,
        deleted,
        sources.into_iter().collect(),
    ))
}

fn create_operation(value: &SnapshotValue, now_ms: i64) -> Result<DraftOperationInput, AppError> {
    Ok(match value.normalized(None, now_ms)? {
        SnapshotValue::World(_) => return Err(AppError::SnapshotHasNoChanges),
        SnapshotValue::Entity(after) => DraftOperationInput::CreateEntity {
            retcon: RetconKind::Additive,
            after,
        },
        SnapshotValue::Relation(after) => DraftOperationInput::CreateRelation {
            retcon: RetconKind::Additive,
            after,
        },
        SnapshotValue::Event(after) => DraftOperationInput::CreateEvent {
            retcon: RetconKind::Additive,
            after,
        },
        SnapshotValue::Claim(after) => DraftOperationInput::CreateClaim {
            retcon: RetconKind::Additive,
            after,
        },
        SnapshotValue::Rule(after) => DraftOperationInput::CreateRule {
            retcon: RetconKind::Additive,
            after,
        },
        SnapshotValue::Goal(after) => DraftOperationInput::CreateGoal {
            retcon: RetconKind::Additive,
            after,
        },
        SnapshotValue::Document(after) => DraftOperationInput::CreateDocument {
            retcon: RetconKind::Additive,
            after,
        },
    })
}

fn update_operation(
    before: &SnapshotValue,
    edited: &SnapshotValue,
    now_ms: i64,
) -> Result<DraftOperationInput, AppError> {
    let after = edited.normalized(Some(before), now_ms)?;
    Ok(match (before, after) {
        (SnapshotValue::World(before), SnapshotValue::World(after)) => {
            DraftOperationInput::UpdateWorld {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Entity(before), SnapshotValue::Entity(after)) => {
            DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Relation(before), SnapshotValue::Relation(after)) => {
            DraftOperationInput::UpdateRelation {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Event(before), SnapshotValue::Event(after)) => {
            DraftOperationInput::UpdateEvent {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Claim(before), SnapshotValue::Claim(after)) => {
            DraftOperationInput::UpdateClaim {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Rule(before), SnapshotValue::Rule(after)) => {
            DraftOperationInput::UpdateRule {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Goal(before), SnapshotValue::Goal(after)) => {
            DraftOperationInput::UpdateGoal {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        (SnapshotValue::Document(before), SnapshotValue::Document(after)) => {
            DraftOperationInput::UpdateDocument {
                retcon: RetconKind::Reinterpretive,
                before: before.clone(),
                after,
            }
        }
        _ => {
            return Err(AppError::InvalidSnapshotImport {
                path: PathBuf::from("manifest.json"),
                reason: "object type changed for a stable ID".to_owned(),
            });
        }
    })
}

fn delete_operation(value: &SnapshotValue) -> Result<DraftOperationInput, AppError> {
    Ok(match value {
        SnapshotValue::World(_) => return Err(AppError::SnapshotHasNoChanges),
        SnapshotValue::Entity(before) => DraftOperationInput::DeleteEntity {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
        SnapshotValue::Relation(before) => DraftOperationInput::DeleteRelation {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
        SnapshotValue::Event(before) => DraftOperationInput::DeleteEvent {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
        SnapshotValue::Claim(before) => DraftOperationInput::DeleteClaim {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
        SnapshotValue::Rule(before) => DraftOperationInput::DeleteRule {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
        SnapshotValue::Goal(before) => DraftOperationInput::DeleteGoal {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
        SnapshotValue::Document(before) => DraftOperationInput::DeleteDocument {
            retcon: RetconKind::Replacement,
            before: before.clone(),
        },
    })
}

impl SnapshotValue {
    fn object_ref(&self) -> ObjectRef {
        match self {
            Self::World(value) => ObjectRef::World(value.id()),
            Self::Entity(value) => ObjectRef::Entity(value.id()),
            Self::Relation(value) => ObjectRef::Relation(value.id()),
            Self::Event(value) => ObjectRef::Event(value.event().id()),
            Self::Claim(value) => ObjectRef::Claim(value.id()),
            Self::Rule(value) => ObjectRef::Rule(value.id()),
            Self::Goal(value) => ObjectRef::Goal(value.id()),
            Self::Document(value) => ObjectRef::Document(value.object().id()),
        }
    }

    fn world_id(&self) -> nirmata_core::WorldId {
        match self {
            Self::World(value) => value.id(),
            Self::Entity(value) => value.world_id(),
            Self::Relation(value) => value.world_id(),
            Self::Event(value) => value.event().world_id(),
            Self::Claim(value) => value.world_id(),
            Self::Rule(value) => value.world_id(),
            Self::Goal(value) => value.world_id(),
            Self::Document(value) => value.object().world_id(),
        }
    }

    fn metadata_value(&self) -> Result<Value, AppError> {
        match self {
            Self::World(value) => serde_json::to_value(value),
            Self::Entity(value) => serde_json::to_value(value),
            Self::Relation(value) => serde_json::to_value(value),
            Self::Event(value) => serde_json::to_value(value),
            Self::Claim(value) => serde_json::to_value(value),
            Self::Rule(value) => serde_json::to_value(value),
            Self::Goal(value) => serde_json::to_value(value),
            Self::Document(value) => serde_json::to_value(value),
        }
        .map_err(AppError::SnapshotSerialization)
    }

    fn same_editable_content(&self, other: &Self) -> Result<bool, AppError> {
        let mut left = self.metadata_value()?;
        let mut right = other.metadata_value()?;
        strip_editorial_fields(self.object_ref().kind(), &mut left);
        strip_editorial_fields(other.object_ref().kind(), &mut right);
        Ok(left == right)
    }

    fn validate_export_controls(
        &self,
        manifest: &SnapshotManifest,
        current: &Self,
        based_on_current: bool,
        root: &Path,
    ) -> Result<(), AppError> {
        if let Self::World(world) = self
            && world.current_revision().to_string() != manifest.base_revision
        {
            return Err(invalid(
                root,
                "world revision and manifest base disagree".to_owned(),
            ));
        }
        if based_on_current && !self.same_editorial_controls(current) {
            return Err(invalid(
                root,
                format!(
                    "editorial version fields were altered on {}",
                    self.object_ref()
                ),
            ));
        }
        Ok(())
    }

    fn validate_new_controls(
        &self,
        manifest: &SnapshotManifest,
        root: &Path,
    ) -> Result<(), AppError> {
        if matches!(self, Self::World(_)) || self.version() != Some(1) {
            return Err(invalid(
                root,
                "new snapshot objects must start at version 1".to_owned(),
            ));
        }
        if let Self::Claim(claim) = self
            && claim.registered_revision_id().to_string() != manifest.base_revision
        {
            return Err(invalid(
                root,
                "new claims must link to the snapshot base revision".to_owned(),
            ));
        }
        Ok(())
    }

    fn same_editorial_controls(&self, current: &Self) -> bool {
        match (self, current) {
            (Self::World(left), Self::World(right)) => {
                left.created_at_ms() == right.created_at_ms()
                    && left.updated_at_ms() == right.updated_at_ms()
            }
            (Self::Entity(left), Self::Entity(right)) => {
                left.version() == right.version()
                    && left.created_at_ms() == right.created_at_ms()
                    && left.updated_at_ms() == right.updated_at_ms()
            }
            (Self::Relation(left), Self::Relation(right)) => left.version() == right.version(),
            (Self::Event(left), Self::Event(right)) => {
                left.event().version() == right.event().version()
                    && left.event().created_at_ms() == right.event().created_at_ms()
                    && left.event().updated_at_ms() == right.event().updated_at_ms()
            }
            (Self::Claim(left), Self::Claim(right)) => left.version() == right.version(),
            (Self::Rule(left), Self::Rule(right)) => {
                left.version() == right.version()
                    && left.created_at_ms() == right.created_at_ms()
                    && left.updated_at_ms() == right.updated_at_ms()
            }
            (Self::Goal(left), Self::Goal(right)) => left.version() == right.version(),
            (Self::Document(left), Self::Document(right)) => {
                left.object().version() == right.object().version()
                    && left.object().created_at_ms() == right.object().created_at_ms()
                    && left.object().updated_at_ms() == right.object().updated_at_ms()
            }
            _ => false,
        }
    }

    fn version(&self) -> Option<u64> {
        match self {
            Self::World(_) => None,
            Self::Entity(value) => Some(value.version()),
            Self::Relation(value) => Some(value.version()),
            Self::Event(value) => Some(value.event().version()),
            Self::Claim(value) => Some(value.version()),
            Self::Rule(value) => Some(value.version()),
            Self::Goal(value) => Some(value.version()),
            Self::Document(value) => Some(value.object().version()),
        }
    }

    fn revalidate_domain(self, root: &Path) -> Result<Self, AppError> {
        self.normalized_for_validation()
            .map_err(|error| invalid(root, format!("invalid domain object: {error}")))
    }

    fn normalized_for_validation(self) -> Result<Self, nirmata_core::DomainError> {
        Ok(match self {
            Self::World(value) => Self::World(World::restore(
                value.id(),
                value.name(),
                value.premise_md(),
                value.epoch_label(),
                value.calendar().cloned(),
                value.current_revision(),
                value.created_at_ms(),
                value.updated_at_ms(),
            )?),
            Self::Entity(value) => Self::Entity(entity_from(
                &value,
                value.version(),
                value.created_at_ms(),
                value.updated_at_ms(),
            )?),
            Self::Relation(value) => Self::Relation(relation_from(&value, value.version())?),
            Self::Event(value) => Self::Event(event_from(
                &value,
                value.event().version(),
                value.event().created_at_ms(),
                value.event().updated_at_ms(),
            )?),
            Self::Claim(value) => Self::Claim(claim_from(&value, value.version())?),
            Self::Rule(value) => Self::Rule(rule_from(
                &value,
                value.version(),
                value.created_at_ms(),
                value.updated_at_ms(),
            )?),
            Self::Goal(value) => Self::Goal(goal_from(&value, value.version())?),
            Self::Document(value) => Self::Document(document_from(
                &value,
                value.object().version(),
                value.object().created_at_ms(),
                value.object().updated_at_ms(),
            )?),
        })
    }

    fn normalized(&self, before: Option<&Self>, now_ms: i64) -> Result<Self, AppError> {
        let result = match (self, before) {
            (Self::World(value), Some(Self::World(old))) => Self::World(World::restore(
                old.id(),
                value.name(),
                value.premise_md(),
                value.epoch_label(),
                value.calendar().cloned(),
                old.current_revision(),
                old.created_at_ms(),
                now_ms,
            )?),
            (Self::Entity(value), Some(Self::Entity(old))) => Self::Entity(entity_from(
                value,
                old.version() + 1,
                old.created_at_ms(),
                now_ms,
            )?),
            (Self::Relation(value), Some(Self::Relation(old))) => {
                Self::Relation(relation_from(value, old.version() + 1)?)
            }
            (Self::Event(value), Some(Self::Event(old))) => Self::Event(event_from(
                value,
                old.event().version() + 1,
                old.event().created_at_ms(),
                now_ms,
            )?),
            (Self::Claim(value), Some(Self::Claim(old))) => {
                Self::Claim(claim_from(value, old.version() + 1)?)
            }
            (Self::Rule(value), Some(Self::Rule(old))) => Self::Rule(rule_from(
                value,
                old.version() + 1,
                old.created_at_ms(),
                now_ms,
            )?),
            (Self::Goal(value), Some(Self::Goal(old))) => {
                Self::Goal(goal_from(value, old.version() + 1)?)
            }
            (Self::Document(value), Some(Self::Document(old))) => Self::Document(document_from(
                value,
                old.object().version() + 1,
                old.object().created_at_ms(),
                now_ms,
            )?),
            (Self::Entity(value), None) => Self::Entity(entity_from(value, 1, now_ms, now_ms)?),
            (Self::Relation(value), None) => Self::Relation(relation_from(value, 1)?),
            (Self::Event(value), None) => Self::Event(event_from(value, 1, now_ms, now_ms)?),
            (Self::Claim(value), None) => Self::Claim(claim_from(value, 1)?),
            (Self::Rule(value), None) => Self::Rule(rule_from(value, 1, now_ms, now_ms)?),
            (Self::Goal(value), None) => Self::Goal(goal_from(value, 1)?),
            (Self::Document(value), None) => {
                Self::Document(document_from(value, 1, now_ms, now_ms)?)
            }
            _ => {
                return Err(AppError::InvalidSnapshotImport {
                    path: PathBuf::from("manifest.json"),
                    reason: "stable object changed type".to_owned(),
                });
            }
        };
        Ok(result)
    }
}

fn entity_from(
    value: &Entity,
    version: u64,
    created: i64,
    updated: i64,
) -> Result<Entity, nirmata_core::DomainError> {
    Entity::restore(
        value.id(),
        value.world_id(),
        value.kind(),
        value.name(),
        value.slug(),
        value.summary(),
        value.body_md(),
        value.attributes_json().as_str(),
        value.aliases().to_vec(),
        version,
        created,
        updated,
    )
}

fn relation_from(value: &Relation, version: u64) -> Result<Relation, nirmata_core::DomainError> {
    Relation::restore(
        value.id(),
        value.world_id(),
        value.source_entity_id(),
        value.target_entity_id(),
        value.kind(),
        value.direction(),
        value.valid_from_tick(),
        value.valid_to_tick(),
        value.certainty(),
        value.source_reference().map(str::to_owned),
        value.metadata_json().as_str(),
        version,
    )
}

fn event_from(
    value: &EventAggregate,
    version: u64,
    created: i64,
    updated: i64,
) -> Result<EventAggregate, nirmata_core::DomainError> {
    let event = value.event();
    Ok(EventAggregate::new(
        Event::restore(
            event.id(),
            event.world_id(),
            event.kind(),
            event.summary(),
            event.body_md(),
            *event.time(),
            event.location_entity_id(),
            event.participants().to_vec(),
            event.affected_goal_ids().to_vec(),
            version,
            created,
            updated,
        )?,
        value.links().to_vec(),
    ))
}

fn claim_from(value: &Claim, version: u64) -> Result<Claim, nirmata_core::DomainError> {
    Claim::restore(
        value.id(),
        value.world_id(),
        value.subject_entity_id(),
        value.content_md(),
        value.predicate_key().map(str::to_owned),
        value.object().cloned(),
        value.polarity(),
        value.authentication(),
        value.holder_entity_id(),
        value.modality(),
        value.register().map(str::to_owned),
        value.epistemic_basis().map(str::to_owned),
        value.source().map(str::to_owned),
        value.source_document_id(),
        value.source_claim_id(),
        value.holder_confidence(),
        value.period(),
        value.registered_revision_id(),
        value.superseded_revision_id(),
        version,
    )
}

fn rule_from(
    value: &Rule,
    version: u64,
    created: i64,
    updated: i64,
) -> Result<Rule, nirmata_core::DomainError> {
    Rule::restore(
        value.id(),
        value.world_id(),
        value.kind(),
        value.statement_md(),
        value.scope(),
        value.severity(),
        value.source().map(str::to_owned),
        value.validator_kind(),
        value.parameters_json().as_str(),
        version,
        created,
        updated,
    )
}

fn goal_from(value: &Goal, version: u64) -> Result<Goal, nirmata_core::DomainError> {
    Goal::restore(
        value.id(),
        value.world_id(),
        value.holder_entity_id(),
        value.desired_state_md(),
        value.priority(),
        value.status(),
        value.period(),
        value.visibility(),
        value.source().map(str::to_owned),
        version,
    )
}

fn document_from(
    value: &DocumentAggregate,
    version: u64,
    created: i64,
    updated: i64,
) -> Result<DocumentAggregate, nirmata_core::DomainError> {
    let object = value.object();
    Ok(DocumentAggregate::new(
        Document::restore(
            object.id(),
            object.world_id(),
            object.title(),
            object.kind(),
            object.author_entity_id(),
            object.perspective_entity_id(),
            object.canon_status(),
            object.body_md(),
            version,
            created,
            updated,
        )?,
        value.references().to_vec(),
    ))
}

fn strip_editorial_fields(object_type: &str, value: &mut Value) {
    let target = match object_type {
        "event" => value.get_mut("event"),
        "document" => value.get_mut("object"),
        _ => Some(value),
    };
    if let Some(Value::Object(map)) = target {
        map.remove("version");
        map.remove("created_at_ms");
        map.remove("updated_at_ms");
        if object_type == "world" {
            map.remove("current_revision");
        }
    }
}

fn expected_prefix(object: &SnapshotObject, manifest: &SnapshotManifest) -> String {
    format!(
        "# Nirmata {} {}\n\n- URI: `{}`\n- World ID: `{}`\n- Variant: `{}`\n- Base revision: `{}`\n\n## Content\n\n",
        object.object_type,
        object.id,
        object.uri,
        manifest.world_id,
        manifest.variant,
        manifest.base_revision
    )
}

fn operation_rank(value: ObjectRef) -> u8 {
    match value {
        ObjectRef::World(_) => 0,
        ObjectRef::Entity(_) => 1,
        ObjectRef::Rule(_) => 2,
        ObjectRef::Goal(_) => 3,
        ObjectRef::Relation(_) => 4,
        ObjectRef::Event(_) => 5,
        ObjectRef::Claim(_) => 6,
        ObjectRef::Document(_) => 7,
    }
}

fn delete_rank(value: ObjectRef) -> u8 {
    match value {
        ObjectRef::Document(_) => 0,
        ObjectRef::Claim(_) => 1,
        ObjectRef::Event(_) => 2,
        ObjectRef::Relation(_) => 3,
        ObjectRef::Goal(_) => 4,
        ObjectRef::Rule(_) => 5,
        ObjectRef::Entity(_) => 6,
        ObjectRef::World(_) => 7,
    }
}

fn read_regular_file(path: &Path, limit: u64) -> Result<Vec<u8>, AppError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| invalid(path, format!("cannot inspect file: {error}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err(invalid(
            path,
            "file is not a supported regular file".to_owned(),
        ));
    }
    fs::read(path).map_err(|error| invalid(path, format!("cannot read file: {error}")))
}

fn read_directory(path: &Path) -> Result<Vec<fs::DirEntry>, AppError> {
    fs::read_dir(path)
        .map_err(|error| invalid(path, format!("cannot read directory: {error}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid(path, format!("cannot inspect directory entry: {error}")))
}

fn current_time_ms() -> Result<i64, AppError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::ClockBeforeUnixEpoch)?
        .as_millis();
    i64::try_from(millis).map_err(|_| AppError::ClockOutOfRange)
}

fn invalid(path: &Path, reason: String) -> AppError {
    AppError::InvalidSnapshotImport {
        path: path.to_path_buf(),
        reason,
    }
}
