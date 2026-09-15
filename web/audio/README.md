# Original browser audio: native-calibrated adapter

`createAudio(assets, report): Promise<AudioHandle>` still implements the shared
`CreateAudio` ABI. The running-browser component from8770111 remains the
playback engine; this revision binds independently executed original native
policies into it. It does not implement UI or gameplay authority.

The source pack stays
`b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d`.
All264 original FLACs and the original710/2693 reference WAVs are unchanged.
New calibration is under `research/browser-audio-policy/`, **outside** the
frozen pack. See that directory's README for actual native probes, source
hashes, operation-by-operation findings and exact publication needs.

## Composition and migration from8770111

```ts
import {
  createAudio, observeAudioState, setSourceMasterVolume,
  setSourceAudioScene, setSourceMusicSelector,
} from "./audio/index.ts";

const audio = await createAudio(assets, reportAudioFeedback);
const unsubscribe = observeAudioState(audio, showActualAudioStatus);

soundButton.addEventListener("click", () => {
  audio.mute(false);
  void audio.unlock().catch(reportAudioFeedback);
});

audio.update(worldView, authoritativeAudioEvents);
audio.volume("music", 0.5);   // SOURCE slider50%, not linear half-amplitude
audio.volume("effects", 1);  // SOURCE slider100%
audio.volume("area", 1);
setSourceMasterVolume(audio, 100); // original integer percentage

setSourceAudioScene(audio, sourceSceneProjection);
setSourceMusicSelector(audio, resolveAuthoritativeNextMusic, "modern");

// Transport loss is not a title screen:
audio.disconnected();
// After a validated reconnect:
audio.update(newWorld, newEvents);

unsubscribe();
await audio.dispose();
```

The UI/reporting variables in this example belong to app composition.
`update(null, [])` is an explicit title/logout reset and selects Scape Main0;
do not send it merely because transport is reconnecting.

**Important migration changes:**

* `volume(channel, normalizedPosition)` now maps the normalized UI position to
  the native integer percentage and nonlinear lookup, not the old provisional
  linear gain. `sourceSliderToMixer`, `sourceMixerToAssetGain` and
  `sourceAudioDefaults` expose the exact conversion.
* Positioned audio no longer requires caller-invented `sourceGain` or a guessed
  distance/fade curve. Feed source coordinates and original parameters; use the
  typed `SourceAudioScene` for renderer/static-object data and exact varps.
  An explicit legacy `sourceGain` remains an override, not native calibration.
  Remove legacy manually computed `sourceDistance`/range-extension logic.
* Source retain is an **inner full-volume radius**, not audible range extension.
* Music is not a seamless loop over the exported one-second release padding.
  The old experimental `boundary:"native_midi_end"` is rejected. Explicit
  single/custom-playlist timer replay uses `boundary:"native_duration"` or omits
  it; the declared native duration and exact PCM EOT are separate quantities.
* Music mode IDs are native **area0 / shuffle1 / single2**. Modern Lumbridge
  contains six original tracks, not just the two already published for it.
  Exact required missing inputs are listed below.

## Actual graph, loading, permission and errors

```text
original AudioBufferSourceNode -> source-calibrated voice gain
  -> music / effects / area routing buses -> explicit mute gain
  -> actual AudioDestinationNode
```

The context renders at22050Hz. Original stereo music and mono effects remain
unchanged. The two original WAV templates `reference.audio.sfx.2693` and
`reference.audio.sfx.710` receive the exact half-gain already baked into FLAC
effects; neither has a fabricated FLAC alias.

`AUDIO_INPUTS` exposes three original metadata path IDs and checksums.
`ClientAssets.url(id)` must resolve them and the matching playable asset IDs
through same-origin HTTP(S). Bytes, lengths, hashes, decoded dimensions,
finite PCM and decoder peak bounds are checked. Metadata is deliberately
fetched as bytes, not through `ClientAssets.json`. Cross-origin URLs, redirects,
missing/corrupt/oversized data and decoder/device errors are explicit failures.
Only metadata loads before a gesture; selected/current-next music and needed
activity/emitter files load progressively through four load slots and a96MiB
decoded LRU. No whole-game download, SoundFont, oscillator, limiter or
normalization substitutes for original input.

Factory resolution is not autoplay permission. `unlock()` requires transient
real user activation, invokes native `resume()` before network awaits, and
checks actual running state with a timeout. `AUDIO_GESTURE_REQUIRED` is
recoverable sound-control feedback, not a fatal renderer/game error.
`readAudioState`/`observeAudioState` expose actual time/state, native mixer
levels, gain/fade state, source/cache accounting and traces; reading never
advances the engine. A running clock routed to `{type:"none"}` is explicitly
not enabled playback.

Native ordinary SFX sample mixer level at dispatch: changing their preference
does not rewrite an already-playing source. Native object ambience continuously
updates its separate envelope. `mute(true)` is the immediate browser-wide
output/privacy control and clears transient work. Context suspension does not
accumulate stale SFX; a genuine gesture resumes existing musical nodes without
duplicates. Disposal stops clocks, aborts owned loads, disconnects gains/sources
and closes the context. Asset failure never retries a large music file on every
unchanged snapshot; explicit control/selection retry is required.

