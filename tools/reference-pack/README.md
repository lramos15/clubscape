# Source reference-pack tooling

This tooling retrieves and validates **source evidence**, makes explicitly
unapproved reference compositions, and builds a review gallery. It never
starts ClubScape, renders a game world, logs into OSRS, creates an account,
re-converts source audio, or approves a baseline.

Read `AGENTS.md` and `docs/machines/sparky.md` first. The executed environment
was Linux ARM64, Python 3.12.3, Pillow 10.2.0, Node 24.18.0 and existing
Chrome for Testing 153.0.8010.12. No dependencies were installed or host
configuration changed. Python uses the existing Pillow package; FLAC
validation reuses the existing checksum-locked `tools/audio-import/codec.py`
reader and libsndfile/FLAC libraries. Encoding functions are never called.

From this worktree's root:

```sh
python3 tools/reference-pack/build.py
python3 tools/reference-pack/validate.py --report --require-complete
python3 -m unittest discover -s tools/reference-pack -p 'test_*.py' -v
node --check tools/reference-pack/browser-check.mjs
```

`build.py` reads only current source fixtures/contracts and owned evidence.
It writes only `research/reference-pack/**`. Existing original images, audio,
source schemas, game code, canonical specs and approval records are unchanged.
Exact source-matching results do not compare any product candidate.

The parent-authorized source-input commit `db103ba` has been integrated.
`native_hud.py` consumes its16 original full-frame Classic HUD/panel/family
images, validates their actual frame and64 native UI-region pixel hashes,
and binds the source metadata into the existing126 cases/29 families.
It does not rerender or edit source-capture code. The original93 fixtures,
100 public images and seven proposals are preserved. The current264-FLAC
audio manifest includes the integrated native-percussion correction and six
new effects. Two previously generated native cue WAVs2693/710 are separately
preserved unchanged as reference templates, not re-converted runtime assets.

`factoring.py` audits literal Section30.2. It retains126 cases and71 tutorial
states while declaring29 visual families,180 distinct variants,11 instructor
phases and11 HUD signatures. `text_oracles.py` projects698 exact pinned
transcript records into independently checked desktop text, source style
runs and native-font metrics; it never renders a new "source capture."
`evidence-families.json` separates missing reference inputs from later
candidate state/source-fidelity/behavior/platform/owner acceptance evidence.

The strict completeness command now passes: native HUD, qualified source
selector observations and the exact owner-approved two audio adaptations
supply all required reference inputs. The gate does not
demand71 separately captured source microstates or an authenticated
arrival-container screenshot:

```sh
python3 tools/reference-pack/validate.py --require-complete
```

Passing normal validation means bytes, decoded images, source classification,
current component/font proofs, original PCM, all distinct visual
families/states/signatures and the complete **case index**
are sound. Strict completeness additionally checks every required input.
Neither grants owner approval or product acceptance. The input pack is
complete; overall approval remains pending.
`validation.json` records these separate facts.
Tests deliberately remove/duplicate cases, remove actual inputs, change
hashes/dimensions, relabel icons/crops as panels/full frames, fabricate
settings/dates, change native metadata, relax tolerances, add masks, promote
proposals, alter silence and falsely assert Mac/owner results. They also
remove a native HUD frame, falsify native region/attachment data, or promote
synthetic fixture text into source dialogue. The native body strings are
for replaying their controlled pixel state only; actual dialogue still uses
the698 pinned transcript records. Tests also
remove family variants (including player/NPC dialogue, spell filtering and
quest ingredient states), lose phases/signatures, reintroduce per-state
screenshot demands, change text/values/glyphs or leave holes in full-panel
pixel partitions. A positive unit case proves that readiness can be true
when real input requirements are satisfied, without marking approval.

## Current audio source/adaptation application

`audio_reference.py` hash-binds the current264-file audio manifest, all
correction/observation/native queue evidence and exact owner record
`milestones/m1-audio-trigger-approval.json`. It rejects altered approval
scope, stale musical hashes, lost percussion-bank setup, changed prior SFX,
missing cue templates, fabricated dates/current-build labels, duplicate food
callbacks, double offsets and invented jingle priority.

The audit reads the pre-correction manifest from the pinned base commit
`e74026a3e9764ea9dd81a19ddd6f87abc32a70bb` only to verify the26-file musical
delta and preservation of223 prior SFX/silence. It does not retain those old
musical hashes as active baselines. All264 FLACs are actually decoded and
checked against the current declared PCM.

The exact native reference WAVs are committed. Routine builds need no other
worktree. Their original one-time, no-conversion import was:

```sh
python3 tools/reference-pack/audio_reference.py --materialize-templates \
  --source /home/lramos15/clubscape/.worktrees/m1-audio-bindings/.local/audio-bindings/candidate-pcm
```

