use nirmata_ai::AiError;
use nirmata_core::{DomainError, RevisionId, VariantId};
use nirmata_store::StoreError;
use std::{error::Error, fmt, io, path::PathBuf};

#[derive(Debug)]
pub enum AppError {
    WorldAlreadyOpen,
    NoWorldOpen,
    ReadOnlyScope,
    ManualReviewNotReady,
    ManualReviewStale {
        base_revision: RevisionId,
        current_revision: RevisionId,
    },
    ManualReviewVariantMismatch {
        expected: VariantId,
        found: VariantId,
    },
    ManualReviewRevalidationFailed,
    NoUndoableRevision,
    UndoTargetNotCurrentLogicalAncestor {
        expected: RevisionId,
        found: RevisionId,
    },
    UndoConflict {
        target_revision: RevisionId,
        reason: String,
    },
    FileAlreadyExists(PathBuf),
    FileNotFound(PathBuf),
    InvalidProjectPath(PathBuf),
    InvalidProjectFormat(PathBuf),
    IncompatibleSchema {
        path: PathBuf,
        found: i64,
        supported: i64,
    },
    ProjectLocked(PathBuf),
    CorruptProject(PathBuf, String),
    InvalidSnapshotParent(PathBuf),
    InvalidSnapshotName(String),
    SnapshotDestinationOccupied(PathBuf),
    SnapshotIo {
        path: PathBuf,
        source: io::Error,
    },
    SnapshotSerialization(serde_json::Error),
    InvalidSnapshotImport {
        path: PathBuf,
        reason: String,
    },
    SnapshotHasNoChanges,
    InvalidLoreImport {
        path: PathBuf,
        reason: String,
    },
    LoreImportBatchNotFound(String),
    InvalidObjectUri(String),
    ObjectNotFound {
        object: &'static str,
        id: String,
    },
    ReviewSessionNotFound(String),
    ReviewSessionConflict(String),
    UnknownReviewOperation(nirmata_core::ChangeOperationId),
    UnknownReviewDecision(nirmata_core::DecisionPointId),
    InvalidReviewDecisionAlternative {
        decision_point_id: nirmata_core::DecisionPointId,
        alternative: String,
    },
    ReviewIssueNotFound {
        operation_id: nirmata_core::ChangeOperationId,
        issue_code: String,
    },
    CannotWaiveHardIssue {
        operation_id: nirmata_core::ChangeOperationId,
        issue_code: String,
    },
    AiBaseRevisionMismatch {
        draft_base_revision: RevisionId,
        current_revision: RevisionId,
    },
    AiRunNotFound(String),
    DeepReviewRunNotFound(String),
    InvalidDeepReview(String),
    AiCritiqueIssueNotFound {
        run_id: String,
        issue_id: String,
    },
    InvalidAiRunTransition {
        run_id: String,
        status: &'static str,
        action: &'static str,
    },
    Ai(AiError),
    Domain(DomainError),
    Storage(StoreError),
    ClockBeforeUnixEpoch,
    ClockOutOfRange,
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorldAlreadyOpen => {
                write!(formatter, "close the current world before opening another")
            }
            Self::NoWorldOpen => write!(formatter, "no world is currently open"),
            Self::ReadOnlyScope => write!(
                formatter,
                "the viewed scope is read-only; return to the active variant head before writing"
            ),
            Self::ManualReviewNotReady => write!(
                formatter,
                "manual review still has unresolved issues and cannot be confirmed"
            ),
            Self::ManualReviewStale {
                base_revision,
                current_revision,
            } => write!(
                formatter,
                "manual review is stale: base revision {base_revision} is behind current head {current_revision}"
            ),
            Self::ManualReviewVariantMismatch { expected, found } => write!(
                formatter,
                "manual review belongs to variant {found}; active write variant is {expected}"
            ),
            Self::ManualReviewRevalidationFailed => write!(
                formatter,
                "manual review became invalid during commit revalidation"
            ),
            Self::NoUndoableRevision => write!(formatter, "there is no committed revision to undo"),
            Self::UndoTargetNotCurrentLogicalAncestor { expected, found } => write!(
                formatter,
                "revision {found} is not the current logical undo target; undo {expected} first"
            ),
            Self::UndoConflict {
                target_revision,
                reason,
            } => write!(
                formatter,
                "revision {target_revision} cannot be undone cleanly: {reason}"
            ),
            Self::FileAlreadyExists(path) => {
                write!(
                    formatter,
                    "{} already exists; choose another file",
                    path.display()
                )
            }
            Self::FileNotFound(path) => {
                write!(formatter, "{} was not found", path.display())
            }
            Self::InvalidProjectPath(path) => write!(
                formatter,
                "{} must be a file ending in .nirmata",
                path.display()
            ),
            Self::InvalidProjectFormat(path) => {
                write!(
                    formatter,
                    "{} is not a valid Nirmata project",
                    path.display()
                )
            }
            Self::IncompatibleSchema {
                path,
                found,
                supported,
            } => write!(
                formatter,
                "{} uses schema version {found}; update Nirmata (supported: {supported})",
                path.display()
            ),
            Self::ProjectLocked(path) => write!(
                formatter,
                "{} is in use; close it in the other process and try again",
                path.display()
            ),
            Self::CorruptProject(path, details) => {
                write!(formatter, "{} is corrupt: {details}", path.display())
            }
            Self::InvalidSnapshotParent(path) => write!(
                formatter,
                "{} must be an existing, non-symbolic-link directory",
                path.display()
            ),
            Self::InvalidSnapshotName(name) => write!(
                formatter,
                "snapshot name {name:?} must contain only letters, numbers, '-' or '_'"
            ),
            Self::SnapshotDestinationOccupied(path) => write!(
                formatter,
                "{} already exists; choose an empty destination name",
                path.display()
            ),
            Self::SnapshotIo { path, source } => {
                write!(
                    formatter,
                    "snapshot filesystem operation failed at {}: {source}",
                    path.display()
                )
            }
            Self::SnapshotSerialization(error) => {
                write!(formatter, "snapshot serialization failed: {error}")
            }
            Self::InvalidSnapshotImport { path, reason } => write!(
                formatter,
                "{} is not a safe, valid snapshot: {reason}",
                path.display()
            ),
            Self::SnapshotHasNoChanges => {
                write!(formatter, "the selected snapshot has no changes to review")
            }
            Self::InvalidLoreImport { path, reason } => write!(
                formatter,
                "{} is not a safe UTF-8 lore source: {reason}",
                path.display()
            ),
            Self::LoreImportBatchNotFound(batch_id) => {
                write!(formatter, "lore import batch {batch_id} was not found")
            }
            Self::InvalidObjectUri(uri) => write!(formatter, "invalid nirmata URI {uri}"),
            Self::ObjectNotFound { object, id } => write!(formatter, "{object} {id} was not found"),
            Self::ReviewSessionNotFound(review_key) => {
                write!(formatter, "manual review {review_key} was not found")
            }
            Self::ReviewSessionConflict(review_key) => write!(
                formatter,
                "another pending review already targets {review_key}; finish or discard it first"
            ),
            Self::UnknownReviewOperation(operation_id) => {
                write!(
                    formatter,
                    "manual review operation {operation_id} was not found"
                )
            }
            Self::UnknownReviewDecision(decision_point_id) => write!(
                formatter,
                "manual review decision {decision_point_id} was not found"
            ),
            Self::InvalidReviewDecisionAlternative {
                decision_point_id,
                alternative,
            } => write!(
                formatter,
                "{alternative} is not an alternative for manual review decision {decision_point_id}"
            ),
            Self::ReviewIssueNotFound {
                operation_id,
                issue_code,
            } => write!(
                formatter,
                "issue {issue_code} was not found for operation {operation_id}"
            ),
            Self::CannotWaiveHardIssue {
                operation_id,
                issue_code,
            } => write!(
                formatter,
                "issue {issue_code} for operation {operation_id} is a hard error and cannot be waived"
            ),
            Self::AiBaseRevisionMismatch {
                draft_base_revision,
                current_revision,
            } => write!(
                formatter,
                "AI request is stale: base revision {draft_base_revision} is behind current head {current_revision}"
            ),
            Self::AiRunNotFound(run_id) => write!(formatter, "AI run {run_id} was not found"),
            Self::DeepReviewRunNotFound(run_id) => {
                write!(formatter, "deep review run {run_id} was not found")
            }
            Self::InvalidDeepReview(message) => write!(formatter, "invalid deep review: {message}"),
            Self::AiCritiqueIssueNotFound { run_id, issue_id } => write!(
                formatter,
                "final critique issue {issue_id} was not found for AI run {run_id}"
            ),
            Self::InvalidAiRunTransition {
                run_id,
                status,
                action,
            } => write!(
                formatter,
                "AI run {run_id} cannot {action} while its status is {status}"
            ),
            Self::Ai(error) => error.fmt(formatter),
            Self::Domain(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
            Self::ClockBeforeUnixEpoch => {
                write!(formatter, "system clock is before the Unix epoch")
            }
            Self::ClockOutOfRange => {
                write!(formatter, "system clock is outside the supported range")
            }
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Ai(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::SnapshotIo { source, .. } => Some(source),
            Self::SnapshotSerialization(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AiError> for AppError {
    fn from(error: AiError) -> Self {
        Self::Ai(error)
    }
}

impl From<DomainError> for AppError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::InvalidExtension(path) => Self::InvalidProjectPath(path),
            StoreError::AlreadyExists(path) => Self::FileAlreadyExists(path),
            StoreError::NotFound(path) => Self::FileNotFound(path),
            StoreError::InvalidFormat(path) => Self::InvalidProjectFormat(path),
            StoreError::IncompatibleSchema {
                path,
                found,
                supported,
            } => Self::IncompatibleSchema {
                path,
                found,
                supported,
            },
            StoreError::Locked(path) => Self::ProjectLocked(path),
            StoreError::Corrupt(path, details) => Self::CorruptProject(path, details),
            StoreError::InvalidObjectUri(uri) => Self::InvalidObjectUri(uri),
            StoreError::ObjectNotFound { object, id } => Self::ObjectNotFound { object, id },
            other => Self::Storage(other),
        }
    }
}
