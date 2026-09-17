"""Build mirror with a private lockfile; tracked workspace manifests/lock remain untouched."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[2]
MIRROR = ROOT / ".local/dying-observation-build"


def prepare(root=ROOT, mirror=MIRROR):
    mirror.mkdir(parents=True, exist_ok=True, mode=0o700)
    for name in ("Cargo.toml", "rust-toolchain.toml"):
        shutil.copyfile(root / name, mirror / name)
    lock = mirror / "Cargo.lock"
    if not lock.exists():
        shutil.copyfile(root / "Cargo.lock", lock)
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    members = []
    for pattern in workspace["workspace"]["members"]:
        members.extend(path for path in root.glob(pattern) if (path / "Cargo.toml").is_file())
    if not 0 < len(members) <= 64:
        raise ValueError("Unexpected bounded workspace member count")
    for member in members:
        destination = mirror / member.relative_to(root)
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(member / "Cargo.toml", destination / "Cargo.toml")
        for source in member.iterdir():
            if source.name in {"Cargo.toml", "Cargo.lock", ".local", "target", ".git"}:
                continue
            target = destination / source.name
            if target.is_symlink():
                if target.resolve() != source.resolve():
                    raise ValueError("Build source link changed")
            elif target.exists():
                raise ValueError("Unexpected file in private build mirror")
            else:
                target.symlink_to(source, target_is_directory=source.is_dir())
    for name in ("assets", "content", "research", "tests", "spec", "docs", "web", "prompt.md"):
        source, target = root / name, mirror / name
        if not source.exists():
            continue
        if target.is_symlink():
            if target.resolve() != source.resolve():
                raise ValueError("Build reference link changed")
        elif target.exists():
            raise ValueError("Unexpected reference in private build mirror")
        else:
            target.symlink_to(source, target_is_directory=source.is_dir())
    return mirror / "Cargo.toml"


def command(arguments):
    manifest = prepare()
    return ["cargo", arguments[0], "--manifest-path", str(manifest), *arguments[1:]]


def main():
    if len(sys.argv) < 2:
        raise ValueError("A Cargo subcommand is required")
    original = (ROOT / "Cargo.lock").read_bytes()
    env = os.environ.copy()
    env["TMPDIR"] = str(ROOT / ".local/journey-build-work")
    env["CARGO_TARGET_DIR"] = str(ROOT / ".local/journey-target")
    try:
        return subprocess.run(command(sys.argv[1:]), cwd=ROOT, env=env, check=False).returncode
    finally:
        if (ROOT / "Cargo.lock").read_bytes() != original:
            raise RuntimeError("Tracked root lock changed during private build")


if __name__ == "__main__":
    raise SystemExit(main())
