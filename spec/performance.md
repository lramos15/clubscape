# M1 benchmark contract status

Status: **benchmark contract fixed; final owner-hardware measurements pending**.

The pre-renderer budget and permitted local profile are now fixed in
`spec/m1-benchmark-contract.json`, tied to the owner-approved reference-pack
hash. Exact owner-run Mac browser/hardware collection and actual performance
acceptance remain pending; no achieved FPS is asserted.

The mandatory target remains 60 rendered FPS at 1920x1080 in desktop Chrome
and Edge with WebGPU on representative modern integrated graphics. Stock
visuals, the approved draw distance and real tutorial/Lumbridge workloads are
required. Empty scenes, lower resolution, a configured frame limiter,
dedicated-GPU runs and software rendering cannot satisfy this target.

The pre-implementation contract records the permitted local GPU, CPU, memory,
OS/driver, Chrome build, display/viewport/UI settings, representative routes,
measurement windows and numeric tolerances. These are fixed inputs, not
achieved measurements. The reference pack is owner-approved; final Mac hardware
and both native browser results must still be collected without changing
the acceptance thresholds.

Sparky was rechecked on 2026-09-13: Ubuntu 24.04.4 ARM64 and NVIDIA GB10 with
driver 580.173.02. Vulkan reports integrated/shared-memory graphics, but the owner
selected the M-series Mac for final Chrome/Edge testing. The pinned Sparky
profile supports permitted engineering checks, not unrun Mac/Edge acceptance.
Preserve its recorded GPU-process sandbox limitation and do not infer benchmark
equivalence from a model name. Actual rendered workloads remain unmeasured.

The independent account-service increment uses bounded correctness workloads,
not release-performance certification: one client, five concurrent synthetic
clients, and a maximum of four concurrent password jobs; local test
PostgreSQL is limited to two CPUs and 512 MiB. These limits protect the shared
host and do not set or lower the release concurrency target.

The frozen M1 contract includes one/five-player 600 ms gameplay workloads.
Actual benchmarks must record server tick time, memory/bandwidth per player,
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