## Native helper APIs and authoritative input

All exports below are additional to the unchanged `AudioHandle` ABI.

| Export | Input/output |
| --- | --- |
| `sourceAudioDefaults()` | source percentages, raw127 constructor preferences, effective255/127/127 mixer levels, gains255/128 and127/128 |
| `sourceSliderToMixer(channel, percent, masterPercent=100)` | exact native float32 index/rounding and nonlinear lookup |
| `sourceMixerToAssetGain(nativeVolume)` | calibrated `nativeVolume/128`, accounting for the original musical render and half-gain effects |
| `setSourceMasterVolume(handle, percent)` | native master applied **before** each channel lookup |
| `sourcePacketSpatial(listener, emitter, range, retain, areaVolume)` | source packet Manhattan-minus128 distance, packed inner retention, float32/ceil integer volume |
| `sourceObjectBounds(tile, sizeX, sizeY, orientation)` | original rotated object footprint in128-unit coordinates |
| `sourceAmbientSpatial(listener, bounds, range, retain, areaVolume)` | original rectangle Manhattan-minus64 distance and source curve0 volume |
| `sourcePacketOwnerVisible(...)`, `sourceAmbientVisible(...)` | separate original packet and object plane/world-owner rules |
| `sourceAmbientFadeDuration`, `sourceAmbientFadeVolume` | original signed duration scaling/integer envelope, including immediate native upward changes |
| `sourceMusicFade(volume, cycles, direction)` | original float32-per-cycle/truncated master levels |
| `sourceObjectDefinition`, `resolveSourceObject(id, varps)` | original120-definition M1 closure and native varbit/varp morph selection; missing required vars fail, never default silently |
| `sourceMusicRegion(tile, areaMode)` | source-qualified detailed M1 polygons and actual native table44 membership |
| `sourceMusicDurationSeconds(group)` | original duration field in600ms units, **not** exact MIDI EOT or claimed server emission time |
| `setSourceAudioScene(handle, scene)` | consume the typed renderer/bridge projection and maintain actual original object streams |
| `setSourceMusicSelector(handle, selector, areaMode)` | request the actual next selection rather than inventing a default playlist or silent exhausted success |

The exact additional bridge input is exported as:

```ts
interface SourceAudioScene {
  listener: { x: number; y: number }; // original128-unit audio listener point
  plane: number;
  instance: string | null;            // authoritative game-instance identity
  owner: SourceWorldOwner | null;     // null for ordinary main-world M1 scenes
  varps: ReadonlyMap<number, number>;  // exact original values, not quest-name guesses
  emitters: readonly {
    id: string; objectId: number; tile: Tile; orientation: number;
    instance: string | null; owner: SourceWorldOwner | null; present: boolean;
  }[];
}
interface SourceWorldOwner {
  id: string;
  exteriorPlane: number;
  audibleInteriorPlane: number;
}
type SourceMusicSelector = (request: {
  previousGroup: number;
  mode: "area" | "single" | "playlist";
  region: SourceMusicRegion | null;
  durationTicks: number | null;
}) => Promise<{ group: number; transition?: SourceMusicTransition }>;
```

The renderer supplies placed audible scenery, including non-interactable
objects absent from a gameplay-entity list. Original object definitions,
rotation, morph vars, owner visibility and listener position determine the
sound; no guessed `sourceGain` is requested from this adapter. Scene removal,
inactive morphs, wrong planes/instances, range changes and stale loads do not
leak or duplicate nodes. Native random background streams use their original
`[minCycles,maxCycles)` interval and do not occupy the packet FIFO.

For music, preserve actual source selections/preferences when available.
`SourceMusicTransition` contains the four integer fields
`fadeOutDelayCycles`, `fadeOutCycles`, `fadeInDelayCycles`, `fadeInCycles`.
The native defaults are title **[0,0,0,100]**, background script9630
**[0,60,60,0]**, and jingle/resume **[0,0,0,0]**. The selector input is a concrete
bridge requirement, not permission to grant unlocked tracks or invent source
server polygon/timer decisions. A missing selector is reported explicitly at
pass completion rather than hidden as successful ongoing audio.

## Shared AudioEvent payloads

The existing shared envelope remains unchanged. `sourceId` is an index4 group
for `sound`, index6 group for `music`, index11 group for jingle/level-up, a
sequence for `animation`, and widget153 for the supported quest-scroll close.
Use `assetId:null` or the **matching exact** catalog ID.

All gameplay audio requires `committed:true` from the validated adapter, not
an optimistic UI intent. Quest audio additionally requires its completed
authoritative quest entry. Initial snapshots seed completion history without
replaying old outcomes.

* `sound`: explicit `repeatCount` and `delayCycles`; optional qualified
  `selector`, correlation IDs and source position. `phase:"prepare"` loads a
  needed activity cue only. Original packet delay2 dispatches on processing
  call3; capacity50 includes a dispatched entry until the next cycle, and
  overflow drops only the new request. The original `-10`-cycle loading grace
  expires without late playback. Instrument offsets/leading trim are **not
  added again** to the full original waveform.
