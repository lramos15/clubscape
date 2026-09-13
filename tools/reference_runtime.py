#!/usr/bin/env python3
"""Acquire a pinned original reference client; inspect bytecode without running it."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import reference_inputs

ROOT = Path(__file__).resolve().parents[1]
NAMESPACE = ".local/reference-runtime/"


def validate_manifest(manifest):
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise ValueError("Unsupported reference runtime manifest.")
    version = manifest.get("version")
    if not isinstance(version, str) or not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("A fixed reference runtime version is required.")
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != 2:
        raise ValueError("Exactly the pinned injected client and API artifacts are required.")
    identifiers = set()
    for item in artifacts:
        if not isinstance(item, dict) or item.get("id") not in {"injected-client", "runelite-api"}:
            raise ValueError("Unknown reference runtime artifact.")
        identifier = item["id"]
        if identifier in identifiers:
            raise ValueError("Duplicate reference runtime artifact.")
        identifiers.add(identifier)
        suffix = "-runtime" if identifier == "runelite-api" else ""
        filename = f"{identifier}-{version}{suffix}.jar"
        expected_url = f"https://repo.runelite.net/net/runelite/{identifier}/{version}/{filename}"
        if item.get("url") != expected_url or item.get("path") != NAMESPACE + filename:
            raise ValueError("Runtime artifact URL/path does not match its pinned identity.")
        if type(item.get("size_bytes")) is not int or not 0 < item["size_bytes"] <= reference_inputs.MAX_INPUT_BYTES:
            raise ValueError("Reference runtime artifact exceeds its bounded retrieval budget.")
        if not isinstance(item.get("sha256"), str) or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"]):
            raise ValueError("Reference runtime artifact requires a published SHA-256.")
    return artifacts


def build_id_section(output):
    lines = output.splitlines()
    for index, line in enumerate(lines):
        if re.search(r"\bgetBuildID\(\);$", line):
            end = index + 1
            while end < len(lines) and lines[end].strip():
                end += 1
            return lines[index:end]
    raise ValueError("Original client does not expose the expected getBuildID method; no compatibility inferred.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["fetch", "inspect"])
    args = parser.parse_args()
    try:
        manifest_path = ROOT / "research/reference-runtime.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        artifacts = validate_manifest(manifest)
        downloaded = 0
        paths = []
        for item in artifacts:
            path = reference_inputs.input_path(ROOT, item, NAMESPACE)
            if args.operation == "fetch":
                downloaded += int(reference_inputs.fetch_one(ROOT, item, namespace=NAMESPACE))
            else:
                if not path.is_file() or path.stat().st_size != item["size_bytes"]:
                    raise ValueError("Pinned runtime inputs missing or changed; run just reference-runtime-fetch.")
                reference_inputs.validate_bytes(item, path.read_bytes())
            paths.append(path)
        report = {
            "reference_runtime_version": manifest["version"],
            "source_manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
            "verified_artifacts": len(paths),
            "downloaded_artifacts": downloaded,
            "client_executed": False,
            "reference_pack_approved": False,
            "clubscape_compatibility_verified": False,
        }
        if args.operation == "inspect":
            java = shutil.which("java")
            if java is None:
                raise ValueError("JDK required for offline bytecode inspection; see the machine setup record.")
            javap = Path(java).resolve().with_name("javap")
            if not javap.is_file():
                raise ValueError("The selected Java installation has no javap; a full JDK is required.")
            version = subprocess.run([str(javap), "-version"], capture_output=True, text=True, check=True, timeout=30)
            output = subprocess.run(
                [str(javap), "-public", "-c", "-classpath", os.pathsep.join(map(str, paths)), "client"],
                capture_output=True, text=True, check=True, timeout=60,
            )
            report["javap_version"] = version.stdout.strip()
            report["get_build_id_bytecode"] = build_id_section(output.stdout)
        print(json.dumps(report, indent=2))
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Reference runtime error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
