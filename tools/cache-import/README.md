# Original OSRS M1 source importer

This is **extraction tooling, not a ClubScape renderer or an approved reference
pack**. It operates on actual original cache bytes. It never accepts terms,
logs in, reads personal RuneLite state, or connects players to a public world.

## Frozen selection

[`selection.json`](../../research/current-source/selection.json) records the
current official RuneLite injected runtime **1.12.38** and the newest complete
live/en public OSRS cache observed on 2026-09-13: **build 240, cache 2695**.
The owner removed the requirement to use 2695; it happens to remain newest in
the retrieved current metadata. The public official gamepack is a small
farewell-family Applet stub, not the selected renderer. No downgrade to 235
was made.

The original injected client's `getRevision()` changed from **0 to 240** when
its real `init()` consumed the recorded official configuration. Its opaque
build ID `33653951311.245` is **not** interpreted as revision 245.
This verifies configuration initialization, **not** full original-renderer cache
loading or in-world compatibility. The separate original-data decoding results
are recorded in `research/current-source/validation.json`.

The complete OpenRS2 disk ZIP is 190,266,299 bytes, SHA-256
`8f6bd170d2f97e310aaa7700626f7140647a97ce926195ef7625d738f4aa4157`.
Its 25 original files total 229,196,602 bytes. Whole downloads stay under
ignored `.local/`; every original disk file has its own size/hash record.
RuneLite creates two additional empty index files for zero-group indexes 16/23;
only those exact empty additions are allowed.

## Reproduce

Run from the repository/worktree root. Python 3.12 and JDK 17 are used; no Python
packages, global Gradle install, GPU, browser, Docker service or account are
needed. Read the project's machine setup before using host-specific paths.

```bash
python3 tools/cache-import/import_cache.py fetch
python3 tools/cache-import/import_cache.py prepare
python3 tools/cache-import/import_cache.py probe
python3 tools/cache-import/import_cache.py extract
python3 -m unittest discover -s tools/cache-import -p 'test_*.py' -q
python3 tools/cache-import/import_cache.py test-integrity
python3 tools/cache-import/import_cache.py publish
python3 tools/cache-import/import_cache.py validate-published
```

`fetch` reuses verified downloads and otherwise streams the selected URLs,
rejecting oversized, truncated or SHA-mismatched responses. It does **not**
refetch "latest" or silently advance the selection. ZIP member paths, duplicate
entries, advertised decompressed sizes and all individual hashes are checked.
Existing corrupted inputs fail nonzero rather than being silently replaced.
No command deletes personal settings or modifies upstream tooling.

`prepare` checks `dependencies.json`, reuses the installed pinned RuneLite
decoder JAR and exact Gradle-cache dependencies read-only, then copies only
verified dependencies into this worktree's `.local/current-source/tooling/`.
Missing Maven dependencies are fetched from their locked URLs and checked.
On Sparky the documented upstream checkout is discovered under
`~/.cache/clubscape/upstream/runelite`. Other hosts can pass
`--runelite-source PATH` and `--java-home PATH`.

If the pinned decoder is absent, the command fails with a specific prerequisite.
Build the **exact** source in the project's ignored directory, not in a global
checkout:

```bash
mkdir -p .local/cache-tooling .local/java-work
git clone --quiet https://github.com/runelite/runelite .local/cache-tooling/runelite
git -C .local/cache-tooling/runelite checkout --quiet --detach ac79ed8bd8926bec7bf172aa291574b4d944b0e7
JAVA_TOOL_OPTIONS="-Djava.io.tmpdir=$PWD/.local/java-work" \
GRADLE_USER_HOME="$PWD/.local/cache-tooling/gradle" \
  .local/cache-tooling/runelite/gradlew \
  --project-dir .local/cache-tooling/runelite/cache --no-daemon jar
python3 tools/cache-import/import_cache.py prepare \
  --runelite-source .local/cache-tooling/runelite
```

Use a verified JDK 17 via `JAVA_HOME` for that source build. The existing
checksum-pinned upstream Gradle wrapper supplies Gradle. The importer refuses a
different built JAR checksum; do not bypass that check or silently relabel a
different source build. The exact installed binary was used for this task.

All Java executions use an isolated worktree `user.home`, headless AWT and a
worktree-local scratch path; image disk caching is disabled. They do not inspect
`~/.runelite`. The probe exits after configuration initialization, so it is not
a persistent source client or reference capture session.

## Output and integrity contract

The detailed versioned contract is
[`extraction-contract.json`](../../research/current-source/extraction-contract.json).
The editable source-data request is
[`m1-request.json`](../../research/current-source/m1-request.json).

