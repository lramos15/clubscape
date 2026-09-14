use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use clubscape_content::{
    ARTIFACT_VERSION, MAX_INPUT_BYTES, ValidationMode, compile_content, encode_compiled,
    read_content_json, sha256,
};
use clubscape_game_types::{GameError, GameErrorCode, GameResult};
use serde_json::json;

#[derive(Parser)]
#[command(
    version,
    about = "Strictly validate GameContent JSON and compile a deterministic content artifact"
)]
struct Arguments {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    /// Explicitly allow synthetic TestFixture provenance; never use for runtime content.
    #[arg(long)]
    test_fixture: bool,
}

fn main() -> ExitCode {
    match run(Arguments::parse()) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "{}",
                json!({"ok": false, "code": error.code, "message": error.message})
            );
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Arguments) -> GameResult<serde_json::Value> {
    let input_path = &arguments.input;
    let input = File::open(input_path).map_err(|error| file_error(input_path, error))?;
    let size = input
        .metadata()
        .map_err(|error| file_error(input_path, error))?
        .len();
    if size > MAX_INPUT_BYTES as u64 {
        return Err(file_error(
            input_path,
            format!("input exceeds {MAX_INPUT_BYTES} bytes"),
        ));
    }
    let mut bytes = Vec::new();
    input
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| file_error(input_path, error))?;
    let definition = read_content_json(&bytes)?;
    let mode = if arguments.test_fixture {
        ValidationMode::TestFixture
    } else {
        ValidationMode::Runtime
    };
    let compiled = compile_content(definition, mode)?;
    let artifact = encode_compiled(&compiled)?;
    if fs::canonicalize(input_path).ok() == fs::canonicalize(&arguments.output).ok()
        && arguments.output.exists()
    {
        return Err(file_error(
            &arguments.output,
            "output must not overwrite the source input",
        ));
    }
    write_artifact(&arguments.output, &artifact)?;
    Ok(json!({
        "ok": true,
        "output": arguments.output,
        "file_bytes": artifact.len(),
        "sha256": sha256(&artifact),
        "artifact_version": ARTIFACT_VERSION,
        "codec": "messagepack_named",
        "schema_version": compiled.definition().schema_version,
        "revision": compiled.definition().revision,
        "baseline": compiled.definition().baseline,
        "counts": compiled.counts(),
        "validation": compiled.report(),
    }))
}

fn file_error(path: &Path, error: impl std::fmt::Display) -> GameError {
    GameError::new(
        GameErrorCode::InvalidInput,
        format!("{}: {error}", path.display()),
    )
}

fn write_artifact(path: &Path, bytes: &[u8]) -> GameResult<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| file_error(parent, error))?;
    let name = path
        .file_name()
        .ok_or_else(|| file_error(path, "output must name a file"))?;
    let mut partial_name = name.to_os_string();
    partial_name.push(format!(".clubscape-{}.part", std::process::id()));
    let partial = parent.join(partial_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|error| file_error(&partial, error))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&partial, path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(file_error(path, error));
    }
    Ok(())
}
