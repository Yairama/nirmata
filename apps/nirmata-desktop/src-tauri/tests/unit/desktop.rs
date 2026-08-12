use super::{
    CommandError, SimulationScenarioCommand, apply_manual_review_action, apply_manual_review_edit,
    begin_manual_review_edit, close_world, confirm_manual_review, create_world, dotenv_value,
    get_current_world, get_provider_credential_status, get_related_context, list_revision_history,
    list_simulation_scenarios, list_timeline_events, open_uri, open_world, parse_ai_run_id,
    parse_deep_review_mode, parse_object_uri, parse_project_path, parse_revision_id,
    parse_simulation_scenario_id, parse_snapshot_directory, parse_snapshot_name,
    parse_snapshot_parent, preview_manual_draft, read_logical_vfs, read_manual_review,
    revalidate_manual_review, run_simulation_scenario, search_world, undo_revision,
};
use nirmata_app::{AiError, AppError, NirmataApp, StoreError};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

#[test]
fn rejecting_invalid_project_paths_happens_before_opening_files() {
    let error = parse_project_path(Path::new("C:\\data\\world.txt"))
        .expect_err("only .nirmata files are accepted");
    assert_eq!(error.code, "invalid_project_path");
    assert!(error.message.contains(".nirmata"));
}

#[test]
fn rejecting_invalid_object_uris_happens_before_dispatch() {
    let error =
        parse_object_uri("javascript:alert(1)").expect_err("only nirmata:// URIs are accepted");
    assert_eq!(error.code, "invalid_object_uri");
    assert!(error.message.contains("invalid nirmata URI"));
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
    assert!(cancelled.message.contains("cancelled"));

    let truncated = CommandError::from(AppError::Ai(AiError::InvalidResponse(
        "structured AI output is invalid: truncated JSON".to_owned(),
    )));
    assert_eq!(truncated.code, "provider_response_error");
}

#[test]
fn rejecting_unsafe_snapshot_paths_happens_before_dispatch() {
    let relative =
        parse_snapshot_parent(Path::new("exports")).expect_err("snapshot parent must be absolute");
    assert_eq!(relative.code, "invalid_snapshot_parent");
    let import_relative = parse_snapshot_directory(Path::new("snapshot"))
        .expect_err("snapshot import directory must be absolute");
    assert_eq!(import_relative.code, "invalid_snapshot_directory");

    for unsafe_name in [
        "",
        ".hidden",
        "../escape",
        "nested/path",
        "name with spaces",
    ] {
        let error = parse_snapshot_name(unsafe_name).expect_err("unsafe name must be rejected");
        assert_eq!(error.code, "invalid_snapshot_name");
    }
    let safe_name = parse_snapshot_name("memory-world_01")
        .unwrap_or_else(|_| panic!("safe snapshot name must be accepted"));
    assert_eq!(safe_name, "memory-world_01");
}

#[test]
fn durable_storage_failures_map_to_recoverable_command_states() {
    let path = PathBuf::from("world.nirmata");
    let locked = CommandError::from(AppError::ProjectLocked(path.clone()));
    assert_eq!(locked.code, "project_locked");
    assert!(locked.message.contains("try again"));

    let constraint = CommandError::from(AppError::Storage(StoreError::Database(
        path.clone(),
        "UNIQUE constraint failed: entities.world_id, entities.slug".to_owned(),
    )));
    assert_eq!(constraint.code, "constraint_error");

    let derived_index = CommandError::from(AppError::Storage(StoreError::Database(
        path,
        "derived index update failed".to_owned(),
    )));
    assert_eq!(derived_index.code, "storage_error");

    let future_schema = CommandError::from(AppError::IncompatibleSchema {
        path: PathBuf::from("future.nirmata"),
        found: 7,
        supported: 6,
    });
    assert_eq!(future_schema.code, "incompatible_schema");
    assert!(future_schema.message.contains("update Nirmata"));
}

