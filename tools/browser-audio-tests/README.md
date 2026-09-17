# Running-browser/native audio comparison

The fixture imports the real `web/audio/index.ts` factory. It is not a second
audio engine or a file-decoder substitute. The source comparisons now use
executed original native policies in `research/browser-audio-policy/`, in
addition to the frozen v1.3.0 source files. Neither the fixture's synthetic
world snapshots nor its HTML controls are ClubScape gameplay/UI acceptance.

Read `AGENTS.md`, `prompt.md`, and `docs/machines/sparky.md` first. The already
verified worktree dependencies are Node24.18, TypeScript7.0.2 and Playwright
Core1.63.0. Restore only after a missing-dependency failure:

```sh
pnpm --dir web install --frozen-lockfile
```

## Commands

```sh
pnpm --dir web exec tsc --noEmit
node --test web/audio/audio.test.ts web/audio/native-policy.test.ts web/audio/reward-levels.test.ts web/audio/supplement.test.ts web/audio/preferences.test.ts

# Full actual playback; complete playlist, Modern-area and single timer replays:
node tools/browser-audio-tests/run.mjs

# Iteration: shorter lifecycle checks plus actual offline waveform/fade render:
node tools/browser-audio-tests/run.mjs --quick

# Bounded preference controls only; muted output for shared-host privacy:
python3 tools/browser-audio-tests/native/preference_controls.py run
node tools/browser-audio-tests/run.mjs --preferences-only --quick --mute-output
# Full existing playback + new preference/native Single duration comparisons:
node tools/browser-audio-tests/run.mjs --mute-output

# New results without replacing historical component evidence:
node tools/browser-audio-tests/run.mjs --mute-output \
  --report-file research/browser-audio-policy/cue-timing/component-full.json
```

The cue-timing continuation also runs the original shell's composed
audio/storage fixture against its immutable delivered bridge and the owned
candidate repair. See `research/browser-audio-policy/cue-timing/README.md`.
Both harnesses now retain the real server's COOP/COEP/CSP; all script
orchestration in the owned driver remains gesture-free. These are page
response headers, not changes to global browser/graphics/sound settings.

Reproduce the native probes first when changing policy:

```sh
python3 tools/browser-audio-tests/native/probe.py preferences
python3 tools/browser-audio-tests/native/probe.py position
python3 tools/browser-audio-tests/native/probe.py music
python3 tools/browser-audio-tests/native/probe.py objects
python3 tools/browser-audio-tests/native/probe.py catalog
python3 tools/browser-audio-tests/native/probe.py scripts
python3 tools/browser-audio-tests/native/probe.py pcm
python3 tools/browser-audio-tests/native/public_maps.py
python3 tools/browser-audio-tests/native/generate_policy.py
```

The authorized additive source publication is separate from the frozen pack:

```sh
python3 tools/browser-audio-tests/native/audio_supplement.py publish
python3 tools/browser-audio-tests/native/audio_supplement.py verify
python3 tools/browser-audio-tests/native/supplement_oracles.py
```

Publication renders each of the nine originals **twice** at native255, compares
all PCM/source input/instrument relationships, and losslessly expands the original
16-bit PCM into unity-gain FLAC24. Additional actual native44/136 renders check
the configured slider-level gain behavior. It never writes base source files.
`supplement_oracles.py` derives both channel hashes from independently checked
integer PCM **before** browser decoding. Saturated samples remain exact native
values; this avoids the old FLAC16 positive-full-scale float bias without
changing a waveform, adding attenuation, a limiter or a new tolerance.

The native runner reuses hash-locked source-owner artifacts/private cache.
`--reuse`/`--cache` can select equivalent verified local copies. It does not
fetch a game pack or launch a game/account session. The owned isolated JVM
sets `-Duser.home` and `-Djava.io.tmpdir`; no personal preferences are read or
written. See the native research README for exact method/data bindings.

## Real browser evidence

