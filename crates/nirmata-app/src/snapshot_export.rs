use crate::{AppError, NirmataApp};
use nirmata_core::document::{ContentReference, ObjectRef};
use nirmata_store::CanonSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) const SNAPSHOT_FORMAT: &str = "nirmata-vfs-snapshot";
pub(crate) const SNAPSHOT_FORMAT_VERSION: u32 = 1;
pub(crate) const TYPE_DIRECTORIES: [&str; 8] = [
    "worlds",
    "entities",
    "relations",
    "events",
    "claims",
    "rules",
    "goals",
    "documents",
];
static STAGING_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportSnapshotInput {
    pub parent_directory: PathBuf,
    pub snapshot_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSnapshotResult {
    pub path: PathBuf,
    pub world_id: String,
    pub base_revision: String,
    pub logical_hash: String,
    pub object_count: usize,
    pub variant_id: String,
    pub variant: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotObject {
    pub(crate) object_type: String,
    pub(crate) id: String,
    pub(crate) uri: String,
    pub(crate) path: String,
    pub(crate) content_start_byte: usize,
    pub(crate) content_hash: String,
    pub(crate) metadata_hash: String,
    pub(crate) metadata: Value,
    pub(crate) references: Vec<SnapshotReference>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotReference {
    pub(crate) source_uri: String,
    pub(crate) target_uri: String,
    pub(crate) ordinal: u32,
}

#[derive(Serialize)]
pub(crate) struct LogicalSnapshot<'a> {
    pub(crate) format: &'static str,
    pub(crate) format_version: u32,
    pub(crate) hash_algorithm: &'static str,
    pub(crate) world_id: String,
    pub(crate) variant: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) variant_id: Option<&'a str>,
    pub(crate) base_revision: String,
    pub(crate) canon_schema_version: i64,
    pub(crate) objects: &'a [SnapshotObject],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotManifest {
    pub(crate) format: String,
    pub(crate) format_version: u32,
    pub(crate) hash_algorithm: String,
    pub(crate) world_id: String,
    pub(crate) variant: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) variant_id: Option<String>,
    pub(crate) base_revision: String,
    pub(crate) canon_schema_version: i64,
    pub(crate) logical_hash: String,
    pub(crate) objects: Vec<SnapshotObject>,
}

struct PreparedSnapshot {
    world_id: String,
    base_revision: String,
    logical_hash: String,
    files: Vec<(String, Vec<u8>)>,
    manifest: Vec<u8>,
    variant_id: String,
    variant: String,
}

impl NirmataApp {
    pub fn export_vfs_snapshot(
        &self,
        input: ExportSnapshotInput,
    ) -> Result<ExportSnapshotResult, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let parent = validate_parent_directory(&input.parent_directory)?;
        validate_snapshot_name(&input.snapshot_name)?;
        let destination = parent.join(&input.snapshot_name);
        ensure_destination_available(&destination)?;

        let variant = active
            .store
            .get_variant(active.read_scope.variant_id)?
            .ok_or_else(|| {
                nirmata_store::StoreError::InvalidVariant("viewed variant was not found".to_owned())
            })?;
        let prepared = prepare_snapshot(
            &active.store.read_canon_snapshot_scoped(active.read_scope)?,
            &variant,
        )?;
        publish_atomically(&parent, &input.snapshot_name, |staging| {
            write_prepared_snapshot(staging, &prepared)
        })?;

        Ok(ExportSnapshotResult {
            path: destination,
            world_id: prepared.world_id,
            base_revision: prepared.base_revision,
            logical_hash: prepared.logical_hash,
            object_count: prepared.files.len(),
            variant_id: prepared.variant_id,
            variant: prepared.variant,
        })
    }
}

fn validate_parent_directory(parent: &Path) -> Result<PathBuf, AppError> {
    if parent.as_os_str().is_empty() {
        return Err(AppError::InvalidSnapshotParent(parent.to_path_buf()));
    }
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| AppError::InvalidSnapshotParent(parent.to_path_buf()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::InvalidSnapshotParent(parent.to_path_buf()));
    }
    fs::canonicalize(parent).map_err(|source| AppError::SnapshotIo {
        path: parent.to_path_buf(),
        source,
    })
}

fn validate_snapshot_name(name: &str) -> Result<(), AppError> {
    if name.is_empty()
        || name.len() > 80
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(AppError::InvalidSnapshotName(name.to_owned()));
    }
    Ok(())
}

