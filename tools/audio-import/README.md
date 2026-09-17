# Original M1 audio conversion

**Source preparation, not ClubScape presentation implementation or acceptance.**
This runs the checksum-locked **original injected 1.12.38 audio bytecode**
against **cache2695/build240**. It does not use a generic soundfont, Java MIDI
synthesizer, replacement sound, source-game account, sound device, or public world.

## Outputs

The canonical manifest is
[`audio-runtime.json`](../../assets/manifests/osrs/audio-runtime.json).
Playable assets are under `assets/source/osrs/audio-runtime/`:

* Five complete required music tracks: **Scape Main0**, Newbie Melody62,
  Scape Cave144, Harmony76, Autumn Voyage2.
* Thirty source quest/low-level skill/combat/death jingle inputs. Some are
  explicitly alternatives, not an assertion they all play in the journey.
* 229 audible original SFX: scene ambience plus the identified journey sounds,
  including the six source-identified additions in the binding audit.
* One additionally decoded **source silence2411**, not a playable-file count.
  Its5ms definition produces110 zero samples in the original synth. Keep its
  **74/100 weight** in sequence13612/frame1; removing it and renormalizing the
  other sounds would change source behavior.

Music/jingles are **22050Hz, stereo, FLAC16**, with native synth volume128.
SFX are **22050Hz, mono, FLAC24**, containing all16 original source bits at
exactly half gain. The24-bit container makes that attenuation **lossless and
exactly reversible**. Original synth saturation is preserved and counted;
there is no new limiter, normalization, clipping, resampling or replacement.
The original unattenuated16-bit PCM hash is retained for every asset.

The binding follow-up found and corrected a missing native startup call:
`dg.ay` initializes MIDI channel9 to percussion bank128. The earlier renderer
omitted that call. All35 musical inputs were rechecked;26 file payloads changed,
while **all223 existing SFX hashes and silence2411 stayed unchanged**.
See `research/audio-source/musical-startup-correction.json`. A reference pack
containing the old musical hashes needs a parent-owned refresh; this is not an
approval change.

The narrower cue/selector audit and its remaining blockers are documented in
[`research/audio-source/bindings-README.md`](../../research/audio-source/bindings-README.md).
`python3 tools/audio-import/bindings.py audit` reads the original cache without
write handles. `publish-audio` performs the necessary musical correction and
new-cue import, using `convert --reuse-existing-sfx` rather than re-rendering
existing effects.

## Reproduce

Read `AGENTS.md`, `prompt.md` and `docs/machines/sparky.md` first. Run from this
worktree's root with existing Python3.12/JDK17. Dependency hashes are in
[`dependencies.json`](dependencies.json). The final FLAC byte lock describes the
verified existing ARM64 libsndfile/FLAC libraries; it is not an instruction to
change global packages. Another encoder build must not be silently relabeled.

With a local copy of the existing current-source extraction:

```sh
python3 tools/audio-import/audio_import.py prepare
python3 tools/audio-import/audio_import.py convert
AUDIO_IMPORT_INTEGRATION=1 python3 -m unittest discover \
  -s tools/audio-import -p 'test_*.py' -q
python3 tools/audio-import/audio_import.py validate
python3 tools/audio-import/audio_import.py verify-inputs
```

For the task-supplied source worktree, only **read** its artifacts:

```sh
python3 tools/audio-import/audio_import.py prepare \
  --extracted ../m1-runtime-inputs/.local/current-source/extracted \
  --artifacts ../m1-runtime-inputs/.local/current-source \
  --cache ../m1-runtime-inputs/.local/current-source/cache-2695
```

`prepare` first reuses verified JARs in `.local/audio-import/tooling/`, then the
supplied artifact directory and its `tooling/` child. If an artifact is missing,
it fails with its name. `--fetch-artifacts` permits only the locked HTTPS URLs,
sizes and SHA-256 values. The cache decoder prerequisite and whole-cache
retrieval are documented in [`tools/cache-import`](../cache-import/README.md);
do not change its source selection to work around an integrity failure.

All large originals, a private cache copy, generated classes and native WAV
intermediates stay in ignored `.local/audio-import/`. Opening RuneLite's Store
can create empty index files, so the tool **never opens another worktree's disk
store**: it first verifies and copies all original disk files, then rechecks the
private copy after decoding. Missing/corrupt inputs are not silently replaced.
Java `user.home`, scratch paths and compiler paths are worktree-local; JVM
performance-file creation is disabled. No personal RuneLite settings are read.

The actual file layouts of all230 selected source sound groups are inspected.
They have only file0. Additional/digital files would fail explicitly rather than
silently falling back to legacy data.

### Browser file-decoder check

Use the already verified Chrome executable; no npm packages are needed:

```sh
node tools/audio-import/browser_check.mjs /path/to/verified/chrome
```

The fixture binds a random loopback port, serves only these assets and their
checked numeric oracles, launches an isolated sandboxed browser, and uses
`OfflineAudioContext.decodeAudioData`. Every decoded float32 channel must match
an independently calculated integer-to-float conversion model **exactly**.
It checks real samples/durations, not merely HTTP success or container headers.
The browser and server are closed afterwards.