The browser executable is checked as Chrome for Testing153. Default:
`~/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome`;
`CLUBSCAPE_CHROME` may point to an equivalent already verified executable.
The actual sandbox page must confirm namespace/PID/network/seccomp protection.
`--no-sandbox` is never used. Normally the default headless `--mute-audio` is
removed. The explicit `--mute-output` option retains that **browser-local**
privacy flag and records it in the evidence; it does not change host sound
services, sink routing, native context state or the real production graph.

**All fixture JS evaluation uses CDP `userGesture:false`.** Playwright's normal
`evaluate` marks evaluation as a user gesture and would invalidate autoplay
proof. Only actual mouse/keyboard input unlocks/resumes the context. Both
programmatic unlock and synthetic DOM click are rejected beforehand.

The tests instrument native `AudioBufferSourceNode.start`/`ended` calls, context
state/time/rate, and actual graph connections. A real AudioWorklet monitors
post-gain/post-mute samples and both channels' clipping/peak counts. Its own
branch outputs zero, but the production master remains **independently
connected to the real AudioDestinationNode**, and that connection is checked.
Neither a decoded buffer nor a nonzero monitor alone is treated as playback.

The suite exercises:

* Corrected original music0/62/144/76/2 and actual bow2693, rat710/713/711,
  goblin469/472/471, smelt2725 and source gathering/eating cues.
* All nine additive inputs actually start through the factory: original Modern
  music64/327/163/145 and native255 jingles40/54/58/64/65. Their complete decoded
  channel hashes match independent native-PCM float oracles exactly.
* Real slider movement across native128→255 representation selection with a
  delayed load. The old source is swapped at its current offset/end; configured
  volume is preserved without duplicate music, clipping or an action restart.
* Real native defaults255/127/127, nonlinear channel/master composition, live
  original-WAV waveform/gain comparisons, and source fader master steps.
* Native packet FIFO50, delay2 on processing call3, `-10` loading grace,
  original silence2411's74/100 branch and its queue occupancy.
* Stable IDs, server/frame callback correlation, both approved adaptations,
  repeat snapshots, source modal-deferred Cook reward playback including a
  pre-trained2→5 committed delta with supplied group34, reconnect/reset,
  last-accepted jingle replacement, ignored auxiliary values and `-1`.
* A source scene projection with original object114's **1×2 footprint**, native
  rectangle/retention gain, signed fades, native150ms visibility fade, wrong
  plane/instance, old-node cleanup and no caller-supplied `sourceGain`.
* Original morph34815/source varp491 preventing false base-sound3141 playback;
  actual random object16433 ambience from its source groups/interval, outside
  the packet FIFO.
* Actual native MIDI fade steps and an `OfflineAudioContext` waveform render.
  Every sample is checked. The exact discontinuity may select the immediately
  adjacent gain step due to a single time-to-sample rounding; that error is
  recorded independently in sample/time units, well within the existing20ms
  event bound, rather than widening the PCM tolerance.
* Full-length explicit playlist replay at source table44's229×600ms duration
  (**137.4s**), distinct from the asset's exact native MIDI EOT. The file's
  original release is retained; it is not made into a seamless buffer loop.
  This is a declared source-duration projection, not a capture of the server's
  exact per-account next-song submission time.
* Full-length Modern-area next-track selection without any caller selector,
  source163 single-mode replay at194×600ms, and idempotent declared
  area/single/shuffle/playlist state with source-unlock checks.
* Unknown/mismatched IDs, noncommitted input, corrupt/truncated bytes, HTTP
  failure, same-origin/hash checks, decoder failure, cancelled stale loads,
  and no snapshot-driven large-file retry storm.
* Real context suspension/closure and a real browser-local silent output sink,
  plus explicitly labeled injected resume rejection/timeout, with real
  gesture recovery and no duplicate resumed music.
* Pre-trained Cook2→5 reward deferral and source-group preservation plus
  pre-trained21→21 with no level-up playback, both through the real factory.
* Direct Skip through a real keyboard gesture, native Area/Single guards,
  source click2266, same pending request coalescing, remembered jingle track/
  transition replacement, same-group zero-transition retention, and complete
  shuffle bags across mute, reconnect and rollover.