fn ensure_destination_available(destination: &Path) -> Result<(), AppError> {
    match fs::symlink_metadata(destination) {
        Ok(_) => Err(AppError::SnapshotDestinationOccupied(
            destination.to_path_buf(),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(AppError::SnapshotIo {
            path: destination.to_path_buf(),
            source,
        }),
    }
}

fn prepare_snapshot(
    snapshot: &CanonSnapshot,
    variant: &nirmata_store::Variant,
) -> Result<PreparedSnapshot, AppError> {
    let world = snapshot.world();
    let world_id = world.id().to_string();
    let base_revision = world.current_revision().to_string();
    let variant_id = variant.id.to_string();
    let mut objects = Vec::new();
    let mut files = Vec::new();

    add_object(
        &mut objects,
        &mut files,
        ObjectRef::World(world.id()),
        world,
        world.premise_md(),
        snapshot.content_references(),
        &world_id,
        &base_revision,
        &variant.name,
    )?;
    for entity in snapshot.entities() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Entity(entity.id()),
            entity,
            entity.body_md(),
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }
    for relation in snapshot.relations() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Relation(relation.id()),
            relation,
            "",
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }
    for event in snapshot.events() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Event(event.event().id()),
            event,
            event.event().body_md(),
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }
    for claim in snapshot.claims() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Claim(claim.id()),
            claim,
            claim.content_md(),
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }
    for rule in snapshot.rules() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Rule(rule.id()),
            rule,
            rule.statement_md(),
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }
    for goal in snapshot.goals() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Goal(goal.id()),
            goal,
            goal.desired_state_md(),
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }
    for document in snapshot.documents() {
        add_object(
            &mut objects,
            &mut files,
            ObjectRef::Document(document.object().id()),
            document,
            document.object().body_md(),
            snapshot.content_references(),
            &world_id,
            &base_revision,
            &variant.name,
        )?;
    }

    let logical = LogicalSnapshot {
        format: SNAPSHOT_FORMAT,
        format_version: SNAPSHOT_FORMAT_VERSION,
        hash_algorithm: "sha256",
        world_id: world_id.clone(),
        variant: &variant.name,
        variant_id: Some(&variant_id),
        base_revision: base_revision.clone(),
        canon_schema_version: snapshot.schema_version(),
        objects: &objects,
    };
    let logical_hash = hash_json(&logical)?;
    let manifest = SnapshotManifest {
        format: SNAPSHOT_FORMAT.to_owned(),
        format_version: SNAPSHOT_FORMAT_VERSION,
        hash_algorithm: "sha256".to_owned(),
        world_id: world_id.clone(),
        variant: variant.name.clone(),
        variant_id: Some(variant_id.clone()),
        base_revision: base_revision.clone(),
        canon_schema_version: snapshot.schema_version(),
        logical_hash: logical_hash.clone(),
        objects: objects.clone(),
    };
    let mut manifest =
        serde_json::to_vec_pretty(&manifest).map_err(AppError::SnapshotSerialization)?;
    manifest.push(b'\n');

    Ok(PreparedSnapshot {
        world_id,
        base_revision,
        logical_hash,
        files,
        manifest,
        variant_id,
        variant: variant.name.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn add_object<T: Serialize>(
    objects: &mut Vec<SnapshotObject>,
    files: &mut Vec<(String, Vec<u8>)>,
    object: ObjectRef,
    value: &T,
    prose: &str,
    all_references: &[ContentReference],
    world_id: &str,
    base_revision: &str,
    variant_name: &str,
) -> Result<(), AppError> {
    let object_type = object.kind();
    let id = object_id(object);
    let uri = object.to_string();
    let path = format!("{}/{}.md", type_directory(object_type), id);
    let prefix = format!(
        "# Nirmata {object_type} {id}\n\n- URI: `{uri}`\n- World ID: `{world_id}`\n- Variant: `{variant_name}`\n- Base revision: `{base_revision}`\n\n## Content\n\n"
    );
    let content_start_byte = prefix.len();
    let mut bytes = prefix.into_bytes();
    bytes.extend_from_slice(prose.as_bytes());

    let mut metadata = serde_json::to_value(value).map_err(AppError::SnapshotSerialization)?;
    remove_prose_and_embedded_references(object_type, &mut metadata);
    let references = all_references
        .iter()
        .filter(|reference| reference.source() == object)
        .map(|reference| SnapshotReference {
            source_uri: reference.source().to_string(),
            target_uri: reference.target().to_string(),
            ordinal: reference.ordinal(),
        })
        .collect();
    objects.push(SnapshotObject {
        object_type: object_type.to_owned(),
        id,
        uri,
        path: path.clone(),
        content_start_byte,
        content_hash: hash_bytes(&bytes),
        metadata_hash: hash_json(&metadata)?,
        metadata,
        references,
    });
    files.push((path, bytes));
    Ok(())
}

pub(crate) fn remove_prose_and_embedded_references(object_type: &str, metadata: &mut Value) {
    let prose_field = match object_type {
        "world" => "premise_md",
        "entity" | "event" | "document" => "body_md",
        "claim" => "content_md",
        "rule" => "statement_md",
        "goal" => "desired_state_md",
        "relation" => return,
        _ => return,
    };
    let object = match object_type {
        "event" => metadata.get_mut("event"),
        "document" => {
            if let Some(map) = metadata.as_object_mut() {
                map.remove("references");
            }
            metadata.get_mut("object")
        }
        _ => Some(metadata),
    };
    if let Some(Value::Object(map)) = object {
        map.remove(prose_field);
    }
}

pub(crate) fn hash_json(value: &impl Serialize) -> Result<String, AppError> {
    serde_json::to_value(value)
        .and_then(|value| serde_json::to_vec(&value))
        .map(|bytes| hash_bytes(&bytes))
        .map_err(AppError::SnapshotSerialization)
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(crate) fn type_directory(object_type: &str) -> &'static str {
    match object_type {
        "world" => "worlds",
        "entity" => "entities",
        "relation" => "relations",
        "event" => "events",
        "claim" => "claims",
        "rule" => "rules",
        "goal" => "goals",
        "document" => "documents",
        _ => unreachable!("ObjectRef returned an unknown kind"),
    }
}

fn object_id(object: ObjectRef) -> String {
    match object {
        ObjectRef::World(id) => id.to_string(),
        ObjectRef::Entity(id) => id.to_string(),
        ObjectRef::Relation(id) => id.to_string(),
        ObjectRef::Event(id) => id.to_string(),
        ObjectRef::Claim(id) => id.to_string(),
        ObjectRef::Rule(id) => id.to_string(),
        ObjectRef::Goal(id) => id.to_string(),
        ObjectRef::Document(id) => id.to_string(),
    }
}

fn write_prepared_snapshot(staging: &Path, prepared: &PreparedSnapshot) -> Result<(), AppError> {
    for directory in TYPE_DIRECTORIES {
        let path = staging.join(directory);
        fs::create_dir(&path).map_err(|source| AppError::SnapshotIo { path, source })?;
    }
    for (relative_path, bytes) in &prepared.files {
        write_new_file(&staging.join(relative_path), bytes)?;
    }
    write_new_file(&staging.join("manifest.json"), &prepared.manifest)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| AppError::SnapshotIo {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|source| AppError::SnapshotIo {
            path: path.to_path_buf(),
            source,
        })
}

fn publish_atomically(
    parent: &Path,
    snapshot_name: &str,
    write: impl FnOnce(&Path) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let destination = parent.join(snapshot_name);
    ensure_destination_available(&destination)?;
    let mut staging = StagingDirectory::create(parent)?;
    write(staging.path())?;
    ensure_destination_available(&destination)?;
    fs::rename(staging.path(), &destination).map_err(|source| AppError::SnapshotIo {
        path: destination,
        source,
    })?;
    staging.published = true;
    Ok(())
}

struct StagingDirectory {
    path: PathBuf,
    published: bool,
}

impl StagingDirectory {
    fn create(parent: &Path) -> Result<Self, AppError> {
        for _ in 0..100 {
            let nonce = STAGING_NONCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".nirmata-export-{}-{nonce}.staging",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(AppError::SnapshotIo { path, source }),
            }
        }
        let path = parent.join(".nirmata-export.staging");
        Err(AppError::SnapshotIo {
            path,
            source: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not reserve a unique snapshot staging directory",
            ),
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn induced_staging_failure_is_cleaned_without_publishing() {
        let parent = std::env::temp_dir().join(format!(
            "nirmata-export-failure-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir(&parent).expect("create test parent");

        let error = publish_atomically(&parent, "snapshot", |staging| {
            let partial = staging.join("partial.md");
            fs::File::create(&partial).expect("create partial staging file");
            Err(AppError::SnapshotIo {
                path: partial,
                source: io::Error::other("induced write failure"),
            })
        })
        .expect_err("induced failure must abort publication");

        assert!(matches!(error, AppError::SnapshotIo { .. }));
        assert!(!parent.join("snapshot").exists());
        assert_eq!(
            fs::read_dir(&parent).expect("read test parent").count(),
            0,
            "staging must be removed"
        );
        fs::remove_dir(parent).expect("remove test parent");
    }
}
