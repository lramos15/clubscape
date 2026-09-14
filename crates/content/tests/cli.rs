mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use common::*;
use serde_json::Value;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TestFiles(PathBuf);

impl TestFiles {
    fn new() -> Self {
        let path = PathBuf::from("test-output").join(format!(
            "cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn run(&self, fixture_mode: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_clubscape-content"));
        command
            .arg("--input")
            .arg(self.0.join("input.json"))
            .arg("--output")
            .arg(self.0.join("compiled.bin"));
        if fixture_mode {
            command.arg("--test-fixture");
        }
        command.output().unwrap()
    }
}

impl Drop for TestFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_is_usable_reports_machine_readable_counts_hash_and_persists_loadable_artifacts() {
    let files = TestFiles::new();
    fs::write(
        files.0.join("input.json"),
        serde_json::to_vec(&fixture()).unwrap(),
    )
    .unwrap();
    let output = files.run(true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["ok"], true);
    assert_eq!(summary["counts"]["collision_cells"], 49);
    assert_eq!(summary["counts"]["spawns"], 5);
    assert_eq!(summary["counts"]["interfaces"], 2);
    assert_eq!(
        summary["validation"]["interface_definition_validation_performed"],
        true
    );
    assert_eq!(
        summary["validation"]["recipe_tool_reference_validation_performed"],
        true
    );
    assert_eq!(summary["validation"]["mode"], "test_fixture");
    assert_eq!(
        summary["validation"]["source_verification_performed"],
        false
    );
    let bytes = fs::read(files.0.join("compiled.bin")).unwrap();
    assert_eq!(summary["sha256"], sha256(&bytes));
    assert_eq!(summary["file_bytes"], bytes.len());
    load_compiled(&bytes, ValidationMode::TestFixture).unwrap();
    let output = files.run(true);
    assert!(output.status.success());
    assert_eq!(bytes, fs::read(files.0.join("compiled.bin")).unwrap());
    assert_eq!(
        fs::read_dir(&files.0).unwrap().count(),
        2,
        "no partial files left behind"
    );
}

#[test]
fn cli_defaults_to_runtime_and_missing_invalid_or_fixture_inputs_exit_nonzero() {
    let files = TestFiles::new();
    let missing = files.run(false);
    assert!(!missing.status.success());
    let error: Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert_eq!(error["ok"], false);
    assert!(error["message"].as_str().unwrap().contains("input.json"));
    assert!(!files.0.join("compiled.bin").exists());
    fs::write(
        files.0.join("input.json"),
        serde_json::to_vec(&fixture()).unwrap(),
    )
    .unwrap();
    assert!(!files.run(false).status.success());
    assert!(!files.0.join("compiled.bin").exists());
    fs::write(files.0.join("input.json"), b"{}").unwrap();
    let invalid = files.run(true);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("missing field"));
    fs::write(
        files.0.join("input.json"),
        serde_json::to_vec(&runtime_policy_fixture()).unwrap(),
    )
    .unwrap();
    let runtime = files.run(false);
    assert!(
        runtime.status.success(),
        "{}",
        String::from_utf8_lossy(&runtime.stderr)
    );
    let summary: Value = serde_json::from_slice(&runtime.stdout).unwrap();
    assert_eq!(summary["validation"]["mode"], "runtime");
    assert!(
        summary["validation"]["evidence"]["inference"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn invalid_compile_preserves_previous_output_and_input_cannot_be_overwritten() {
    let files = TestFiles::new();
    let input = serde_json::to_vec(&fixture()).unwrap();
    fs::write(files.0.join("input.json"), &input).unwrap();
    assert!(files.run(true).status.success());
    let previous = fs::read(files.0.join("compiled.bin")).unwrap();
    fs::write(files.0.join("input.json"), b"{\"invalid\":true}").unwrap();
    assert!(!files.run(true).status.success());
    assert_eq!(previous, fs::read(files.0.join("compiled.bin")).unwrap());
    fs::write(files.0.join("input.json"), &input).unwrap();
    let same_file = Command::new(env!("CARGO_BIN_EXE_clubscape-content"))
        .arg("--input")
        .arg(files.0.join("input.json"))
        .arg("--output")
        .arg(files.0.join("input.json"))
        .arg("--test-fixture")
        .output()
        .unwrap();
    assert!(!same_file.status.success());
    assert_eq!(input, fs::read(files.0.join("input.json")).unwrap());
}
