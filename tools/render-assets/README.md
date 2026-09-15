# tools/render-assets

Build-time exporter that runs the pinned original runtime (`injected-client-1.12.38`, revision
240, cache 2695) against the hash-verified original cache and writes **neutral render buffers**
into `assets/compiled/render`. It reuses the read-only bootstrap of `tools/source-capture`
(`capture.prepare`, `OriginalCache`, `OriginalCapture`) and never produces a candidate image or a
source reference image: it serialises the live client structures the original renderer reads.

Also contains `compare.py`, the unmasked `native_scene_model` comparison used to check browser
captures against the approved reference PNGs.

## Requirements

* JDK 17 (`~/.local/share/jdks/temurin-17.0.20.1+1` on Sparky; override with `--java-home`).
* The verified current-source inputs (`.worktrees/m1-runtime-inputs/.local/current-source` or
  `.local/current-source`), i.e. the same inputs `tools/source-capture` uses.
* Python 3.12 (standard library only; `compare.py` additionally needs `numpy`, e.g. in a local
  venv under `.local/`).

Nothing is installed globally; compiled classes, a private `user.home` and scratch directories
live under `.local/render-assets`. The exporter never reads `~/.runelite`.

## Usage

```bash
python3 tools/render-assets/export.py                       # everything (profile "all")
python3 tools/render-assets/export.py --profile scenes      # the five fixture scenes + gzip twins
python3 tools/render-assets/export.py --profile npcs        # NPC base models, baked frames, packs
python3 tools/render-assets/export.py --profile prune-textures
python3 tools/render-assets/export.py --profile blocks      # all 61 M1 map squares (content/m1/geometry)
python3 tools/render-assets/export.py --profile blocks 12850 12851   # selected squares
python3 tools/render-assets/export.py --profile scenes-pinned  # frame-0 validation twins for the block tests
python3 tools/render-assets/export.py --profile anim        # skeletal sequences, NPC definitions, player body, worn items
python3 tools/render-assets/export.py --profile dynamic     # door/fire/state object variants and ground-item stacks
python3 tools/render-assets/export.py --profile widgets     # original if3 model components (interface 679:73)
python3 tools/render-assets/export.py --profile minimap     # map-scene sprites/shape masks, map-element icon sprites, per-square minimap sidecars (+ original icon pass)
python3 tools/render-assets/export.py --profile zoom        # original full-HUD viewport zoom per canvas size (hud/zoom-table.json)
python3 tools/render-assets/export.py --profile pose-fits   # per-item per-pose gear contact fits (gear/pose-fits.json; runs the renderer's pose-fit-table bin, no JDK/cache)
python3 tools/render-assets/export.py --profile compress    # (re)write scenes/*.gz, blocks/*.gz + manifest
python3 tools/render-assets/export.py --profile unpack      # restore raw *.bin from published *.gz (pinned bytes; unpublished block twins optional)
python3 tools/render-assets/export.py --profile pack-blocks # deterministic world-block pack + blocks.index.json
python3 tools/render-assets/export.py --profile unpack-blocks .local/render-assets/dist/clubscape-render-blocks-<hash>.tar
python3 tools/render-assets/export.py --profile verify-blocks  # strict: every pinned block buffer present + hashed
python3 tools/render-assets/export.py --verify-only         # hash-check assets/compiled/render
```

Profiles: `tables`, `palette`, `textures`, `models` (tree 1277 / model 1570 lit as captured),
`npcs` (3028 goblin, 2063 penguin; sequences 6181/6180 and 5668/5666), `scenes`, `blocks`,
`scenes-pinned`, `anim`, `dynamic`, `widgets`, `minimap` (also `minimap/mapicons.bin`: every
map element the original shows on the minimap with its sprite from `ps.as(false)`, and the
original `bu.aa` icon pass per square in the sidecar's `MICN` chunk), `zoom` (runs the original
Resizable-Classic layout at 16 canvas sizes and records `client.fk` plus the cs2 6200/6202 zoom
parameters), `pose-fits` (Rust only: `cargo run --release -p clubscape-renderer --features tools
--bin pose-fit-table`, deterministic, frames over the fit targets counted in the manifest's
`gear_pose_fits`), `prune-textures` (keep only textures referenced by the exported
scenes/blocks/models), `compress`, `unpack`, `pack-blocks`, `unpack-blocks`, `verify-blocks`.

### World block package (no source cache or JDK needed to consume)

The 61 M1 map squares are reproducible original-loader exports (`blocks`, ≈ 56 MB as gzip twins)
and are not committed. `pack-blocks` writes them plus the 61 minimap sidecars into one
deterministic tar (sorted PAX entries, mtime 0, uid/gid 0, mode 0644 — the same bytes on every
run) under `.local/render-assets/dist/clubscape-render-blocks-<content-hash>.tar`, and records
in the committed `assets/compiled/render/blocks.index.json` the pack's SHA-256/size, the manifest
hash it belongs to, and every member's key, SHA-256 and size (gzip and inflated). The shell can
publish/host that tar or its extracted members under its render asset base URL: the adapter
fetches `blocks/<square>.bin.gz` / `blocks/<square>.models.bin.gz` / `minimap/blocks/<square>.bin`
by manifest key and verifies both the gzip and inflated hashes. `unpack-blocks <tar>` installs
the members into the asset tree, checking the pack hash, every member hash, path containment
and the inflated raw buffers (which the native tests read), with plain Python 3 — no source
cache, JDK or original runtime. `verify-blocks` is the strict presence + hash check
(`--verify-only` treats blocks as optional).