* Full output: `.local/current-source/extracted/`, with `bundle.json` recording
  every source archive/file, full archive revision, CRC, original payload hash,
  output hash and counts.
* Bounded committed data: `assets/source/osrs/cache2695/`. It includes complete
  selected terrain and placement arrays, aggregated source definitions and
  legacy animation data, original UI RGBA atlases, native font metrics, title
  JPEG, music MIDI and sound-source data, and eight representative original
  tree/goblin/penguin meshes. No replacement geometry or generic icons are used.
* Full output inventory: `assets/manifests/osrs/cache2695-full-bundle.json.gz`.
  This is an index of **actual completed extraction**, not just retrieval URLs.
  Re-running `extract` recreates its referenced full model/audio-sample inputs.
* Published file hashes and limitations:
  `assets/manifests/osrs/cache2695-published.json`.
* Committed runtime, test, repeatability and representative evidence:
  `research/current-source/validation.json`.

`validate-published` checks the committed source inputs and full inventory with
Python alone; it needs neither the large download nor Java. It verifies hashes,
published mesh/terrain bounds and actual PNG data without claiming rendering.

The importer verifies CRC and the low 16-bit disk revision trailer before
stripping it. It preserves the full 32-bit revision from the index. Upstream
`configureForRevision` methods take **archive/index revisions**, not game build
240; using the latter misdecodes current definitions and interfaces.

`validate` rechecks every referenced output, decompressed JSON hash, model
triangle index and attribute cardinality, native terrain/placement bounds,
font metrics, PNG chunk CRCs/actual scanlines, MIDI headers and record counts.
Fourteen original empty source meshes are explicitly marked; they are not
replacement art, and no representative model is empty.
`test-integrity` validates an actual original tree container, then rejects
corrupted bytes and a wrong CRC using **in-memory copies**, and finally rechecks
the unchanged original cache files. Unit tests additionally cover missing files,
nonzero CLI errors, ZIP traversal/incompleteness, invalid geometry, corrupt PNGs
and relocated source placements.

`scan --output .local/current-source/dependency-scan` is an optional source-ID
discovery command. It joins the named Death's Coffer definition to actual cache
locations and reports source square 12633. The whole-cache exploratory lookup
reports any unparseable outside-slice location stream separately; it is not
whole-world compatibility certification. Required `extract` inputs still fail
closed on a decoding error.

## Interpretation limits

* Source bounds include complete Tutorial Island surface/underground,
  Lumbridge/castle/basement/river/bridges, swamp mining, bank/shop, connected
  farms/mill and surrounding scenery. Death's Office is also extracted from its
  actual source square. These are conservative **data bounds**, not approved
  playable fences, view distance, cameras or a live instance-copy mapping.
* Tree object **1277** uses model **1570** and source recolor **3470 → 5029**.
  Goblin **3028** is a level-2 candidate with six original model parts and
  standing/walking sequences **6181/6180**. Penguin NPC **2063** uses model
  **21547**, source scales **75/128**, and sequences **5668/5666**; it is
  **not an approved player adaptation**. Preserve definition transforms and
  recolors instead of rendering unmodified meshes and calling them faithful.
* Ordinary NPC spawn tables, authoritative state, tutorial guards, live object
  morph states and source camera settings are not inferred from cache presence.
  Both Henja/Survival Expert and Cook variants remain explicit candidates.
* Legacy frames/transform groups are decoded. A modern curve animation and its
  skeleton source bytes are preserved; curve/matrix playback remains explicit
  conversion work, not falsely reported as decoded animation playback.
* All music patches and compressed samples/setup are preserved in full output.
  MIDI is **not** a faithful source recording by itself; a generic soundfont
  would change the sound. Source synthesizer and timing/trigger validation remain.
  Current cache name hashes resolve **Scape Main to group 0**, not the older
  wiki seed 16. Other requested tracks resolve to 62, 144, 76 and 2.
* Widget definitions and their directly referenced sprites/fonts are decoded;
  dynamic clientscripts, generated item icons, layout behavior and visibility
  require further closure and source observation.
* The title JPEG is the original **383×503 source background component**, not
  a full title/login screenshot. Sprite sheets, geometry and this background are
  source assets, **not stock source-game reference captures**.

An authorized source-account/terms handoff, calibrated stock Resizable-Classic
captures/audio and **owner reference-pack approval** remain before ClubScape
presentation implementation. No terms were accepted, no login performed, no
source or ClubScape presentation self-approved, and no gameplay/RuneLite
compatibility acceptance is claimed.
