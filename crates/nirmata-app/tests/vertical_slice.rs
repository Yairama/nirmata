use nirmata_app::{AppError, CreateWorldInput, NirmataApp};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

#[test]
fn creates_closes_and_reopens_world_from_disk() {
    let path = project_path("vertical-slice");
    let unused_path = project_path("must-not-create");
    let mut app = NirmataApp::default();

    let created = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "The Memory Empire".to_owned(),
            premise_md: "A mineral can store memories.".to_owned(),
            epoch_label: "Before the Collapse".to_owned(),
        })
        .expect("create world");

    assert!(path.is_file());
    assert!(matches!(
        app.create_world(CreateWorldInput {
            path: unused_path.clone(),
            name: "Another world".to_owned(),
            premise_md: String::new(),
            epoch_label: String::new(),
        })
        .expect_err("a second world must not open"),
        AppError::WorldAlreadyOpen
    ));
    assert!(!unused_path.exists());

    app.close_world().expect("close world");
    drop(app);

    let child = Command::new(env::current_exe().expect("current test executable"))
        .args(["--exact", "child_process_reopens_world", "--nocapture"])
        .env("NIRMATA_TEST_PROJECT", &path)
        .env("NIRMATA_TEST_WORLD_ID", created.world_id.to_string())
        .env(
            "NIRMATA_TEST_REVISION_ID",
            created.current_revision.to_string(),
        )
        .output()
        .expect("run reopen process");
    assert!(
        child.status.success(),
        "child process failed:\n{}",
        String::from_utf8_lossy(&child.stderr)
    );

    let mut restarted_app = NirmataApp::default();
    let reopened = restarted_app
        .open_world(path.clone())
        .expect("reopen persisted world");
    assert_eq!(reopened.world_id, created.world_id);
    assert_eq!(reopened.current_revision, created.current_revision);
    assert_eq!(reopened.world.name(), "The Memory Empire");
    assert_eq!(reopened.world.premise_md(), "A mineral can store memories.");
    restarted_app.close_world().expect("close reopened world");
    drop(restarted_app);

    let mut verification_app = NirmataApp::default();
    verification_app
        .open_world(path.clone())
        .expect("project remains valid after close");
    verification_app.close_world().expect("final close");
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn child_process_reopens_world() {
    let Ok(path) = env::var("NIRMATA_TEST_PROJECT") else {
        return;
    };
    let expected_world_id = env::var("NIRMATA_TEST_WORLD_ID").expect("expected world id");
    let expected_revision_id = env::var("NIRMATA_TEST_REVISION_ID").expect("expected revision id");
    let mut app = NirmataApp::default();

    let reopened = app
        .open_world(PathBuf::from(path))
        .expect("child process reopens world");
    assert_eq!(reopened.world_id.to_string(), expected_world_id);
    assert_eq!(reopened.current_revision.to_string(), expected_revision_id);
    app.close_world().expect("child process closes world");
}

#[test]
fn reports_actionable_open_errors() {
    let missing = project_path("missing");
    let mut app = NirmataApp::default();
    let error = app
        .open_world(missing.clone())
        .expect_err("missing file must fail");

    assert!(matches!(error, AppError::FileNotFound(ref path) if path == &missing));
    assert!(error.to_string().contains("was not found"));
}
