# M1 benchmark contract status

Status: **target-hardware contract blocked; no measurements passed**.

The mandatory target remains 60 rendered FPS at 1920x1080 in desktop Chrome
and Edge with WebGPU on representative modern integrated graphics. Stock
visuals, the approved draw distance and real tutorial/Lumbridge workloads are
required. Empty scenes, lower resolution, a configured frame limiter,
dedicated-GPU runs and software rendering cannot satisfy this target.

Before presentation/performance implementation, record the target GPU, CPU,
memory, OS/driver, browser builds, display/viewport/UI settings, representative
routes, measurement windows and numeric acceptance tolerances. Those fields
are not supplied at kickoff and must not be filled with invented measurements.
The visual/audio pack must also be approved before that implementation.

Sparky was rechecked on 2026-09-13: Ubuntu 24.04.4 ARM64 and NVIDIA GB10 with
driver 580.173.02. It has not been established as the representative Section 36
target, and no complete hardware/browser contract or rendered-workload evidence
exists. Do not infer a desktop GPU classification or benchmark equivalence from
the NVIDIA model name. Pin a qualifying representative environment and measure
both required browsers before treating any host as acceptance evidence.

The independent account-service increment uses bounded correctness workloads,
not release-performance certification: one client, five concurrent synthetic
clients, and a maximum of four concurrent password jobs; local test
PostgreSQL is limited to two CPUs and 512 MiB. These limits protect the shared
host and do not set or lower the release concurrency target.

Later gameplay benchmarks must pin one-to-five-player and proposed launch
concurrency workloads and record server tick time, memory/bandwidth per player,
CPU/entity cost, WASM startup, frame distributions/stalls, streaming/download
size, content builds and integration/CI throughput under Section 36.
The launch concurrency target remains a draft release-contract input.

## Owner-run final browser target

During the fleet continuation the owner selected their M-series Mac for
testing after implementation, and authorized continuing available Sparky
engineering checks. Apple Silicon supplies integrated graphics. The final
target's exact chip, memory, macOS and real Chrome/Edge versions must be
recorded by the owner-run harness; no unexecuted measurement is certified here.
Local Sparky results remain separately identified, including any sandbox
diagnostic limitations. No sandbox-disabling flag or security-policy bypass
was authorized. See `milestones/m1-owner-followups.json`.
