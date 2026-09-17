# Native browser-audio calibration

This is **new calibration evidence**, not a modification of the owner-approved
v1.3.0 pack (`b62e1970…674d`) or a claim that the old pack already measured these
policies. The unchanged injected **1.12.38**, game revision **240**, cache
**2695** and all tool JARs are hash checked. No external account, credentials,
personal RuneLite settings, audio hardware, source-file rewrite or patched
native method is used. Original cache access uses `BindingCache`'s read-only
handles. Archive transport, callback reporting and preference-save IO are
isolated; the executed policy/mixer/interpreter code is original.

## Reproduce

Read `AGENTS.md`, `prompt.md`, and `docs/machines/sparky.md` first. Existing JDK17,
Node24, TypeScript7 and Playwright1.63 are reused; no dependency installation is
needed on the verified host.

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

pnpm --dir web exec tsc --noEmit
node --test web/audio/audio.test.ts web/audio/native-policy.test.ts
node tools/browser-audio-tests/run.mjs
```

`probe.py` defaults to the already locked audio-owner tooling and prepared input
directory, and the existing source-owner private cache. `--reuse` and `--cache`
can supply equivalent **hash-verified** local inputs. Generated classes, JVM
home, work files, diagnostic PCM and disassemblies are under the owned ignored
`tools/browser-audio-tests/native/.run/`. Every Java process sets `-Duser.home`,
`-Djava.io.tmpdir`, `-XX:-UsePerfData`, a 1 GiB heap and two active processors.
The JVM owns and terminates its otherwise-idle native executors.

`public_maps.py` reuses the already pinned Newbie Melody and Scape Cave page
snapshots. Its one additional public input is **Map:Lumbridge music,
revision15258397, 2026-07-09**, before the baseline freeze. The cached response
is reused rather than repeatedly checking freshness. It is qualified public
geography, **not a native-server polygon capture**. Native table44 independently
corroborates the exact six-track membership and Harmony default-area marker.

## What was actually closed

### 1. Native preferences and channel gain

`cy()` constructs raw music/effects/area preferences of **127**, with master
**100%** and the legacy 8-bit option **false**. A freshly constructed `dm` is
zero until the real `em.fa` refresh runs. After that native refresh, effective
music/effect/area mixer levels are **255 / 127 / 127**. Calling the constructor
alone and claiming silence is the default would be wrong.

The native `dm` tables are retained exactly: 256 music entries and128
effect/area entries. The index is:

```text
round_float32((channelPercent / 100f) * (masterPercent / 100f) * maximum)
clamped to 0..maximum; maximum = music255, effects/area127
```

The table lookup follows this operation, so master50 is **not** a post-mix
half-gain: at channel100/master50 the native levels are music44, effects22,
area22. The original setter opcodes, getters and native `PcmPlayer`/device were
executed. Every tested source SFX stereo sample exactly equals
`(original_s16 * nativeVolume) >> 8`.

Relative to the existing musical volume128 renders and half-gain effect files,
the source-calibrated channel gain is `nativeVolume / 128`. Thus fresh effective
gains are **255/128** for music and **127/128** for effects/area, not unity.
The two unattenuated reference WAVs keep their existing per-voice half-gain.
Packet effects sample volume at dispatch; the native preference setter does
not rewrite their already-playing PCM stream. Browser-wide `mute()` remains
the explicit immediate output/privacy control.

### 2. Native spatial/object/morph behavior

`client.ib` was executed on real original SFX through its actual PCM mixer.
Packet distance is **Manhattan distance from the source tile center, minus128**,
clamped at0. Radius is `range*128`; source retain is an **inner full-volume
radius**, `max(0,((retain&31)-1)*128)`, not an extension of audible distance.
Outside/equal outer radius is dropped. The attenuated integer volume is the
native float32 fraction times the area mixer, **rounded up**.

Native `cl.av` / `iz.az` object distance is Manhattan distance to the actual
object rectangle, **minus64**, clamped at0. Object retention is `retain*128`.
All selected M1 definitions use source distance curve0. Source `dz.aj` plane
and owner-visibility checks and `rq.az` packet-owner checks are distinct and
retained, including source visibility IDs0/1/2 and parent/interior-plane rules.

The original object decoder supplies **120 definitions**, including the
transitive morph closure, their actual dimensions, source sound settings and
two required varbit definitions. Ten executed `om.dl` cases independently
verify varbit selection/fallback. For example, original object34815's base
sound3141 must **not** be played merely because it is present in metadata:
source varp491/bit2 selects29096 or29097, both without that sound.

Original `cr`/`dw`/`wc` emitters were executed. A new continuous loop begins at
its resolved native volume. Native configured fade changes use a **signed**
`current-target` duration scale. In this build, increases produce negative
duration and therefore apply immediately; do not substitute an aesthetic
300ms fade-in. Decreases use the original integer envelope (127→63 scales300ms
to151ms). Plane/eligibility loss uses150ms; ordinary range/despawn fade-out
uses the original setting. Random background sounds have a separate
source-cycle timer and do not consume the 50-entry packet FIFO; `it.al`
selects `[minimum, maximum)`.

### 3. Native music transitions and replay

Actual `wo`/`wp` music fades accumulate **float32** and send truncated integer
master levels each source cycle. They are not a generic continuous gain ramp.
Native delay tasks complete on processing call `delay+1`; float accumulation
can require an extra finishing call at some volumes.

Original `pd.br` requests title Scape Main at native255 with transition
`[outDelay0, outDuration0, inDelay0, inDuration100]`: **a two-second title fade-in**
and **no native MIDI loop flag**. The current native script9630's missing
transition arguments resolve to **[0,60,60,0]**. Native jingle requests still
replace current work with zero fades, retain the background playlist, and
overwrite the global remembered transition values with **[0,0,0,0]**. Resume
reinitializes the remembered track; it does not preserve an old MIDI playhead.

The bounded preference continuation additionally executes the later-request
case: a different background request during that jingle replaces the remembered
track and transition parameters; a same-group request is ignored. See
[`preference-controls.md`](preference-controls.md) for the12 native state
fixtures, first-use/remembered volume distinctions, three original saved slots
and actual-browser control contract.

The client-side source does not choose arbitrary next songs on buffer EOF:
the native reference census shows the last-track, area override and fresh-login
preference effects crossing the server/script boundary. The module provides an
explicit `SourceMusicSelector` bridge instead of inventing three songs or
silently treating an exhausted pass as successful ongoing playback. After the
additive publication below, this bridge is **optional**: the factory's default
continuation uses the exact known area membership and native duration rules.
`SourceMusicState` supplies actual mode/unlocks/loop preferences, not a guessed
next-song function.

Native table44 durations are retained separately from exact MIDI EOT frames.
Explicit browser single/custom-playlist replays use those600ms duration units
and a fresh one-pass source; they never set a seamless loop over release
padding. This timer projection is disclosed separately from observed server
emission timing. A real authoritative music event and its supplied fade
arguments remain the stronger input.

### 4. Modes, M1 geography and exact track identities

Native script318 labels the actual mode IDs: **area0, shuffle1, single2**.
The already pinned February2026 official policy says login returns to Area
unless remembering the prior mode is enabled. Native source varbits19734,
19735,19736 and19737 preserve the relevant source preferences; they are not
invented settings.

The previous tiny seed-square approximation is replaced by the actual pinned
Tutorial polygons, the Tutorial cave rectangle and the dated291-point
Lumbridge polygon. The native music table independently identifies area1:

| Original row | Track | Index6 group | Source duration units |
| --- | --- | ---: | ---: |
| 2549 | Autumn Voyage | 2 | 229 |
| 2583 | Book of Spells | **64** | 546 |
| 2674 | Dream | **327** | 257 |
| 2721 | Flute Salad | **163** | 194 |
| 2777 | Harmony, area-default marker1 | 76 | 362 |
| 3237 | Yesteryear | **145** | 397 |

Native rows2938/3012 identify Newbie62 and Scape Cave144. Classic's eastern
Autumn square remains distinct from Modern-area membership. The polygon is
qualified public evidence; the client does not contain a native server
selector program defining all polygon-boundary/plane decisions. The adapter
must retain actual music events and source preferences rather than relabel
public geometry as observed server behavior.

## Publication requirements identified and now fulfilled

The native probes identified these precise requirements. Authority commit
`58a40d1194aa5806f2fa8919bfe2877aaa7d9721` then authorized the strictly additive
publication below, without changing any frozen file, manifest or tolerance:

1. **Four formerly unpublished original music tracks:** index6 groups
   **64,327,163,145**, identified above by original row/membership/CRC evidence.
   The old264-file manifest remains unchanged. The four original native255
   tracks are now published under distinct `audio-supplement.music.*.native255`
   identities; a jingle with the same numeric ID is not that music asset.
2. **Five fixed-render limitations at the native full mixer:** index11 jingles
   **40,54,58,64,65**. All35 musical inputs were independently rendered through
   the original player at native128 and255. The128 controls exactly reproduce
   the frozen PCM hashes. The255 controls show that scaling a baked128 render
   is not bit-identical to the native note/mixer calculation. RMS gain error is
   within the existing0.25dB bound (maximum0.146646884dB), but those five inputs
   add **1,8,2,11,2** clipped samples respectively. Each now has the **exact
   previously predicted original native255 PCM**, with the locked
   hash/frame/saturation count, in a distinct additive representation. The
   runtime selects it when the old128 representation cannot safely express
   the requested level. A limiter, normalization, lower default or rewritten
   frozen file is **not** substituted.
3. **Typed app inputs, not a request to compute unknown gain:** renderer
   listener coordinates/placed source objects, authoritative source varps for
   morphs, and native music selections/source preferences. Concrete interfaces
   and helpers are in `web/audio/native-scene.ts` and `web/audio/README.md`.

### Additive source pipeline and actual factory consumption

`assets/manifests/osrs/audio-m1-supplement.json` pins the unchanged base manifest,
all nine new IDs/files, original levels, source track and PCM/container hashes,
integer MIDI EOT plus1s release, native saturation and provenance:

```text
manifest SHA256 840aef91bac9a1fd042bdb1c3662ff92a378e279f48335108730e168af550d91
asset prefix    asset.source.osrs.cache2695.audio-supplement.
music suffixes  music.64.native255, music.327.native255,
                music.163.native255, music.145.native255
