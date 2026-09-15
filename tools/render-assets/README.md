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
python3 tools/render-assets/export.py --profile compress    # (re)write scenes/*.gz, blocks/*.gz + manifest
python3 tools/render-assets/export.py --profile unpack      # restore raw scenes/*.bin from *.gz
python3 tools/render-assets/export.py --verify-only         # hash-check assets/compiled/render
```

Profiles: `tables`, `palette`, `textures`, `models` (tree 1277 / model 1570 lit as captured),
`npcs` (3028 goblin, 2063 penguin; sequences 6181/6180 and 5668/5666), `scenes`, `blocks`,
`scenes-pinned`, `anim`, `dynamic`, `widgets`, `prune-textures` (keep only textures referenced by
the exported scenes/blocks/models), `compress`, `unpack`.

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
models, NPC packs and `scenes/*.bin.gz` (deterministic gzip: mtime 0, level 9, ≈ 11 MB for the
five scenes). Kept local and reproducible (hashes in the manifest): `tables.bin`,
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