#[test]
fn development_env_parser_reads_only_the_requested_value() {
    let contents = "# local config\nBASE_URL='https://example.test'\nPROVIDER_API_KEY=secret\n";
    assert_eq!(
        dotenv_value(contents, "BASE_URL").as_deref(),
        Some("https://example.test")
    );
    assert_eq!(dotenv_value(contents, "MISSING"), None);
}

#[test]
fn invalid_ai_run_ids_are_rejected_before_dispatch() {
    let error = parse_ai_run_id("not-a-run-id").expect_err("run ids must be UUIDs");
    assert_eq!(error.code, "invalid_ai_run_id");
}

#[test]
fn simulation_scenario_ids_and_command_dtos_are_strict() {
    let id = "1f2c8be0-093a-4f31-b6b4-c8db7c1fa2da";
    assert_eq!(
        parse_simulation_scenario_id(id)
            .unwrap_or_else(|_| panic!("valid simulation scenario id"))
            .to_string(),
        id
    );
    let error =
        parse_simulation_scenario_id("not-a-scenario").expect_err("scenario ids must be UUIDs");
    assert_eq!(error.code, "invalid_simulation_scenario_id");

    assert!(
        serde_json::from_value::<SimulationScenarioCommand>(json!({
            "scenarioId": id,
            "unexpected": true
        }))
        .is_err(),
        "command DTOs must reject unknown fields"
    );
}

#[test]
fn deep_review_mode_is_explicit_and_closed() {
    assert_eq!(
        parse_deep_review_mode("deep_impact").unwrap_or_else(|_| panic!("deep mode")),
        nirmata_app::DeepReviewMode::DeepImpact
    );
    assert_eq!(
        parse_deep_review_mode("audit").unwrap_or_else(|_| panic!("audit mode")),
        nirmata_app::DeepReviewMode::Audit
    );
    assert_eq!(
        parse_deep_review_mode("propose")
            .expect_err("standard proposal cannot become deep silently")
            .code,
        "invalid_deep_review_mode"
    );
}

fn acceptance_project_path() -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create acceptance directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    directory.join(format!(
        "foundation-acceptance-{}-{nonce}.nirmata",
        std::process::id()
    ))
}

fn invoke_ipc(
    webview: &WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost"
                .parse()
                .expect("valid Tauri test URL"),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_owned(),
        },
    )
    .map(|body| body.deserialize::<Value>().expect("JSON IPC response"))
}

fn invoke_ok(webview: &WebviewWindow<MockRuntime>, command: &str, body: Value) -> Value {
    invoke_ipc(webview, command, body)
        .unwrap_or_else(|error| panic!("{command} failed across IPC: {error}"))
}

#[test]
fn simulation_commands_preserve_boundary_errors_across_ipc() {
    let app = mock_builder()
        .manage(Arc::new(Mutex::new(NirmataApp::default())))
        .invoke_handler(tauri::generate_handler![
            list_simulation_scenarios,
            run_simulation_scenario,
        ])
        .build(mock_context(noop_assets()))
        .expect("build simulation command app");
    let webview = WebviewWindowBuilder::new(&app, "simulation", Default::default())
        .build()
        .expect("build simulation command webview");

    let no_world = invoke_ipc(&webview, "list_simulation_scenarios", json!({}))
        .expect_err("listing without a world must fail");
    assert_eq!(no_world["code"], "no_world_open");

    let invalid_id = invoke_ipc(
        &webview,
        "run_simulation_scenario",
        json!({ "input": { "scenarioId": "not-a-scenario" } }),
    )
    .expect_err("invalid scenario ids must fail at the command boundary");
    assert_eq!(invalid_id["code"], "invalid_simulation_scenario_id");
}

fn preview_form(
    webview: &WebviewWindow<MockRuntime>,
    object_type: &str,
    existing_uri: Option<&str>,
    values: Value,
) -> Value {
    let response = invoke_ok(
        webview,
        "preview_manual_draft",
        json!({
            "input": {
                "objectType": object_type,
                "existingUri": existing_uri,
                "objective": format!("Foundation acceptance: edit {object_type}"),
                "sourceUris": existing_uri.into_iter().collect::<Vec<_>>(),
                "assumptions": [],
                "values": values,
            }
        }),
    );
    assert_eq!(response["fieldIssues"], json!([]), "{object_type} fields");
    assert_eq!(response["draft"]["objectType"], object_type);
    assert_eq!(
        response["draft"]["mode"],
        if existing_uri.is_some() {
            "update"
        } else {
            "create"
        }
    );
    response
}

