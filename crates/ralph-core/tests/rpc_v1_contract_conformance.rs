use std::fs;
use std::path::{Path, PathBuf};

use jsonschema::{Draft, JSONSchema};
use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should be resolvable")
}

fn load_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed reading {}: {err}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|err| panic!("failed parsing json {}: {err}", path.display()))
}

fn compile_validator(schema: &Value) -> JSONSchema {
    JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema)
        .unwrap_or_else(|err| panic!("failed compiling schema: {err}"))
}

fn compile_def_validator(root_schema: &Value, def_name: &str) -> JSONSchema {
    let defs = root_schema
        .get("$defs")
        .cloned()
        .expect("schema must expose $defs");
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": defs,
        "$ref": format!("#/$defs/{def_name}")
    });
    compile_validator(&schema)
}

fn collect_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed listing {}: {err}", dir.display()))
        .map(|entry| entry.expect("dir entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn assert_all_valid(validator: &JSONSchema, dir: &Path) {
    for path in collect_json_files(dir) {
        let instance = load_json(&path);
        if let Err(errors) = validator.validate(&instance) {
            let reasons = errors
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            panic!(
                "expected fixture to be valid: {}\nvalidation errors: {}",
                path.display(),
                reasons
            );
        }
    }
}

fn assert_all_invalid(validator: &JSONSchema, dir: &Path) {
    for path in collect_json_files(dir) {
        let instance = load_json(&path);
        assert!(
            !validator.is_valid(&instance),
            "expected fixture to be invalid: {}",
            path.display()
        );
    }
}

#[test]
fn rpc_v1_fixture_conformance() {
    let root = repo_root();

    let rpc_schema = load_json(&root.join("crates/ralph-api/data/rpc-v1-schema.json"));
    let event_schema = load_json(&root.join("crates/ralph-api/data/rpc-v1-events.json"));

    let request_validator = compile_def_validator(&rpc_schema, "requestEnvelope");
    let response_validator = compile_def_validator(&rpc_schema, "responseEnvelope");
    let error_validator = compile_def_validator(&rpc_schema, "errorEnvelope");
    let event_validator = compile_def_validator(&event_schema, "eventEnvelope");

    let fixtures_root = root.join("crates/ralph-core/tests/fixtures/rpc-v1");

    assert_all_valid(&request_validator, &fixtures_root.join("requests/valid"));
    assert_all_invalid(&request_validator, &fixtures_root.join("requests/invalid"));

    assert_all_valid(&response_validator, &fixtures_root.join("responses/valid"));
    assert_all_invalid(
        &response_validator,
        &fixtures_root.join("responses/invalid"),
    );

    assert_all_valid(&error_validator, &fixtures_root.join("errors/valid"));
    assert_all_invalid(&error_validator, &fixtures_root.join("errors/invalid"));

    assert_all_valid(&event_validator, &fixtures_root.join("events/valid"));
    assert_all_invalid(&event_validator, &fixtures_root.join("events/invalid"));
}