* `animation`: `phase:"start"` schedules actual source frame/cycle sums;
  `phase:"frame"` plus `frame` is already at that source boundary.
  `phase:"prepare"` loads just needed candidates; `phase:"cancel"` removes
  that activity's pending/playing cues. Use common `actionId`, `iteration`
  and/or `cueId` across server and renderer callbacks.
* `jingle` / `level_up`: explicit original group, optional **unused integer**
  `auxiliary`. `-1` is a no-op sentinel. Last accepted request wins, with no
  quest-versus-skill ranking or priority field.
* `quest_complete`: `questId` Learning the Ropes or Cook's Assistant, group
  `null`/152, legitimately committed. A Cook reward-caused `level_up` supplies
  `causeQuestId:"quest.cooks_assistant"`, `skillId` and its **source-selected**
  jingle group. The actual skill/base-level/XP delta is captured from the
  immutable before/after completion snapshots. Optional `previousLevel`,
  `level` and `completionId` must agree with that transaction. Any real
  increase—including pre-trained or multi-level gains—is deferred until
  `interface_closed`, source153, matching `questId` and `completionId`.
  A reward with no base-level increase produces **no level-up cue**.
* `music`: explicit source group and mode; manual selection requires
  `unlocked:true`, playlists an explicit JSON-string array of at most100
  distinct available source groups. Optional source transition fields above.

Native frame cues remain621→2603 at4/16cycles;625→3220 at11/50;
733→2597 at8/45 and10/70;879→2735 at3/18;898→3790/3791/3790/3791
at5/20,7/41,9/62,11/83. Source silence2411 remains the **74/100** branch of
13612/frame1, occupies the native FIFO, and has no file or replacement node.
The audible alternatives retain weights5,6,5,5,5.

Qualified selectors remain shortbow release2693; rat710/713/711;
goblin469/472/471; bronze smelt-start2725. These are dated public observations,
not invented frame sounds in426/493x/618x/899. Other explicit original packet
IDs do not create unverified action bindings. UI2266 requires its explicit
`binding:"source_packet"`; it is not sounded for every click.

Both owner choices stay **`approved_adaptation`**: Learning completion152 once;
ordinary shrimp315/bread2309 eating2393 once through12526/frame1 after four
source cycles. The2393 waveform's first nonzero sample9393 is baked content,
not another enqueue delay. Silent829 is not assigned a fabricated cue.
Audio never grants XP/items, advances quests or manufactures reward dialogue.
In particular, the dated Cook observation of33/level4 is a golden example,
not a universal reward condition or selector. The later observed34/level5 is
still qualified as a separate ordinary Cooking level-up, not newly asserted
quest-selector evidence. The gate preserves the supplied group; it never
chooses33/34 from a level. See
`research/browser-audio-policy/cook-reward-boundary.md`.

Feed coherent committed world/event batches so the completion's before/after
skills remain observable. Within that batch, an XP/level notification may
precede its supplied semantic completion event; the audio gate stages that
existing event without generating another quest completion. It retains the
original committed delta until dismissal even if later snapshots raise a
skill further, and deduplicates callback/reconnect repeats.

Stable event/cue IDs survive repeated snapshots and reconnects for the handle
lifetime. The ledger never evicts IDs and replays history; its100,000-key limit
fails explicitly. A fresh handle requires the server's new event floor from
`spec/game-networking.md`. No tokens, credentials or outcomes are persisted
by the audio module.

## Exact remaining input requirements, not the old policy placeholders

1. Modern Lumbridge's native table44 area1 has **2,64,327,163,76,145**, with
   Harmony76 marked area-default. Original music **64,327,163,145** still lacks
   approved playable publication. `AUDIO_SOURCE_MUSIC_ASSET_REQUIRED` names
   the exact index/group; no nonexistent FLAC alias or jingle substitution.
2. All35 musical inputs were rendered by the original player at128 and255.
   RMS gain differences of scaled frozen inputs stay within0.25dB, but source
   jingles **40,54,58,64,65** can introduce respectively **1,8,2,11,2** extra
   clipped samples at full native mixer. Those unsafe gain cases fail as
   `AUDIO_NATIVE_GAIN_INPUT_REQUIRED`; provide a source-native representation/
   synthesis path, not a limiter, normalization or changed frozen asset.
3. Wire the concrete source-scene and next-music bridge inputs above. Source
   native/public calibration does not substitute for authoritative scene vars,
   unlock state or server music selections. Public polygons retain their
   qualification instead of pretending to be native-server captures.

The old constructor/default, generic attenuation, arbitrary fade and missing
membership placeholders are now backed by executed original policies and
qualified source maps. The remaining missing publications/bridge inputs are
precise. Independent tests live in `tools/browser-audio-tests`; none claims
gameplay, owner Mac/Edge, host-speaker perception or M1 acceptance.