* Three separately identified100-slot saved playlists, actual source track
  encoding, holes/first-hole reuse, original selection/keep-playing behavior,
  explicit empty state, and rejected corrupt/unknown/locked preferences.
* Native first-use100/20/45/25 and actual saved37/21/66/83 mute restoration,
  slider-zero memory semantics, global privacy mute without preference changes,
  detached serialization, same-state idempotence, reset/character noninheritance
  and an old pending gesture rejected after logout/re-entry with the same ID.
* A full194×600ms bound-preference Single replay with native4137 clear,
  corresponding to original9630's unconditional Single re-entry, without
  looping the released samples or introducing clipping.

The saturation fixture uses an explicit0.01 gain for50 simultaneous packet
effects. It proves queue/order/lifecycle behavior without manufacturing a mix
clip; it does not claim50 adversarial unity-gain overlaps cannot clip.

## Clipping, tolerances and qualification

The unchanged bounds are in `research/browser-audio-policy/bounds.json`:
original/decoded source identity, decoder float allowance2.3e-5, gain0.25dB,
known source-cycle timing20ms, zero **additional** clipping.

Native source full volume can itself clip. Independent original native128 and
255 renders of all35 musical inputs distinguish this from new clipping.
Jingle33 at native255, for example, has12 saturated source samples; the frozen
128 render scaled to the corresponding gain has8 at a subset of those positions.
The suite does not falsely assert an absolute-zero global clip counter after
that legitimate native configuration. SFX stress and safe music transition
checks compare their **additional** clip counts. The five formerly unsafe
scaled-jingle cases now use their original native255 supplemental representations.
No limiter or normalizer is used. Historic failure proof stays in
`native-pcm.json`; exact fulfilling PCM and rendered controls are in
`supplement-source-inputs.json` and `supplement-level-calibration.json`.

The focused Cook conformance cases additionally run:

```sh
python3 tools/browser-audio-tests/native/cook_reward_boundary.py
node --test web/audio/reward-levels.test.ts
```

Their source numeric fixtures use unchanged recipe/reward XP and original
thresholds; they do not play/seed a quest or alter gameplay XP. The existing
browser completion fixture now covers pre-training, an XP notification before
the supplied completion in one transaction, wrong-scroll close, duplicate
callbacks and source-group preservation. No production quest event is
manufactured by the audio implementation.

## Reproducibility and containment

`results.json` records the last run's full/quick scope, versions, actual native
starts/ends, waveform/gain/timing, observed failures, and tested implementation
hashes. All266 original playable files **and nine additive files** are checked
before and after every run; the base manifest/approval hashes remain unchanged.
A code/harness change during a run invalidates its result. Missing callbacks
after deliberately closing the actual context are recorded as missing, not
fabricated; graph/resource cleanup is separately asserted.
`--preferences-only` writes the separate `preference-results.json`, so focused
iterations do not replace the broader playback result. A full run includes all
prior32 browser cases and nine additional preference/control cases. Historical
29-unit/32-browser/85-pack evidence remains in the earlier component commit;
new calibration is additive and does not promote those checks to gameplay
acceptance. The native preference probe is limited to twelve fixed state
fixtures; it repeats those same cases in two isolated JVMs, never opens an
account and fails on swallowed native interpreter errors.

The server binds a random **loopback-only** port and serves only allowlisted
modules, fixture, pinned metadata and original audio. Other browser network
requests are blocked. The worktree-local profile/caches use ignored `.run/`;
Chromium's short socket root is the existing owned `web/audio/` directory.
Only this runner's newly created scratch entries are removed. No `/tmp`,
global sound service, driver, sandbox setting, personal profile, process
detachment or source-file rewrite is used. Browsers and server are closed in
`finally`, and the final runtime graph has no retained connections.

These are audio-only headless tests. Screenshots would require the machine
guide's headful Xvfb setup. Physical host-speaker perception, real game/server
journey, owner Mac/Edge, presentation fidelity acceptance and full M1 acceptance
are still separate gates. The exact additive publication/bridge contract is in
`web/audio/README.md` and `research/browser-audio-policy/README.md`.