The importer required matching prior waveform-comparison SHA-256 and native
decoded-PCM provenance before copying. It did not use the separate historical
eight-bit control waveforms.

## Exact public retrieval

The final acquisition list and every exact original blob/page identity are
in `research/reference-pack/v1/public-media.json` and `pages.json`.
`sources/*.json.gz` retains complete original file descriptions/notices and
page-revision content. The full upstream widget-symbol source, including
its BSD notice, is retained as a hash-checked gzip.

```sh
# Exact pinned originals and public page revisions, to isolated scratch:
python3 tools/reference-pack/retrieve.py --output .local/reference-pack/retrieved

# A bounded network check, without refetching every source:
python3 tools/reference-pack/retrieve.py --output .local/reference-pack/retrieved --limit 3
```

The original MediaWiki SHA-1 and byte length are required in addition to our
SHA-256. **Cloudflare Polish may recompress even PNGs requested from their
original URL.** Such responses are rejected, not silently accepted as the
original. A fresh public `reference_original` query key prevents replaying
that optimized cached response; the returned bytes must still exactly match
imageinfo. If a file has since changed, the retrieval tool asks MediaWiki for
the exact recorded upload timestamp, follows its original archived URL and
requires the pinned blob hash. No login, cookies, credentials or alternate
private origin is used.

New discovery is explicit and bounded, not routine validation:

```sh
python3 tools/reference-pack/fetch.py pages 'Learning the Ropes' 'Music Player'
python3 tools/reference-pack/fetch.py search '"Tutorial Island"' --namespace 6
python3 tools/reference-pack/fetch.py search '"Tutorial Island"' --namespace 6 --offset 30
python3 tools/reference-pack/fetch.py media-info 'Learning the Ropes reward scroll.png'
python3 tools/reference-pack/fetch.py retrieve 'Learning the Ropes reward scroll.png'
```

`media-info` retrieves the latest blob metadata, while routine reproduction
uses `retrieve.py` and **pinned** identities. Older history continuation is
not described as exhausted. Adding an input without an explicit disposition
and case/supplementary mapping fails assembly. Do not refresh an approved
version in place or treat a new upload timestamp as its capture date.

## Source recording and gallery browser check

The first run genuinely decoded the public 1920x1080 MP4 in sandboxed Chrome,
extracted full source frames at 0/5/.../40 seconds and decoded nonzero audio.
The exact recording, browser, source hash, seek times, dimensions and decoded
signal hashes are in `research/reference-pack/v1/browser-media.json`.
The source-frame time is a video seek timestamp, not an observed native
animation-clock/packet offset.

The existing read-only Playwright installation used on this host was:

```sh
node tools/reference-pack/browser-check.mjs \
  --chrome /home/lramos15/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome \
  --playwright /home/lramos15/clubscape/.worktrees/m1-browser-platform/tools/browser-harness/node_modules/playwright-core/index.mjs \
  --gallery-only

# Validate the final pack (no source images/audio/proposals are rewritten):
python3 tools/reference-pack/validate.py --report
```

`--gallery-only` preserves the existing original recording/frame provenance.
It records the reviewed manifest hash in `v1/gallery-validation.json`, checks
all126 case IDs,29 family IDs and11 HUD signature IDs at all four viewports,
all16 native HUD frame IDs, seven selector records,264+2 audio controls, the
owner-summary manifest hash/contact sheet and actual decoding of the two
source WAV references. It exercises the71-state filter. Use `--gallery` only when deliberately
reproducing the original source-recording decode as well.

Other hosts must supply their own existing compatible browser/Playwright
paths rather than assume Sparky paths. This is ordinary source-image/video
and HTML review-tool validation, **not WebGPU gameplay or a Mac benchmark**.
The local server is bound to `127.0.0.1` on an ephemeral port, serves only the
reference/audio paths, is checked for responsiveness and is closed in
`finally`. Chromium's sandbox is enabled; no sandbox-disabling flags are used.

To review without a server, open
`research/reference-pack/v1/owner-review.html` for the compact decision scope
and exact manifest SHA, then `gallery/index.html` for the full indexed pack.
`owner-review-contact.png` is a concise whole-image review sheet. The gallery never
autoplays audio or video. Full source links preserve native pixels; contact
sheets are identified as scaled navigational previews, not comparison
baselines. Product implementation, live journey, native script/state
calibration, final Mac Chrome/Edge results and all approvals remain separate.

`audio-reference.json` preserves the final selector/queue policy.
`audio-handoff.json` marks its required inputs complete. Learning the Ropes152
and ordinary-food2393/12526frame1/four cycles remain `approved_adaptation`,
NOT verified current OSRS selectors. Dated Cook/shortbow/rat/goblin/smelting
observations retain their dates. Only the owner can grant the separate final
pack approval or later product acceptance.