Chrome153 uses a float32 reciprocal of32767 for positive int16 samples, versus
32768 for negative samples. All35 music/jingle files exactly matched that
conversion; the FLAC24 effects matched the signed32-bit power-of-two model.
The largest measured difference from the symmetric reference float
representation for the corrected pack was **0.000022172927856445312**. Original integer PCM and the
lossless files are unchanged. No empirical tolerance or listening-equivalence
claim is substituted for the exact forward-model checks. Expectations are
generated from hash-verified source PCM **before** browser decoding.

The fixture uses the short worktree-local `.local/` socket path to stay within
Linux's Unix-domain-socket path limit; it does not use a global scratch path.

This does **not** start audible playback or exercise a ClubScape UI, autoplay,
gestures, live triggers, transitions, reconnects, mute or volume controls.
Those remain separate acceptance work after pack approval.

## What is actually executed

`SourceAudio.java` invokes the original:

1. Jagex track decoder and MIDI event reader.
2. Instrument patches, original compressed samples/Vorbis setup, and sample
   decoding, including original key tuning, exclusive classes, pan and loops.
3. MIDI sequencer, note envelopes and PCM mixer.
4. Original PcmPlayer scheduling/voice budget in its **512-frame blocks**.
5. Original PCM device quantization, captured through a write-only
   `SourceDataLine` proxy; no device is opened.
6. Original16-bit SFX envelope/oscillator/noise/filter implementation.

Only the archive transport is adapted. An uninitialized original Archive
subclass supplies verified bytes without executing the network/disk-registering
constructor. The exact official client resource JAR supplies `/runelite/index`;
all selected audio groups are checked to have **no RuneLite overlay**.
The original runtime is not patched or replaced with a decompiled imitation.

The current original track decoder sometimes reuses0xFF metadata running status;
the cache library emits explicit statuses. `midi.py` compares **every normalized
track, event and tick**, not just file lengths, and records the equivalence.
It independently computes the original integer MIDI clock/end frame, which is
checked against the native player's end-of-track block.

## Timing and source relationships

`research/audio-source/source-map.json` joins actual source sequence frames,
sound IDs/weights/repeats, object sound fields and source placement regions to
the journey rule IDs. Net fishing621, bronze mining625, firemaking733,
bronze chopping879 and smithing898 have actual embedded sound events.

SFX instrument offsets are already baked into the full waveform; do not add
them twice. Source object distance/retention/change-tick/visibility/fade fields
and raw loop sample positions are retained without assuming their live state.
Nominal20ms frame-start sums are not calibrated wall-clock enqueue offsets.

Tracks contain **one native pass plus1s release**. They are **not** falsely
advertised as seamless loops. Source MIDI end ticks, exact integer-clock end
frames, instrument sample loops and the unobserved music-player repeat policy
are distinct manifest fields. Do not blindly repeat the release-padded file.

Unclosed current ranged/NPC-variant/weapon/eating selectors remain explicit,
even where a cue identity has been established. Smelting2725
is a prepared, explicitly **unverified current-binding candidate** from an
independent implementation using a different animation. UI2266 is not a sound
for every click. No missing action receives an arbitrary replacement.

## Evidence and tests

* `extra-inputs.json.gz`: actual additional extraction, original group CRC/full
  revision/container hash, file IDs and individual output hashes.
* `layout-check.json`: actual original layouts for all selected SFX groups.
* `conversion-evidence.json.gz`: all original patch/key/sample relationships,
  native PCM hashes, MIDI event comparisons, original device checks, SFX
  durations/offsets/echo parameters and loop bounds.
* `validation.json`: actual published-file decode, sample/duration/level/
  spectral measurements, exact FLAC round trips and reversible source PCM.
* `browser-decode.json`: separate browser **file decoding**, never playback
  acceptance.
* `references.json`: pinned symbol/wiki identities, hashes, provenance and
  explicit confidence limits. Research prose/code snapshots stay ignored.

The tests cover corrupt/missing inputs, unsafe paths, unknown staged files,
nonzero CLI failures, malformed MIDI, exact event equivalence, duration-vs-echo
field regression, original silent weighting, FFT/signal checks and lossless
codec behavior and exact codec float-conversion models. The original audio suite's
integration test re-renders a complete Scape Main, a quest
jingle, and six representative actual effects and checks their original PCM
hashes again. This is mechanical signal/source evidence, **not** a claim of
human perceptual equivalence.

The binding follow-up adds native opcode/queue tests, current metadata/
callback assertions, explicit unclosed-gate checks and recording-matcher
positive/negative controls. Install its optional, hash-pinned research packages
only under `.local/audio-bindings/python` using
`binding-media-requirements.txt` and `binding-analysis-requirements.txt`.
Set `TMPDIR` and the package cache path inside `.local`; no global environment
or personal downloader configuration is needed.

Notices: [third-party dependencies](THIRD_PARTY_NOTICES.txt) and
[original audio](../../assets/source/osrs/audio-runtime/NOTICE.txt).
