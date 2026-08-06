use super::{CommandError, parse_object_uri, parse_project_path, parse_revision_id};
use nirmata_app::{AiError, AppError};
use std::path::Path;
use std::time::Duration;

#[test]
fn rejecting_invalid_project_paths_happens_before_opening_files() {
    let error = parse_project_path(Path::new("C:\\data\\world.txt"))
        .expect_err("only .nirmata files are accepted");
    assert_eq!(error.code, "invalid_project_path");
}

#[test]
fn rejecting_invalid_object_uris_happens_before_dispatch() {
    let error =
        parse_object_uri("javascript:alert(1)").expect_err("only nirmata:// URIs are accepted");
    assert_eq!(error.code, "invalid_object_uri");
}

#[test]
fn rejecting_invalid_revision_ids_happens_before_undo() {
    let error = parse_revision_id("not-a-revision-id").expect_err("revision ids must be UUIDs");
    assert_eq!(error.code, "invalid_revision_id");
}

#[test]
fn provider_errors_map_to_stable_command_codes() {
    let timeout = CommandError::from(AppError::Ai(AiError::RequestTimedOut(Duration::from_secs(
        5,
    ))));
    assert_eq!(timeout.code, "provider_timeout");

    let cancelled = CommandError::from(AppError::Ai(AiError::RequestCancelled));
    assert_eq!(cancelled.code, "provider_cancelled");
}
