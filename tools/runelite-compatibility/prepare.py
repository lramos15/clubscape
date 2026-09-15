#!/usr/bin/env python3
"""Pin/reuse official runtime inputs and generate the real protocol bindings."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/runelite-compatibility"
LOCAL = ROOT / "runelite/compatibility/artifacts"
DEFAULT_SOURCE = ROOT.parent / "m1-runtime-inputs/.local/current-source"
CAPTURE_LIBS = ROOT.parent / "m1-source-captures/.local/source-capture/tooling"


def digest(path: Path, algorithm="sha256"):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, algorithm).hexdigest()


def verify(path, record):
    expected_size = record.get("size_bytes", record.get("size"))
    expected_hash = record.get("sha256", record.get("hash"))
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"Missing/unsafe pinned dependency: {path}")
    if expected_size is not None and path.stat().st_size != expected_size:
        raise ValueError(f"Pinned dependency size mismatch: {path.name}")
    if expected_hash and digest(path) != expected_hash:
        raise ValueError(f"Pinned dependency SHA-256 mismatch: {path.name}")
    return path


def fetch(url, path, maximum):
    request = urllib.request.Request(url, headers={"User-Agent": "ClubScape-M1-feasibility"})
    with urllib.request.urlopen(request, timeout=90) as response, path.open("xb") as output:
        received = 0
        while chunk := response.read(1024 * 1024):
            received += len(chunk)
            if received > maximum:
                raise ValueError("Pinned dependency exceeds download bound")
            output.write(chunk)


def obtain(record, candidates, destination):
    if destination.exists():
        return verify(destination, record)
    for candidate in candidates:
        if candidate.is_file():
            verify(candidate, record)
            shutil.copyfile(candidate, destination)
            return verify(destination, record)
    url = record.get("url", record.get("path"))
    if not url or not url.startswith("https://"):
        raise ValueError(f"Missing dependency, no pinned HTTPS URL: {record['name']}")
    partial = destination.with_suffix(destination.suffix + ".download")
    try:
        fetch(url, partial, record.get("size_bytes", record.get("size")))
        verify(partial, record)
        partial.replace(destination)
    finally:
        partial.unlink(missing_ok=True)
    return destination


def linux_arm64(record):
    targets = record.get("platform")
    return targets is None or any(
        target.get("name") == "linux" and target.get("arch") == "aarch64"
        for target in targets
    )


def prepare(source, java_home):
    if platform.machine() != "aarch64":
        raise ValueError("This pinned protoc/native tuple is Linux ARM64, not a host-independent install.")
    lock = json.loads((TOOL / "dependencies.json").read_text())
    LOCAL.mkdir(parents=True, exist_ok=True)
    libraries = LOCAL / "libraries"
    libraries.mkdir(exist_ok=True)
    home = LOCAL / "build-home"
    home.mkdir(exist_ok=True)
    bootstrap_record = lock["runtime_bootstrap"]
    bootstrap_path = ROOT / bootstrap_record["path"]
    if not bootstrap_path.exists():
        verify(source / "bootstrap.json", bootstrap_record)
        shutil.copyfile(source / "bootstrap.json", bootstrap_path)
    verify(bootstrap_path, bootstrap_record)
    bootstrap = json.loads(bootstrap_path.read_text())
    if bootstrap["version"] != "1.12.38":
        raise ValueError("Do not refresh the selected runtime in this bounded experiment.")
    records = [r for r in bootstrap["artifacts"] if linux_arm64(r)]
    decoder = json.loads((ROOT / lock["decoder_lock"]).read_text())
    records.extend([decoder["decoder"], next(
        r for r in decoder["libraries"] if r["name"] == "commons-compress-1.10.jar"
    )])
    paths = []
    for record in records:
        name = record["name"]
        paths.append(obtain(record, [
            source / name, source / "tooling" / name, CAPTURE_LIBS / name,
        ], libraries / name))
    proto = lock["protobuf_generation"]
    compiler = LOCAL / proto["name"]
    if not compiler.exists():
        fetch(proto["url"], compiler, proto["maximum_bytes"])
    if digest(compiler, "sha1") != proto["upstream_sha1"]:
        raise ValueError("Pinned protoc checksum mismatch")
    verify(compiler, proto)
    compiler.chmod(0o700)
    generated = LOCAL / "generated"
    generated.mkdir(exist_ok=True)
    schema = ROOT / "crates/protocol/proto"
    command = [
        str(compiler), f"--proto_path={schema}", f"--java_out=lite:{generated}",
        str(schema / "account.proto"), str(schema / "game.proto"),
    ]
    subprocess.run(command, check=True, cwd=ROOT, timeout=60)
    java = java_home / "bin/java"
    version = subprocess.run(
        [str(java), "-version"], text=True, capture_output=True, check=True
    ).stderr.strip()
    report = {
        "schema_version": 1,
        "runtime_version": bootstrap["version"],
        "jdk": {"java": str(java.resolve()), "version": version, "sha256": digest(java)},
        "libraries": [{
            "name": p.name, "size_bytes": p.stat().st_size, "sha256": digest(p)
        } for p in paths],
        "protoc": {
            "name": compiler.name, "sha256": digest(compiler),
            "upstream_sha1": proto["upstream_sha1"],
        },
        "schemas": [{
            "path": str(p.relative_to(ROOT)), "sha256": digest(p)
        } for p in [schema / "account.proto", schema / "game.proto"]],
        "generated_binding_sha256": {
            str(p.relative_to(generated)): digest(p) for p in sorted(generated.rglob("*.java"))
        },
        "generation_command": command,
        "compatibility_verified": False,
    }
    (ROOT / "research/runelite-feasibility/build-inputs.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
    (LOCAL / "classpath.txt").write_text(os.pathsep.join(str(p) for p in paths))
    print(json.dumps({
        "libraries": len(paths), "runtime": bootstrap["version"],
        "protoc_sha256": digest(compiler), "java": str(java.resolve()),
        "generated_classes": len(report["generated_binding_sha256"]),
    }))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--java-home", type=Path, required=True)
    args = parser.parse_args()
    prepare(args.source.resolve(), args.java_home.resolve())
