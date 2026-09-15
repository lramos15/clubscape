# Original browser audio

`index.ts` implements `CreateAudio` from `web/shared/contracts.ts`:

```ts
import { createAudio, AudioFailure, observeAudioState } from "./audio/index.ts";

const audio = await createAudio(assets, reportAudioFeedback);
const unsubscribe = observeAudioState(audio, showActualAudioStatus);

// Invoke directly in the trusted input handler, before unrelated await calls.
soundButton.addEventListener("click", () => {
  audio.mute(false);
  void audio.unlock().catch(reportAudioFeedback);
});

// These are immutable, validated server/renderer projections, not UI intents.
audio.update(worldView, audioEvents);
audio.volume("music", 0.5);
audio.volume("effects", 1);
audio.volume("area", 1);
audio.mute(true);
audio.disconnected();

// On final app teardown:
unsubscribe();
await audio.dispose();
```

The example's UI/reporting functions belong to app composition. This directory
does not implement UI, gameplay, quest rewards, inventory consumption, or server
authority. The executable browser fixture imports **this factory**, not a second
implementation or a decoder-only stand-in.

## Assets and actual output

The factory checks the shared pack identity
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`.
`AUDIO_INPUTS` exports the three required metadata path IDs and their exact
hashes: the **corrected** audio manifest, source sequence/object map, and approved
audio-reference contract. `ClientAssets.url(id)` must resolve these paths and
the original asset IDs through same-origin HTTP(S). Metadata is fetched as
bytes, not through `ClientAssets.json`, so its original hash can be verified.
Asset URLs cannot redirect or send audio to another origin.

The manifest contains 264 original FLACs. The approved reference contract also
provides original WAVs **`reference.audio.sfx.2693` and
`reference.audio.sfx.710`**; neither has an invented FLAC alias. All 266 playable
payloads are checksum-locked. Those two WAVs are unattenuated native PCM, so
their voice gain is exactly 0.5 to match the FLAC effects' already-baked,
reversible half gain. No file is modified.
Their observed bindings request one pass; repeat counts above one are rejected
because the reference WAV records do not establish native sample-loop bounds.
FLAC effects use their retained actual loop bounds and preserve the tail on
finite repeats; they do not repeat arbitrary leading silence or replace the
waveform with an oscillator.

The graph is:

```text
original AudioBufferSourceNode -> per-voice GainNode
  -> music / effects / area GainNode -> mute GainNode -> AudioDestinationNode