Current pack: `clubscape-render-blocks-d76bc2ab72b552a1.tar`, SHA-256
`99984d72eb3e00e9614ba712f6ecb5ebeab1ba6c7c29116826dc735369d30aeb`, 56 381 440 bytes,
183 members (122 block twins + 61 sidecars), content hash
`d76bc2ab72b552a1a097bea19296beb0d8f79b34ca0d1e424cd84813a57f787e`.

`anim` (`AnimExport.java`) writes the original skeletons and frames (`et`/`em`) of the required
player sequences (808/819/824/836, 879, 625, 621, 733, 897/896/899/898, 386/390/422/423, 426,
711, 829/12526/827) plus every stand/walk/rotate/run/combat sequence of the 27 M1 NPC
definitions, each definition's lit base model (`pl.ag` lighting, no animation), the 12 equippable
M1 items' worn models (`op.jm`, lit as `lc.bd` does) and the default male body as the human
label-retarget reference. `dynamic` (`DynamicExport.java`) lights door objects at wall types 0/9 ×
4 rotations, the fire (26185, 5 frames of sequence 475) and flour-bin states through the original
`om.sg`, and the 116 ground items per quantity threshold through `op.aa`. `widgets`
(`WidgetExport.java`) decodes interface 679's model component with the original `lw.ag` if3
decoder and records its zoom/offset/rotation/content-type fields. Scenes and blocks also carry
the tile settings (`TSET`/`BSET`, `ez.vs`) used by roof removal.

`blocks` loads every requested 64×64 map square through the original loader with ≥ 16 tiles of
real neighbour margin (bases on the 8-tile chunk lattice), serialises the square's own tile
arrays and placements in square-relative units, and bakes each animated scenery renderable as
one lit model per source frame (`dy.vn` with the state pinned) plus the plain model and the
sequence timing (`ou.bk/bu/bf/bo`). `scenes-pinned` re-exports the five fixture scenes with the
same frame-0 pinning so `crates/renderer/tests/blocks.rs` can prove block assembly equals the
direct 104×104 export.

`minimap` (`MinimapExport.java`) writes what the renderer's port of the original minimap
(`client.bm`) needs beyond the blocks: `minimap/mapscenes.bin` — the map-scene indexed sprites
(`oy.aq`, loaded with `hk.ao`/`fs.ac` from the sprite group the graphics defaults name) and the 16
tile-shape coverage masks `client.gq()` rasterises from the tile-shape models — and, per square,
`minimap/blocks/<square>.bin` with every wall's placement config (`fe.getConfig()`, type |
rotation << 6; the block records only keep the orientation, which types 1 and 3 share) and the
`mapSceneId`/size/`mapIconId` of every object definition referenced by the square's walls, game
objects and floor decorations. The squares are loaded exactly as the block export loads them.
The sidecars are small (~640 KB for the 61 M1 squares) and published with the manifest.

The scene profile replays the capture's model profile first so the seeded random start frames
of animated scenery match the approved captures, then loads the scenes in capture order
(castle plaza, river bridge, starting house, survival coast, windmill route) through the original
`rl4.fn` map loader and serialises the live `ez` arrays (tiles, paints, shaped tiles, walls,
decorations, game objects, zone lists) together with a deduplicated model pack.

## Outputs

`assets/compiled/render/manifest.json` records identity (runtime, revision, cache id, brightness
0.8, texture size 128, approved reference pack SHA), the texture/NPC/scene tables and a `files`
map with SHA-256 + size for every buffer. Formats are documented in
`crates/renderer/README.md`.

Published in git: `manifest.json`, `palette.bin`, `textures/*.bin` (43 used textures), base
models, NPC packs, `scenes/*.bin.gz` (deterministic gzip: mtime 0, level 9, ≈ 11 MB for the
five scenes), `minimap/*` (map scenes, map icons, 61 sidecars), `hud/zoom-table.json` and
`gear/pose-fits.json`. Kept local and reproducible (hashes in the manifest): `tables.bin`,
`models/baked/*` validation bakes, raw `scenes/*.bin`, `scenes/*.pinned*`, and `blocks/`
(61 squares, ≈ 75 MB gzip — serve them next to the manifest for region scenes).

## Comparing captures

```bash
python3 tools/render-assets/compare.py candidate.png source.png --diff diff.png
python3 tools/render-assets/compare.py --batch <candidate_dir> <source_dir> --diff-dir <dir> [--json]
```

Profile `native_scene_model` (research/reference-pack/v1/comparison-policy.json): interior max
channel error ≤ 2, interior mean ≤ 0.35, 1-px band around **source** edges, changed band
fraction ≤ 0.5 %, no masking. Diff images mark interior differences red and band differences
amber over a darkened source.
