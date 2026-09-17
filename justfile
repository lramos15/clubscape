default:
    @just --list

doctor:
    python3 tools/dev.py doctor

bootstrap:
    python3 tools/dev.py bootstrap

db-start:
    python3 tools/dev.py db-start

db-stop:
    python3 tools/dev.py db-stop

server:
    python3 tools/dev.py server

[positional-arguments]
sim url="http://127.0.0.1:4010":
    cargo run --quiet --locked -p clubscape-sim -- account-lifecycle --url "$1"

fmt:
    cargo fmt --all

lint:
    cargo fmt --all -- --check
    cargo clippy --quiet --workspace --all-targets --locked -- -D warnings
    python3 tools/milestone.py validate

test-fast:
    cargo test --quiet --workspace --locked
    python3 -m unittest discover -s tests/workflow -v

test: test-fast test-integration

test-integration:
    python3 tools/dev.py integration

check-wasm:
    cargo check --quiet -p clubscape-protocol --target wasm32-unknown-unknown --locked

content-build:
    python3 tools/m1-content/build.py

content-check:
    python3 tools/m1-content/validate.py --repeat

asset-check:
    python3 tools/cache-import/import_cache.py validate-published
    python3 tools/audio-import/audio_import.py validate

milestone-status:
    python3 tools/milestone.py status

milestone-validate:
    python3 tools/milestone.py validate

milestone-check:
    python3 tools/milestone.py check

reference-fetch:
    python3 tools/reference_inputs.py fetch

reference-verify:
    python3 tools/reference_inputs.py verify

reference-runtime-fetch:
    python3 tools/reference_runtime.py fetch

reference-runtime-inspect:
    python3 tools/reference_runtime.py inspect