```

An actual `AudioContext` renders at 22050 Hz. Device-rate conversion, if needed,
is the browser's output-device operation, not a replacement source conversion.
No oscillator, SoundFont, compressor, limiter, normalization, or replacement
sound is used. Musical stereo and effects' original samples are retained.
Decoder dimensions/finite samples/peak bounds are checked after file hashes.

Only metadata loads before a gesture. A selected track, its explicitly needed
playlist successor, and current activity/emitter cues load on demand. A jingle
also prepares the remembered regional track without starting it prematurely.
The decoder has four load slots and a 96 MiB LRU decoded cache. Superseded loads
are aborted; stopped sources and their gains are disconnected. Unknown,
oversized, truncated, corrupted, or undecodable inputs produce `AudioFailure`,
never silence masquerading as successful playback.

## Permission, feedback, and lifecycle

`unlock()` checks **transient** `navigator.userActivation.isActive`, invokes the
real `resume()` before awaiting any assets, waits for actual `running` state,
and rejects a timeout/failure. Factory resolution does **not** mean playback is
unlocked. `AUDIO_GESTURE_REQUIRED` is recoverable sound-control feedback, not a
fatal game/renderer error; the app must classify it accordingly.

`readAudioState(handle)` and `observeAudioState(handle, listener)` expose actual
context state/time, pending gesture, output route, gain settings, background
selection/exhaustion/failure, queued sources, buffer accounting, and a bounded
trace. They never advance the engine or fake readiness. A running clock routed
to Chrome's `{type:"none"}` output sink is explicitly **not enabled playback**.
Real suspension/interruption, sink changes and unexpected context closure are
reported. A suspended existing musical node resumes without creating another
copy; queued transient effects are discarded rather than burst later.

`disconnected()` cancels transient work and all playing nodes but preserves
event deduplication and the remembered selection. On a validated reconnect,
call `update(newWorld, newEvents)`; unlock again if the browser actually requires
a gesture. **Do not translate reconnecting to `update(null, [])`**:
`null` means an explicit title/logout reset and selects Scape Main 0.
Repeated unchanged snapshots do not restart music.

Mute and channel zero reject new transient requests. Rejected/muted effects
are not replayed when controls are restored. An asset failure does not cause a
large music fetch on every world snapshot; a new explicit selection, music
control retry, or trusted unlock retries it. `dispose()` is idempotent, stops
the source clock, aborts loads, disconnects the graph and closes the context.

## AudioEvent adapter contract

Keep the shared envelope unchanged. `sourceId` means an index-4 group for
`sound`, index-6 group for `music`, index-11 group for `jingle`/`level_up`, a source
sequence for `animation`, and widget 153 for the supported quest-scroll close.
Use `assetId: null` or the **matching** exact catalog ID. IDs and scalar
payloads are bounded/validated; nonfinite controls are rejected.
Only the fields below (plus `committed`, `actionId`, `cueId`) are accepted.
Unknown `gain`/fade/offset/trim/priority aliases fail explicitly; they do not
silently select a default. Use `sourceGain` for an explicitly supplied mix gain.

All game sounds require `payload.committed: true`. This is a contract with the
validated app adapter, **not client authentication or permission to derive an
outcome from a button click**. Quest completion is additionally checked against
the corresponding completed authoritative `WorldView.player.quests` entry.
Initial snapshots seed completion history without playing old completions.

| Kind | Supported payload and meaning |
| --- | --- |
| `sound` | `phase: "start"` (default), explicit integer `delayCycles` and `repeatCount`; optional `selector`, `actionId`, `cueId`, and the positional fields below |
| `sound` preparation | `phase: "prepare"`, `actionId`; load this *needed* activity cue but do not enqueue/play it |
| `animation` | `phase: "start"` (default), `actionId`, optional nonnegative `iteration`; dispatch the original sequence's frame cues on their source-cycle sums |
| `animation` frame | `phase: "frame"`, exact `frame`, same `actionId`/`iteration`; the callback is **already at** frame entry, so it does not add the start-to-frame delay again |
| `animation` preparation/cancel | `phase: "prepare"` loads only that sequence's audible candidates; `phase: "cancel"` cancels queued/playing cues and its preparation |
| `jingle`, `level_up` | Explicit original group; optional unused integer `auxiliary`. Group `-1` is the native no-op sentinel, not cancellation |
| `quest_complete` | `questId: "quest.learning_the_ropes"` or `"quest.cooks_assistant"`; `sourceId: null` or `152`; must accompany legitimate committed completion |
| Cook reward `level_up` | Group `33`, `level: 4`, `causeQuestId: "quest.cooks_assistant"`; hold while that completion's quest scroll is open |
| `interface_closed` | `sourceId: 153`, `questId: "quest.cooks_assistant"`, `completionId` equal to the completion event ID; releases the deferred reward request |
| `music` | Explicit group and `mode: "area"` (default), `"single"`, `"playlist"`, or `"shuffle"`; details below |

`sourceCycle` is source-clock provenance, **not a 600 ms server tick** or a
guessed network latency. Game-triggered sounds must carry their actual received
delay. The source FIFO has capacity 50, retains existing entries on overflow,
and processes delay 2 as `1, 0, dispatch, remove`. The dispatched entry occupies
its slot until the following processing call. A short scheduler lookahead uses
`AudioBufferSourceNode.start(when)` to avoid JS timer/render-quantum drift.
Cold/missed timing and a stalled source clock report explicit failures.
Use preparation events ahead of the actual committed cue instead of pretending
a network/decode delay is source timing.

### Correlation and source bindings

The adapter must retain stable event IDs across duplicates/reconnects. For a
renderer callback and corresponding game notification, supply the same
`actionId`, sequence, frame and iteration, or an identical explicit `cueId`.
The canonical animation key is actor/action/sequence/frame/iteration.
Do not enqueue both with unrelated IDs. `sound` callbacks for a frame also
supply `sequenceId` and `frame`. Jingle callbacks can share the originating
`actionId`/`cueId` with semantic completion events.

The ledger is retained for the handle lifetime; it never silently evicts IDs
and replays history. Its 100,000-key bound fails explicitly. On a genuinely new
handle/browser session the app must establish the authoritative event floor
described in `spec/game-networking.md`, not resend historical transient events.
No audio settings, tokens or quest outcomes are written to browser storage.

| Source sequence / event | Original sound and frame/cycle |
| --- | --- |
| Net fishing 621 | 2603, frame 4 / cycle 16 |
| Bronze mining 625 | 3220, frame 11 / cycle 50 |
| Tinder 733 | 2597, frames 8 / 10, cycles 45 / 70 |
| Bronze chopping 879 | 2735, frame 3 / cycle 18 |
| Smithing 898 | 3790 / 3791 / 3790 / 3791, frames 5 / 7 / 9 / 11, cycles 20 / 41 / 62 / 83 |
| Eating 12526 | 2393, frame 1 / cycle 4, once |
| Sequence 13612 frame 1 | Original weights: silence 2411 **74**, 10983 **5**, 10984 **6**, 10985 **5**, 10986 **5**, 10987 **5** |

Silence 2411 remains an explicit 110-zero-sample source outcome and occupies
the native FIFO normally. It has no file/source node, requests no missing URL,
and is not a fallback. Weighted selection is made once; the adapter may supply
`weightRoll` 0–99 for an already-resolved source observation/replay. Otherwise
the presentation uses unbiased bounded randomness without changing the weights.

The optional qualified `selector` names are:

```text
shortbow_release -> 2693
rat_attack / rat_hit / rat_death -> 710 / 713 / 711
goblin_attack / goblin_hit / goblin_death -> 469 / 472 / 471
bronze_smelt_start -> 2725
```

These preserve `dated_public_source_observation`, not a claim of a current
build-240 server trace. They are game-triggered boundaries, not invented frame
sounds in 426, 493x, 618x or 899. Source sequences without frame sounds reject
animation-sound inference. Other explicit original packet cues remain playable
without assigning them to an unverified game action. UI 2266 additionally
requires `binding: "source_packet"`; there is no universal click sound.

**Both owner adaptations remain `approved_adaptation`:**

* Learning the Ropes submits 152 exactly once on committed completion.
* Ordinary shrimp **315** / bread **2309** require `itemId`, local actor,
  committed consumption and sequence **12526**. Its frame-1 sound starts after
  four source cycles. `sound` with `selector: "ordinary_food"` is an adapter
  alias for this same sequence path, not an extra cue. The asset's first
  nonzero sample is frame **9393**; that baked waveform onset is **not** another
  queue/trim delay. Silent 829 is not assigned an invented frame sound.

Cook keeps observed 152, then its reward-level 33 after widget-153 dismissal.
The audio layer does not create the level-up dialogue or grant rewards.
There is no quest-versus-skill ranking: **last accepted jingle request wins**,
including replacement of pending loads and active music. A `-1` request does
not erase its predecessor; auxiliary values have no priority. The remembered
playlist survives the jingle and is restarted, not a suspended MIDI playhead.

### Positions and active objects

`tile: null` selects the effects bus. Positioned cues use the area bus and
retain their source `range` and `retain`; source animation/object metadata
supplies those fields where available.

For nonlocal cues, supply `sourceGain` in [0,1] and `sourceDistance` in the same
units as the source range/retention fields. These must come from the bound
source mixer policy in the adapter; this module does **not** invent a Euclidean,
inverse-square, camera-based or stereo-panner attenuation curve. Local
same-tile actor cues use distance 0 / gain 1. Wrong plane/instance and distances
beyond range+retention are not admitted.

Ambient `sound` events additionally require `ambient: true`, `objectId`,
`active: true`, the current available authoritative entity, and the source
definition's actual sample loop. A cache definition alone never starts a sound.
The emitter key is `ambient/<actorId>/<sourceId>`. Further starts do not create
duplicate nodes. `phase: "update"` supplies its new source gain/distance;
`phase: "stop"` stops that exact emitter. Ordinary positional updates/stops
address their existing `cueId`. Inactive entities, region/instance changes and
out-of-retention emitters are disconnected.

On a moving listener/emitter, update the supplied spatial mix in that same world
update. Without it the stale node is stopped with explicit
`AUDIO_POLICY_UNBOUND` feedback, rather than silently retaining the wrong volume
or inventing a fade curve. Native object fade/visibility/change-tick fields stay
in the bound metadata; loading that metadata does **not** implement an unknown
live morph/visibility/fade policy.

## Music and the deliberately unguessed boundaries

Known journey scope selection is Scape Main **0** on title, Newbie **62** in
Tutorial surface squares 12336/12592, Scape Cave **144** in 12436, Harmony **76**
in Lumbridge 12850, and Autumn Voyage **2** in eastern Classic square 12851.
The approved pinned Autumn page gives its Classic 3200–3264 / 3264–3328
polygon. **12595 is a mill extraction seed, not an Autumn music boundary.**
Unknown squares require an explicit music event; they are not assigned the
nearest available song. These named scopes are not invented Modern-area
polygons.

An area selection plays one original pass, does **not** set `loop=true` on the
release-padded file, and does not restart on every snapshot. Its remembered
selection survives exhaustion; exhaustion explicitly requests the next source
selection instead of fabricating a Modern playlist.

Manual modes require `unlocked: true` from validated application state. A
playlist is a JSON-string array of at most 100 distinct, available original
groups; `"shuffle"`/`shuffle: true` uses a shuffle without replacement. Only
the current and next needed track load. Manual mode survives region changes.
Changing the remembered region during a jingle does not interrupt the jingle.

Repeating/advancing requests must explicitly include
`boundary: "native_midi_end"`. This is a **technical playback directive** to use
the manifest's integer native MIDI-end frame, not the end of its extra 22050
release samples. Single mode loops to that boundary; playlists schedule the next
source on the audio clock at it. It is not an assertion that a rendered
one-pass buffer reproduces every native MIDI voice carry-over/re-entry policy.
Without that explicit directive the runtime rejects an ambiguous repeat rather
than inferring a seamless loop.

### Exact remaining source-policy limitations

The corrected payloads, qualified selectors, owner adaptations and native
FIFO/jingle request rules are implemented; older pre-approval selector prose
does **not** revoke them. The following distinct numeric/live policies are not
established by the approved inputs and are not silently marked verified:

1. Full Modern-area polygons/membership/default-next selections for adjacent
   squares and songs beyond the five prepared music tracks.
2. Native session volume defaults/slider-to-mixer curves and calibrated
   cross-channel levels. `volume` is an explicit linear WebAudio **gain API**;
   unity is not a claim about the source UI's default thumb position.
3. Native positional attenuation/fade/visibility and active object-morph
   evaluation. The adapter must supply actual effective gain/distance/activity.
4. Native music fade/re-entry/release-voice behavior versus the explicitly
   requested one-pass MIDI-end replay. No source fade duration is guessed.

The original PCM/gain/queue tests and real browser output are operational
evidence, not closure of these remaining live-policy calibrations. No limiter
hides mix clipping. Zero extra clipping is checked for the measured scenarios,
not promised for 50 adversarial unity-gain overlapping sounds.

## Composition work remaining

The app owner must import `createAudio`, provide same-origin metadata/asset ID
routes, connect real trusted sound/mute/slider handlers, surface recoverable
permission/device/asset feedback, and feed authoritative events with the above
source/correlation fields. The renderer adapter supplies actual frame callbacks
or one source-cycle start stream, not both under unrelated IDs. Feed Cook's
accepted quest-scroll close; do not invent completion or dialogue in audio.
Music-mode UI requests need an app-to-`music` event adapter; the current shared
services expose unlock and volume, not a separate playlist method.

Use the real server's fresh event floor on login/reconnect and preserve its
stable event IDs. Then execute the legitimate journey through that composed
app. The isolated fixture cannot certify those gameplay/integration paths.
See [the browser test runner](../../tools/browser-audio-tests/README.md) and its
hash-bound `results.json`. No M1 acceptance, owner Mac/Edge result or host
speaker perception is claimed here.
