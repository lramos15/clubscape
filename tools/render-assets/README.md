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
python3 tools/render-assets/export.py --profile compress    # (re)write scenes/*.gz + manifest
python3 tools/render-assets/export.py --profile unpack      # restore raw scenes/*.bin from *.gz
python3 tools/render-assets/export.py --verify-only         # hash-check assets/compiled/render
```

Profiles: `tables`, `palette`, `textures`, `models` (tree 1277 / model 1570 lit as captured),
`npcs` (3028 goblin, 2063 penguin; sequences 6181/6180 and 5668/5666), `scenes`,
`prune-textures` (keep only textures referenced by the exported scenes/models),
`compress`, `unpack`.

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

Published in git: `manifest.json`, `palette.bin`, `textures/*.bin` (33 used textures), base
models, NPC packs and `scenes/*.bin.gz` (deterministic gzip: mtime 0, level 9, ≈ 13 MB for the
five scenes). Kept local and reproducible (hashes in the manifest): `tables.bin`,
`models/baked/*` validation bakes, raw `scenes/*.bin`.

## Comparing captures

```bash
python3 tools/render-assets/compare.py candidate.png source.png --diff diff.png
python3 tools/render-assets/compare.py --batch <candidate_dir> <source_dir> --diff-dir <dir> [--json]
```

Profile `native_scene_model` (research/reference-pack/v1/comparison-policy.json): interior max
channel error ≤ 2, interior mean ≤ 0.35, 1-px band around **source** edges, changed band
fraction ≤ 0.5 %, no masking. Diff images mark interior differences red and band differences
amber over a darkened source.