fn apply_action(webview: &WebviewWindow<MockRuntime>, review_key: &str, action: Value) -> Value {
    invoke_ok(
        webview,
        "apply_manual_review_action",
        json!({ "input": { "reviewKey": review_key, "action": action } }),
    )
}

fn prepare_review_for_commit(webview: &WebviewWindow<MockRuntime>, response: &Value) -> Value {
    let review_key = response["review"]["reviewKey"]
        .as_str()
        .expect("review key");
    let operation_id = response["review"]["operations"][0]["operationId"]
        .as_str()
        .expect("operation id");
    let mut review = apply_action(
        webview,
        review_key,
        json!({ "kind": "accept", "operationId": operation_id }),
    );
    if review["operations"][0]["risk"]["requiresJudgment"] == true
        && review["operations"][0]["risk"]["judgment"].is_null()
    {
        review = apply_action(
            webview,
            review_key,
            json!({
                "kind": "record_judgment",
                "operationId": operation_id,
                "judgment": "I reviewed the cited before and after state.",
            }),
        );
    }
    assert_eq!(
        review["readyToConfirm"], true,
        "review must be confirmable: {review}"
    );
    review
}

fn commit_form(
    webview: &WebviewWindow<MockRuntime>,
    object_type: &str,
    existing_uri: Option<&str>,
    values: Value,
) -> (String, Value) {
    let response = preview_form(webview, object_type, existing_uri, values);
    let review = prepare_review_for_commit(webview, &response);
    let review_key = review["reviewKey"].as_str().expect("review key");
    let session = invoke_ok(
        webview,
        "confirm_manual_review",
        json!({ "input": { "reviewKey": review_key } }),
    );
    (
        response["draft"]["targetUri"]
            .as_str()
            .expect("target URI")
            .to_owned(),
        session,
    )
}

fn acceptance_webview() -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let mut state = NirmataApp::default();
    state
        .set_session_provider_api_key("FOUNDATION_ACCEPTANCE_SECRET".to_owned())
        .expect("configure an in-memory acceptance credential");
    let app = mock_builder()
        .manage(Arc::new(Mutex::new(state)))
        .invoke_handler(tauri::generate_handler![
            create_world,
            open_world,
            get_current_world,
            search_world,
            open_uri,
            get_related_context,
            read_logical_vfs,
            get_provider_credential_status,
            preview_manual_draft,
            apply_manual_review_action,
            read_manual_review,
            begin_manual_review_edit,
            apply_manual_review_edit,
            revalidate_manual_review,
            confirm_manual_review,
            list_timeline_events,
            list_revision_history,
            undo_revision,
            close_world,
        ])
        .build(mock_context(noop_assets()))
        .expect("build mock Tauri app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");
    (app, webview)
}

