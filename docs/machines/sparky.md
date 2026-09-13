# Sparky Development Machine

Verified on 2026-09-13. This is the repository's machine setup record for Sparky;
other hosts must be checked independently. Paths beginning with `~` refer to the
developer's home directory, not the repository.

## Host

- Ubuntu 24.04.4 LTS, Linux ARM64 (`aarch64`).
- NVIDIA GB10, driver 580.173.02; Vulkan enumerates the NVIDIA device.
- 20 CPU cores, 121 GiB RAM, approximately 3.5 TiB free at the initial audit.
- Development is through remote VS Code Insiders; no desktop `DISPLAY` is set.
- Port 8080 was occupied during setup. Check ports before starting services.

## Core Tools

- Git 2.43.0; GitHub CLI 2.45.0 with working GitHub authentication.
- GitHub Copilot CLI 1.0.81 was present in VS Code's managed installation.
- Node.js 24.18.0, npm 11.16.0, Corepack 0.35.0, pnpm 12.4.1.
- Rust/Cargo 1.98.1 through rustup, with rustfmt and Clippy.
- Rust target `wasm32-unknown-unknown` installed.
- wasm-bindgen CLI 0.2.128; match the future project's crate and CLI versions.
- just 1.58.0; Make is also installed.
- GCC/G++ 13.3, CMake 3.28.3, pkg-config, and OpenSSL development headers.
- Python 3.12.3 and pip 24.0.
- Remote VS Code Rust Analyzer 0.3.3041.

Cargo's environment is sourced by `~/.bashrc` and `~/.profile`. For terminals
opened before installation:

```bash
source "$HOME/.cargo/env"
```

TypeScript 7.0.2 was tested in a disposable package, not installed globally.
Application dependencies and their versions still belong in future project
manifests and lockfiles.

## Containers

Docker Engine 29.2.1, Compose 5.0.2, and Buildx 0.31.1 are available to the user
without `sudo`. The cached `postgres:16-alpine` image is native ARM64 and contains
PostgreSQL 16.15. An isolated SQL transaction smoke test passed.

No project database or other game service is running. Use project-specific
Compose configuration when implementation begins. Redis remains conditional on
an actual caching or ephemeral-state requirement in the project specification.

## Blender

Installed: community Blender 5.1.0 for Ubuntu 24.04 ARM64 and NVIDIA GB10.

- [Release and limitations][blender-release].
- Archive: `blender-v10.tar.xz`.
- SHA-256, checked before extraction:

```text
f8c41e767bfb8e42c0e3190ec5132c3bae413c568a17664388341c67d543ab1a
```

Installation: `~/.local/share/clubscape-blender-5.1.0/cmake-make`.
The `~/.local/bin/blender` wrapper supplies the package's `libExt` search path
and forwards arguments. No system NVIDIA driver or CUDA installation was
replaced. Do not run the community source builder on the host without reviewing
its system changes.

Ubuntu runtime packages installed for this build:

```bash
sudo apt install -y --no-install-recommends \
  vulkan-tools xvfb libavdevice60 libblosc1 libfftw3-double3 \
  libgoogle-glog0v6t64 libopenexr-3-1-30 \
  libosdcpu3.5.0t64 libosdgpu3.5.0t64 libpotrace0 \
  libpugixml1v5 libpystring0 libspnav0 libyaml-cpp0.8 libmetis5
```

CPU, CUDA, and OptiX rendering passed nonblank-image checks. GPU tests selected
the GB10 with CPU fallback disabled in the Cycles device preferences. glTF
export and reimport passed. Use `--python-exit-code 1` in Blender script tests
so Python exceptions produce a failing process exit.

## Browser Graphics

The cached ARM64 browser used for verification is Chrome for Testing
153.0.8010.12:

```text
~/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome
```

### Sandbox

The user installed and loaded a browser-specific AppArmor profile at
`/etc/apparmor.d/clubscape-chromium`; its source is at
`~/.config/clubscape/chromium.apparmor`.

```text
abi <abi/4.0>,
include <tunables/global>

profile clubscape-chromium
/home/lramos15/.cache/ms-playwright/chromium-*/chrome-linux{,-arm64}/chrome
flags=(unconfined) {
  userns,
}
```

The absolute attachment path is host-specific. This follows Ubuntu's browser
profile pattern to allow user namespaces for that executable. Do not disable
AppArmor globally or add `--no-sandbox`; Chromium's namespace and seccomp
sandbox checks passed with this profile.

### Visual Tests

Use **windowed Chromium under Xvfb**, with `headless: false` and these arguments:

```text
--enable-unsafe-webgpu
--enable-features=Vulkan
--use-angle=vulkan
--enable-gpu
--ignore-gpu-blocklist
--ozone-platform=x11
```

Run the browser-test process through `xvfb-run --auto-servernum`, using server
arguments `-screen 0 1280x800x24 -nolisten tcp`. Keep Xvfb alive for the entire
test process, not just its initial browser-launch command.

Do **not** add `--disable-vulkan-surface` to this windowed configuration.
Pure headless Chromium successfully executed NVIDIA WebGPU and read back GPU
pixels but produced blank canvas screenshots, even with repeated frames and a
blocklist override. Windowed Xvfb tests passed actual screenshot-pixel checks.
Adapter detection and GPU readback alone are not visual-rendering proof.

These are local test-browser options, not changes to normal browser settings.
Recheck browser versions and GPU behavior when the driver or browser changes.

## Java and RuneLite

The system currently has a Java 8 runtime but no JDK. RuneLite tooling is being
provisioned and tested; it must not yet be considered verified on this host.

Installing or launching upstream RuneLite is not ClubScape compatibility proof.
There is no ClubScape server, scene bridge, or plugin integration yet. The
compatibility acceptance criteria remain in section 12 of
[the project specification](../../prompt.md).

## Verified Core Smoke Tests

- Native Rust compilation, unit test, rustfmt, and Clippy with warnings denied.
- Rust WASM compilation and wasm-bindgen execution in Node and Chromium.
- Native ARM64 TypeScript compilation through pnpm and just.
- PostgreSQL startup, transaction, and shutdown in an isolated ARM64 container.
- Native Vulkan device discovery and sandboxed NVIDIA WebGPU execution.
- Windowed WebGPU screenshot-pixel validation under Xvfb.
- Blender CPU/CUDA/OptiX renders and glTF export/import.

The original test files, test containers, browsers, and servers were disposable
and were cleaned up. These checks establish machine capabilities, not game
completion. No game workspace was scaffolded during provisioning.

[blender-release]: https://github.com/CoconutMacaroon/blender-arm64/releases/tag/v10-5.1
