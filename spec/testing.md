# Validation and evidence

Use the smallest relevant commands and actual components. Initial commands
will cover Rust format/lint/unit checks, protocol WASM compilation, real
PostgreSQL and HTTP account/session integration, and checkpoint gate validation.
Integration runs use bounded synthetic accounts in an isolated disposable
database; they must never point at an existing player database.

Keep these evidence classes separate:

| Evidence | What it establishes | What it cannot establish |
| --- | --- | --- |
| Toolchain smoke checks | Host capability | Any ClubScape feature |
| Protocol and account unit checks | Local invariants | Live persistence or gameplay |
| Account PostgreSQL/HTTP lifecycle | Actual identity/session service | Browser signup, character state or tutorial |
| Headless full journey | Authoritative source-based gameplay | Visual/audio fidelity |
| Chrome and Edge full journey | Real browser/server interactions | Source fidelity without comparisons |
| Source/candidate visual and runtime audio checks | Referenced presentation | Gameplay correctness or target hardware |
| Pinned integrated-GPU measurements | Section 36 rendering performance | Owner presentation acceptance |
| Revision-bound owner review | Explicit visual/audio acceptance | Missing independent evidence |

Every fixed bug needs a regression check. Generated expected outcomes cannot
replace source-grounded formulas, quest stages, acquisition paths or recovery
behavior. Tests must fail on malformed input and unavailable required
dependencies rather than silently skipping or substituting mocks.

`milestones/m1-starter-journey.json` is the durable gate record. Evidence must
identify commands, outcomes, tested revision and relevant artifacts. The
acceptance command must fail closed if a gate, required approval or matching
build is absent; a status command may report incomplete work without passing it.

Content compilation/ID uniqueness, asset manifests, interface controls/resizing,
unsupported-capability feedback and migration/extension behavior have explicit
unpassed M1 gates. The account-only schema and two source-index integrity checks
do not satisfy those full-journey requirements.