jingle suffixes jingle.40.native255, jingle.54.native255,
                jingle.58.native255, jingle.64.native255, jingle.65.native255
```

Every new track is generated by the unchanged original runtime using its
current patches/samples/MIDI and device scheduling, including channel9/bank128
initialization. `AudioSupplement.java` adapts only read-only archive transport;
it records source container/file/instrument/sample hashes and PCM relationships.
Two independent complete runs agree exactly for every new output and input
record. Native255 jingle40/54/58/64/65 hashes match the **prepublication**
`publication-needs.json` expectations; those expectations were not fitted to
new output.

The native16 PCM is stored unchanged in a **unity-gain FLAC24 container**. This
reversible bit-container expansion avoids Chrome's asymmetric positive-peak
FLAC16 float conversion at saturated native samples. It is not attenuation,
gain patching, normalization or a changed waveform. The browser's signed32
power-of-two decoder path then exactly matches every independently computed
channel hash; original native saturation stays at the same sample positions.

Eighteen separate native44/136 controls compare the slider path of all nine
representations. Maximum RMS gain error is0.231189882dB, within the already
frozen0.25dB bound, with zero additional clips. Pointwise mixed-voice differences
are separately reported, not mislabeled bit-identical synthesis. Full-level
published PCM/file decode is exact. Source artifacts, checksummed native
libraries and all275 file identities are verified by:

```sh
python3 tools/browser-audio-tests/native/audio_supplement.py publish
python3 tools/browser-audio-tests/native/audio_supplement.py verify
python3 tools/browser-audio-tests/native/supplement_oracles.py
```

No whole cache or game pack is fetched. No old audio file or encoder/source tool
is modified. The production factory hashes the small additive manifest before
consumption and retains all old128 asset IDs. It loads only current/next needed
tracks and active cues. A live128→255 representation switch preserves original
offset/end time, has one connected source path, and retains the configured
native volume without amplifying the unsafe old representation.

The internal Modern area controller now has the **actual six tracks** and
continues without a required caller selector. Original single/custom playlists
restart/advance on their declared source timers; pending area choices survive
interruptions, and repeating identical source state does not restart music.
The optional `SourceMusicSelector` remains available only for stronger
authoritative decisions/overrides, not as a missing-default workaround.

`native-pcm.json` deliberately reports intrinsic native clipping separately
from extra clipping. For example, native jingle33 at255 has12 saturated samples;
the unchanged128 render scaled to the source gain has8 at a subset of those
positions. Claiming absolute zero source clipping would be false.

`bounds.json` records the unchanged comparison bounds before evaluating the
updated browser. No host-speaker, live-game, Mac/Edge or M1 acceptance is claimed.