#[test]
fn foundation_acceptance_traverses_frontend_ipc_commands_and_persisted_app_workflow() {
    let path = acceptance_project_path();
    let (_app, webview) = acceptance_webview();

    let created = invoke_ok(
        &webview,
        "create_world",
        json!({
            "input": {
                "path": path,
                "name": "Foundation World",
                "premise_md": "A mine stores disputed memories.",
                "epoch_label": "First Bell",
            }
        }),
    );
    let world_uri = format!(
        "nirmata://world/{}",
        created["world"]["id"].as_str().expect("world id")
    );
    let initial_revision = created["current_revision"].clone();
    invoke_ok(&webview, "close_world", json!({}));
    let reopened = invoke_ok(&webview, "open_world", json!({ "path": path }));
    assert_eq!(reopened["current_revision"], initial_revision);

    let credential = invoke_ok(&webview, "get_provider_credential_status", json!({}));
    assert_eq!(credential["configured"], true);
    assert!(
        !credential
            .to_string()
            .contains("FOUNDATION_ACCEPTANCE_SECRET")
    );

    let (mara_uri, _) = commit_form(
        &webview,
        "entity",
        None,
        json!({
            "kind": "person", "name": "Mara", "slug": "mara",
            "aliases": "The Witness", "summary": "Mine archivist.",
            "body_md": "Mara records uncertain testimony.", "attributes_json": "{}"
        }),
    );

    let second_preview = preview_form(
        &webview,
        "entity",
        None,
        json!({
            "kind": "faction", "name": "Archive Guild", "slug": "archive-guild",
            "aliases": "", "summary": "Initial summary.", "body_md": "", "attributes_json": "{}"
        }),
    );
    let second_key = second_preview["review"]["reviewKey"]
        .as_str()
        .expect("second entity review key");
    let second_operation = second_preview["review"]["operations"][0]["operationId"]
        .as_str()
        .expect("second entity operation id");
    let mut edit_request = invoke_ok(
        &webview,
        "begin_manual_review_edit",
        json!({ "input": { "reviewKey": second_key, "operationId": second_operation } }),
    );
    edit_request["values"]["summary"] = json!("Edited before confirmation.");
    let edited = invoke_ok(
        &webview,
        "apply_manual_review_edit",
        json!({
            "input": {
                "reviewKey": second_key,
                "operationId": second_operation,
                "request": edit_request,
            }
        }),
    );
    assert_eq!(edited["review"]["operations"][0]["decision"], "edit");
    assert_eq!(edited["review"]["readyToConfirm"], true);
    invoke_ok(
        &webview,
        "confirm_manual_review",
        json!({ "input": { "reviewKey": second_key } }),
    );
    let guild_uri = edited["review"]["operations"][0]["targetUri"]
        .as_str()
        .expect("guild URI")
        .to_owned();

    let (rule_uri, _) = commit_form(
        &webview,
        "rule",
        None,
        json!({
            "kind": "institutional", "statement_md": "All testimony names its source.",
            "scope": "world", "severity": "advisory", "validator_kind": "",
            "source": "Archive charter", "parameters_json": "{}"
        }),
    );
    let (relation_uri, _) = commit_form(
        &webview,
        "relation",
        None,
        json!({
            "source_entity": guild_uri, "target_entity": mara_uri, "kind": "employs",
            "direction": "directed", "certainty": "approximate", "valid_from_tick": "1",
            "valid_to_tick": "20", "source_reference": "Guild ledger", "metadata_json": "{}"
        }),
    );
    let (goal_uri, _) = commit_form(
        &webview,
        "goal",
        None,
        json!({
            "holder_entity": mara_uri, "desired_state_md": "Preserve the mine archive.",
            "priority": "8", "status": "active", "visibility": "public",
            "source": "Mara's oath", "period_start_tick": "1", "period_end_tick": ""
        }),
    );
    let (event_uri, _) = commit_form(
        &webview,
        "event",
        None,
        json!({
            "kind": "collapse", "summary": "The memory mine collapses.",
            "body_md": "The cause remains unknown.", "time_kind": "interval",
            "time_precision": "day", "time_certainty": "approximate_uncertain",
            "start_tick": "10", "end_tick": "12", "location_entity": "",
            "participants": format!("{mara_uri}|witness|0"),
            "affected_goal_ids": goal_uri, "causal_links": ""
        }),
    );
    let (claim_uri, _) = commit_form(
        &webview,
        "claim",
        None,
        json!({
            "subject_entity": mara_uri, "content_md": "The Guild believes Mara did not cause the collapse.",
            "predicate_key": "mara.caused_collapse", "object_kind": "scalar", "object_value": "true",
            "polarity": "negative", "authentication": "attributed", "holder_entity": guild_uri,
            "modality": "belief", "register": "testimony", "epistemic_basis": "Guild hearing",
            "source": "Hearing record", "source_document": "", "source_claim": "",
            "holder_confidence": "0.6", "period_start_tick": "12", "period_end_tick": ""
        }),
    );
    let (document_uri, _) = commit_form(
        &webview,
        "document",
        None,
        json!({
            "title": "Collapse Chronicle", "kind": "chronicle", "author_entity": mara_uri,
            "perspective_entity": mara_uri, "canon_status": "canonical",
            "body_md": "Foundation acceptance marker.",
            "content_references": format!("{event_uri}|0\n{claim_uri}|1")
        }),
    );

    let updates = [
        (
            "world",
            world_uri.as_str(),
            json!({
                "name": "Foundation World", "premise_md": "A mine stores cited, disputed memories.",
                "epoch_label": "First Bell"
            }),
        ),
        (
            "entity",
            mara_uri.as_str(),
            json!({
                "kind": "person", "name": "Mara", "slug": "mara", "aliases": "The Witness",
                "summary": "Mine archivist and witness.", "body_md": "Mara records uncertain testimony.",
                "attributes_json": "{}"
            }),
        ),
        (
            "rule",
            rule_uri.as_str(),
            json!({
                "kind": "institutional", "statement_md": "All accepted testimony names its source.",
                "scope": "world", "severity": "advisory", "validator_kind": "",
                "source": "Revised archive charter", "parameters_json": "{}"
            }),
        ),
        (
            "relation",
            relation_uri.as_str(),
            json!({
                "source_entity": guild_uri, "target_entity": mara_uri, "kind": "employs",
                "direction": "directed", "certainty": "approximate", "valid_from_tick": "1",
                "valid_to_tick": "20", "source_reference": "Reviewed guild ledger", "metadata_json": "{}"
            }),
        ),
        (
            "goal",
            goal_uri.as_str(),
            json!({
                "holder_entity": mara_uri, "desired_state_md": "Preserve and cite the mine archive.",
                "priority": "8", "status": "active", "visibility": "public",
                "source": "Mara's oath", "period_start_tick": "1", "period_end_tick": ""
            }),
        ),
        (
            "event",
            event_uri.as_str(),
            json!({
                "kind": "collapse", "summary": "The memory mine collapses.",
                "body_md": "The cause and exact instant remain unknown.", "time_kind": "interval",
                "time_precision": "day", "time_certainty": "approximate_uncertain",
                "start_tick": "10", "end_tick": "12", "location_entity": "",
                "participants": format!("{mara_uri}|witness|0"),
                "affected_goal_ids": goal_uri, "causal_links": ""
            }),
        ),
        (
            "claim",
            claim_uri.as_str(),
            json!({
                "subject_entity": mara_uri, "content_md": "The Guild still believes Mara did not cause the collapse.",
                "predicate_key": "mara.caused_collapse", "object_kind": "scalar", "object_value": "true",
                "polarity": "negative", "authentication": "attributed", "holder_entity": guild_uri,
                "modality": "belief", "register": "testimony", "epistemic_basis": "Guild hearing",
                "source": "Reviewed hearing record", "source_document": "", "source_claim": "",
                "holder_confidence": "0.6", "period_start_tick": "12", "period_end_tick": ""
            }),
        ),
        (
            "document",
            document_uri.as_str(),
            json!({
                "title": "Collapse Chronicle", "kind": "chronicle", "author_entity": mara_uri,
                "perspective_entity": mara_uri, "canon_status": "canonical",
                "body_md": "Foundation acceptance marker with cited revisions.",
                "content_references": format!("{event_uri}|0\n{claim_uri}|1")
            }),
        ),
    ];
    for (object_type, uri, values) in updates {
        let (updated_uri, _) = commit_form(&webview, object_type, Some(uri), values);
        assert_eq!(updated_uri, uri);
    }

    let search = invoke_ok(
        &webview,
        "search_world",
        json!({ "input": { "queryText": "acceptance marker", "kind": "document", "limit": 10 } }),
    );
    assert_eq!(search["hits"][0]["uri"], document_uri);
    let opened = invoke_ok(&webview, "open_uri", json!({ "uri": document_uri }));
    assert_eq!(opened["result"]["uri"], document_uri);
    let related = invoke_ok(
        &webview,
        "get_related_context",
        json!({ "input": { "uri": document_uri } }),
    );
    assert!(related.to_string().contains(&event_uri));
    let vfs = invoke_ok(&webview, "read_logical_vfs", json!({}));
    for uri in [
        &mara_uri,
        &guild_uri,
        &rule_uri,
        &relation_uri,
        &goal_uri,
        &event_uri,
        &claim_uri,
        &document_uri,
    ] {
        assert!(vfs.to_string().contains(uri), "VFS must contain {uri}");
    }
    let timeline = invoke_ok(&webview, "list_timeline_events", json!({}));
    assert!(timeline.to_string().contains(&event_uri));

    let stale_preview = preview_form(
        &webview,
        "entity",
        None,
        json!({
            "kind": "person", "name": "Stale Witness", "slug": "stale-witness",
            "aliases": "", "summary": "Pending.", "body_md": "", "attributes_json": "{}"
        }),
    );
    let stale_key = stale_preview["review"]["reviewKey"]
        .as_str()
        .expect("stale review key")
        .to_owned();
    let stale_uri = stale_preview["draft"]["targetUri"]
        .as_str()
        .expect("stale target URI")
        .to_owned();
    commit_form(
        &webview,
        "entity",
        None,
        json!({
            "kind": "person", "name": "Fresh Witness", "slug": "fresh-witness",
            "aliases": "", "summary": "Committed first.", "body_md": "", "attributes_json": "{}"
        }),
    );
    let stale = invoke_ok(
        &webview,
        "read_manual_review",
        json!({ "input": { "reviewKey": stale_key } }),
    );
    assert_eq!(stale["freshness"]["status"], "stale");
    let stale_error = invoke_ipc(
        &webview,
        "confirm_manual_review",
        json!({ "input": { "reviewKey": stale_key } }),
    )
    .expect_err("stale review must not confirm");
    assert_eq!(stale_error["code"], "manual_review_stale");
    let refreshed = invoke_ok(
        &webview,
        "revalidate_manual_review",
        json!({ "input": { "reviewKey": stale_key } }),
    );
    assert_eq!(refreshed["freshness"]["status"], "current");
    assert_eq!(refreshed["readyToConfirm"], true);
    let stale_commit = invoke_ok(
        &webview,
        "confirm_manual_review",
        json!({ "input": { "reviewKey": stale_key } }),
    );

    let history = invoke_ok(&webview, "list_revision_history", json!({}));
    assert_eq!(
        history["currentHeadRevisionId"],
        stale_commit["current_revision"]
    );
    assert!(history["revisions"].as_array().is_some_and(|revisions| {
        revisions.iter().any(|revision| {
            revision["operations"].as_array().is_some_and(|operations| {
                operations.iter().any(|operation| {
                    !operation["before"].is_null() && !operation["after"].is_null()
                })
            })
        })
    }));
    let undo_target = history["undoTargetRevisionId"]
        .as_str()
        .expect("visible undo target");
    assert_eq!(undo_target, stale_commit["current_revision"]);
    let undone = invoke_ok(
        &webview,
        "undo_revision",
        json!({ "input": { "revisionId": undo_target } }),
    );
    assert_ne!(undone["current_revision"], stale_commit["current_revision"]);
    let missing = invoke_ipc(&webview, "open_uri", json!({ "uri": stale_uri }))
        .expect_err("undo must remove the last created entity");
    assert_eq!(missing["code"], "object_not_found");

    invoke_ok(&webview, "close_world", json!({}));
    let final_reopen = invoke_ok(&webview, "open_world", json!({ "path": path }));
    assert_eq!(final_reopen["current_revision"], undone["current_revision"]);
    invoke_ok(&webview, "close_world", json!({}));

    let bytes = fs::read(&path).expect("read accepted project");
    assert!(!String::from_utf8_lossy(&bytes).contains("FOUNDATION_ACCEPTANCE_SECRET"));
    fs::remove_file(path).expect("remove acceptance project");
}
