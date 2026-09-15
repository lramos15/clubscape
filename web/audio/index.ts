import type {
  AudioEvent, AudioHandle, ClientAssets, CreateAudio, Tile, WorldView,
} from "../shared/contracts.ts";
import { BufferCache, repeatedEffect } from "./buffers.ts";
import { actionId, COOK, cueKey, LEARNING, OBSERVED_SELECTORS, stableId, validateEvent } from "./events.ts";
import { AudioFailure, failure, integer, requireAudio, unit } from "./errors.ts";
import { EventLedger, SourceQueue } from "./queue.ts";
import {
  APPROVED_PACK, loadCatalog, regionalTrack, selectWeighted, SOURCE_CYCLE_SECONDS,
  SOURCE_RATE, sourceRandomBelow, sourceRoll, validTile,
} from "./source.ts";
import type { AssetKind, FrameCue, SourceAsset, SourceCatalog } from "./source.ts";
import {
  sourceAudioDefaults, sourceMixerToAssetGain, sourceSliderToMixer, sourcePacketSpatial,
  sourceAmbientSpatial, sourceObjectBounds, sourceAmbientVisible, sourceAmbientFadeDuration,
  sourceAmbientFadeVolume, sourceMusicFade,
} from "./native-policy.ts";
import {
  sourceObjectDefinition, resolveSourceObject, sourceMusicRegion, sourceMusicDurationSeconds,
  SOURCE_BACKGROUND_TRANSITION, SOURCE_TITLE_TRANSITION, SOURCE_JINGLE_TRANSITION,
  SOURCE_MIX_REPRESENTATION_NEEDS,
  SOURCE_UNPUBLISHED_M1_MUSIC,
} from "./native-scene.ts";
import type { SourceAudioScene, SourceMusicTransition, SourceMusicSelector } from "./native-scene.ts";
import { baseSkills, committedRewardLevel, observeRewardLevels } from "./reward-levels.ts";
import type { BaseSkills, RewardLevels } from "./reward-levels.ts";

export * from "./native-policy.ts";
export * from "./native-scene.ts";

export { AudioFailure } from "./errors.ts";
export { AUDIO_INPUTS } from "./source.ts";

type Channel = "music" | "effects" | "area";
type TraceValue = string | number | boolean | null;
export interface AudioTrace {
  readonly type: string;
  readonly audioTime: number;
  readonly wallTime: number;
  readonly data: Readonly<Record<string, TraceValue>>;
}
export interface AudioSnapshot {
  readonly sourcePackSha256: string;
  readonly contextState: string;
  readonly currentTime: number;
  readonly sampleRate: number;
  readonly pendingGesture: boolean;
  readonly unlocked: boolean;
  readonly muted: boolean;
  readonly connected: boolean;
  readonly disposed: boolean;
  readonly outputEnabled: boolean;
  readonly volumes: Readonly<Record<Channel, number>>;
  readonly nativeMixer: Readonly<Record<Channel, number>>;
  readonly masterPercent: number;
  readonly queueSize: number;
  readonly background: Readonly<{ groups: readonly number[]; cursor: number; mode: string; exhausted: boolean; failed: boolean }>;
  readonly voices: readonly Readonly<{
    id: number; sourceId: number; kind: AssetKind; channel: Channel;
    eventId: string; when: number; gain: number; loop: boolean; loopEnd: number;
  }>[];
  readonly cache: Readonly<{ decodedBytes: number; cached: number; pending: number }>;
  readonly policyLimits: readonly string[];
  readonly traces: readonly AudioTrace[];
}

interface Listener {
  id: string; region: string; tile: Tile; instance: string | null;
  entities: Map<string, { tile: Tile; instance: string | null; available: boolean; sourceId: number | null }>;
}
interface Spatial {
  tile: Tile;
  actorId: string | null;
  instance: string | null;
  range: number;
  retain: number;
  distance: number;
  listenerStamp: string;
  nativeComputed: boolean;
  ambient: boolean;
  objectId: number | null;
  orientation: number;
}
interface Effect {
  eventId: string;
  actionId: string;
  key: string;
  asset: SourceAsset;
  channel: "effects" | "area";
  gain: number;
  repeats: number;
  ambient: boolean;
  ambientRandom: boolean;
  spatial: Spatial | null;
  buffer: AudioBuffer | null;
  error: AudioFailure | null;
  lateReported: boolean;
  requestedAt: number;
  dueAt: number;
  enqueuedCycle: number;
  epoch: number;
}
interface Voice {
  id: number;
  asset: SourceAsset;
  channel: Channel;
  source: AudioBufferSourceNode;
  gain: GainNode;
  eventId: string;
  actionId: string;
  key: string;
  when: number;
  level: number;
  spatial: Spatial | null;
  ambient: boolean;
  ambientRandom: boolean;
  musicToken: number;
  stopped: boolean;
  calibrationGain: number;
  ambientFade: { from: number; target: number; start: number; durationMs: number; stop: boolean } | null;
  retiring: boolean;
  musicFader: { direction: "in" | "out"; cycles: number; current: number; nextAt: number; retire: boolean } | null;
}
interface MusicPlan {
  groups: readonly number[];
  cursor: number;
  mode: "once" | "single" | "playlist";
  regionBound: boolean;
  transition?: SourceMusicTransition;
  scopeGroups?: readonly number[];
}

const runtimes = new WeakMap<AudioHandle, Runtime>();
const MAX_EVENT_BATCH = 512;

function stamp(listener: Listener): string {
  const t = listener.tile;
  return `${listener.instance ?? ""}:${t.plane}:${t.x}:${t.y}`;
}
function sameTile(a: Tile, b: Tile): boolean {
  return a.x === b.x && a.y === b.y && a.plane === b.plane;
}
function decimal(value: unknown): value is string {
  return typeof value === "string" && /^(0|[1-9][0-9]{0,19})$/.test(value);
}
function deadline<T>(promise: Promise<T>, milliseconds: number, error: AudioFailure): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([
    promise,
    new Promise<never>((_, reject) => { timer = setTimeout(() => reject(error), milliseconds); }),
  ]).finally(() => clearTimeout(timer!));
}

class Runtime implements AudioHandle {
  private readonly context: AudioContext;
  private readonly catalog: SourceCatalog;
  private readonly report: (error: Error) => void;
  private readonly cache: BufferCache;
  private readonly master: GainNode;
  private readonly buses: Record<Channel, GainNode>;
  private readonly voices = new Map<number, Voice>();
  private readonly queue = new SourceQueue<Effect>();
  private readonly ambientPending = new Map<string, Effect>();
  private readonly traces: AudioTrace[] = [];
  private readonly policyLimits = new Set<string>();
  private readonly observers = new Set<(state: AudioSnapshot) => void>();
  private readonly volumes: Record<Channel, number> = { music: 1, effects: 1, area: 1 };
  private readonly nativeMixer: Record<Channel, number> = { ...sourceAudioDefaults().mixer };
  private masterPercent = 100;
  private sourceScene: SourceAudioScene | null = null;
  private sceneSequence = 0;
  private readonly sourceSceneErrors = new Set<string>();
  private readonly ambientIntervals = new Map<string, number>();
  private musicSelector: SourceMusicSelector | null = null;
  private sourceRequestDue: { plan: MusicPlan; group: number; token: number; at: number } | null = null;
  private nativeAreaMode: "modern" | "classic" = "modern";
  private musicAreaName: string | null = null;
  private listener: Listener | null = null;
  private playerId: string | null = null;
  private revision: bigint | null = null;
  private ledger = new EventLedger();
  private completed = new Set<string>();
  private baseline = true;
  private questScroll: string | null = null;
  private readonly deferredLevels: AudioEvent[] = [];
  private previousSkills: BaseSkills = new Map();
  private currentSkills: BaseSkills = new Map();
  private cookRewardLevels: RewardLevels | null = null;
  private pendingCookCompletionId: string | null = null;
  private readonly prepared = new Map<string, Set<string>>();
  private music: MusicPlan = { groups: [0], cursor: 0, mode: "once", regionBound: true };
  private musicToken = 0;
  private musicExhausted = false;
  private musicFailed = false;
  private musicPending: Promise<void> | null = null;
  private backgroundPreparation: { id: string } | null = null;
  private jingle: SourceAsset | null = null;
  private jinglePending: Promise<void> | null = null;
  private musicalToken = 0;
  private epoch = 0;
  private voiceId = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  private nextCycleAt = 0;
  private processingCycle = 0;
  private hasUnlocked = false;
  private pendingGesture = true;
  private muted = false;
  private connected = true;
  private disposed = false;
  private outputDisabled = false;
  private unlocking: Promise<void> | null = null;

  constructor(context: AudioContext, catalog: SourceCatalog, assets: ClientAssets, report: (error: Error) => void) {
    this.context = context;
    this.catalog = catalog;
    this.report = report;
    this.master = context.createGain();
    this.master.connect(context.destination);
    this.buses = {
      music: context.createGain(), effects: context.createGain(), area: context.createGain(),
    };
    for (const bus of Object.values(this.buses)) bus.connect(this.master);
    this.cache = new BufferCache(assets, context, (notice) => {
      this.trace(notice.type, { ...notice });
      this.publish();
    });
    context.addEventListener("statechange", this.stateChanged);
    context.addEventListener("sinkchange", this.sinkChanged);
    this.trace("context_created", { state: context.state, destinationChannels: context.destination.channelCount });
    this.notifyError(new AudioFailure("AUDIO_GESTURE_REQUIRED",
      "Sound is waiting for a real mouse or keyboard gesture. AudioContext has not been unlocked.",
      true, { state: context.state }));
  }

  snapshot(): AudioSnapshot {
    return Object.freeze({
      sourcePackSha256: APPROVED_PACK, contextState: this.context.state,
      currentTime: this.context.currentTime, sampleRate: this.context.sampleRate,
      pendingGesture: this.pendingGesture, unlocked: this.hasUnlocked, muted: this.muted,
      connected: this.connected, disposed: this.disposed,
      outputEnabled: !this.disposed && !this.outputDisabled && this.context.state !== "closed",
      volumes: Object.freeze({ ...this.volumes }), queueSize: this.queue.size,
      nativeMixer: Object.freeze({ ...this.nativeMixer }), masterPercent: this.masterPercent,
      background: Object.freeze({
        groups: Object.freeze([...this.music.groups]), cursor: this.music.cursor,
        mode: this.music.mode, exhausted: this.musicExhausted, failed: this.musicFailed,
      }),
      voices: Object.freeze(Array.from(this.voices.values(), (voice) => Object.freeze({
        id: voice.id, sourceId: voice.asset.sourceId, kind: voice.asset.kind,
        channel: voice.channel, eventId: voice.eventId, when: voice.when,
        gain: voice.gain.gain.value,
        loop: voice.source.loop, loopEnd: voice.source.loopEnd,
      }))),
      cache: Object.freeze(this.cache.state), policyLimits: Object.freeze([...this.policyLimits]),
      traces: Object.freeze([...this.traces]),
    });
  }

  subscribe(observer: (state: AudioSnapshot) => void): () => void {
    this.observers.add(observer);
    observer(this.snapshot());
    return () => { this.observers.delete(observer); };
  }

  async unlock(): Promise<void> {
    this.requireOpen();
    requireAudio(!this.outputDisabled, "AUDIO_OUTPUT_DISABLED",
      "AudioContext is routed to a silent output sink; restore an enabled device before unlocking sound.");
    requireAudio(navigator.userActivation?.isActive === true, "AUDIO_GESTURE_REQUIRED",
      "Call unlock() directly from a trusted mouse or keyboard event, not a synthetic event or loading callback.");
    if (this.unlocking) return this.unlocking;
    // resume() is invoked in this call stack, BEFORE awaiting network or decoding.
    const resume = this.context.resume();
    this.unlocking = (async () => {
      try {
        await deadline(resume, 2500, new AudioFailure("AUDIO_RESUME_TIMEOUT",
          "The browser did not grant running audio. Try the sound control again."));
        this.requireOpen();
        requireAudio(this.context.state === "running", "AUDIO_CONTEXT_SUSPENDED",
          `AudioContext remains ${this.context.state}; sound is not ready.`);
        this.hasUnlocked = true;
        this.pendingGesture = false;
        this.musicFailed = false;
        this.backgroundPreparation = null;
        this.trace("unlocked", { state: this.context.state });
        this.startClock();
        await this.ensureMusic();
      } catch (error) {
        this.pendingGesture = this.context.state !== "running";
        const problem = failure(error, "AUDIO_UNLOCK", "Cannot enable audio");
        this.notifyError(problem);
        throw problem;
      } finally {
        this.unlocking = null;
        this.publish();
      }
    })();
    return this.unlocking;
  }

  update(world: WorldView | null, inputs: readonly AudioEvent[]): void {
    if (this.disposed) return;
    this.pendingCookCompletionId = null;
    try {
      requireAudio(Array.isArray(inputs) && inputs.length <= MAX_EVENT_BATCH,
        "AUDIO_EVENT", "Audio event batch is not a bounded array.");
      const previous = this.listener;
      if (world === null) {
        if (previous !== null || !this.connected) {
          this.resetPlaying();
          this.listener = null;
          this.revision = null;
          this.baseline = true;
          this.questScroll = null;
          this.deferredLevels.length = 0;
          this.previousSkills = new Map();
          this.currentSkills = new Map();
          this.cookRewardLevels = null;
          this.music = { groups: [0], cursor: 0, mode: "once", regionBound: true };
        }
      } else {
        requireAudio(decimal(world.revision) && decimal(world.tick) && stableId(world.player?.id) &&
          stableId(world.player.region) && validTile(world.player.tile) &&
          (world.player.instance === null || stableId(world.player.instance)) &&
          Array.isArray(world.entities) && world.entities.length <= 10_000 &&
          Array.isArray(world.player.quests),
        "AUDIO_WORLD", "Audio requires a valid immutable authoritative world view.");
        requireAudio(world.player.quests.every((quest) => quest && stableId(quest.id) && typeof quest.completed === "boolean"),
          "AUDIO_WORLD", "Invalid authoritative quest completion data.");
        const skills = baseSkills(world.player.skills);
        const revision = BigInt(world.revision);
        if (world.player.id === this.playerId && this.revision !== null && revision < this.revision) {
          this.trace("stale_snapshot", { revision: world.revision });
          return;
        }
        if (world.player.id !== this.playerId) {
          this.resetPlaying();
          this.playerId = world.player.id;
          this.ledger = new EventLedger();
          this.completed = new Set();
          this.baseline = true;
          this.questScroll = null;
          this.deferredLevels.length = 0;
          this.previousSkills = new Map();
          this.cookRewardLevels = null;
          this.music = { groups: [], cursor: 0, mode: "once", regionBound: true };
        }
        const entities: Listener["entities"] = new Map();
        for (const entity of world.entities) {
          requireAudio(stableId(entity.id) && validTile(entity.tile) &&
            (entity.instance === null || stableId(entity.instance)),
          "AUDIO_WORLD", "Invalid source emitter in world view.");
          entities.set(entity.id, {
            tile: { ...entity.tile }, instance: entity.instance,
            available: entity.available, sourceId: entity.sourceId,
          });
        }
        this.listener = {
          id: world.player.id, region: world.player.region, tile: { ...world.player.tile },
          instance: world.player.instance, entities,
        };
        this.revision = revision;
        this.currentSkills = skills;
        if (this.baseline) {
          for (const quest of world.player.quests) if (quest.completed) this.completed.add(quest.id);
        }
        const musicRegion = sourceMusicRegion(this.listener.tile, this.nativeAreaMode);
        const changedMusicArea = (musicRegion?.name ?? null) !== this.musicAreaName;
        this.musicAreaName = musicRegion?.name ?? null;
        if (changedMusicArea || previous?.region !== this.listener.region || previous?.instance !== this.listener.instance ||
          previous.id !== this.listener.id) {
          this.epoch++;
          this.queue.clear();
          this.prepared.clear();
          this.stopWhere((voice) => voice.asset.kind === "sfx", "region_changed");
          if (this.music.regionBound) {
            const group = musicRegion?.defaultGroup ?? regionalTrack(this.listener.region);
            if (group === null || group !== this.music.groups[this.music.cursor]) {
              this.musicToken++;
              this.musicPending = null;
              this.musicExhausted = false;
              this.musicFailed = false;
              this.nextBackground = null;
              this.music = { groups: group === null ? [] : [group], cursor: 0, mode: "once", regionBound: true,
                transition: SOURCE_BACKGROUND_TRANSITION, scopeGroups: musicRegion?.groups ?? [] };
            }
            this.trace("region", { region: this.listener.region, group });
            if (group === null && !inputs.some((event) => event.kind === "music")) {
              this.unbound(`region:${this.listener.region}`,
                "This source map square has no pinned music switch. Supply the bound music event; no nearby track is substituted.");
            }
          }
          this.cancelUnusedLoads();
        }
      }
      this.connected = true;
      this.startClock();
      const events: AudioEvent[] = [];
      for (const input of inputs) {
        try {
          events.push(validateEvent(input));
        } catch (error) {
          this.notifyError(failure(error, "AUDIO_EVENT", "Rejected audio event"));
        }
      }
      const completion = events.find((event) => event.kind === "quest_complete" && event.payload.questId === COOK &&
        event.payload.committed === true && (event.sourceId === null || event.sourceId === 152) &&
        (event.assetId === null || event.assetId === this.catalog.groups.get("jingle:152")?.id) &&
        (event.actorId === null || event.actorId === this.listener?.id) && !this.ledger.hasEvent(event.id));
      if (completion && !this.completed.has(COOK) && world?.player.quests.some((quest) => quest.id === COOK && quest.completed)) {
        // An XP notification can precede the semantic completion in the same
        // committed batch. Stage only that supplied event, never create one.
        this.pendingCookCompletionId = completion.id;
        this.cookRewardLevels = observeRewardLevels(completion.id, this.previousSkills, this.currentSkills);
      }
      for (const event of events) {
        try {
          if (this.ledger.hasEvent(event.id)) {
            this.trace("duplicate_event", { eventId: event.id });
            continue;
          }
          this.consume(event, world);
          this.ledger.event(event.id);
        } catch (error) {
          this.notifyError(failure(error, "AUDIO_EVENT", "Rejected audio event"));
        }
      }
      this.pendingCookCompletionId = null;
      if (world !== null) {
        for (const quest of world.player.quests) if (quest.completed) this.completed.add(quest.id);
        this.baseline = false;
        this.previousSkills = this.currentSkills;
      }
      this.reconcileSpatial();
      this.startClock();
      void this.ensureMusic().catch((error) => this.notifyError(failure(error, "AUDIO_PLAYBACK", "Cannot play background music")));
      this.publish();
    } catch (error) {
      this.notifyError(failure(error, "AUDIO_WORLD", "Cannot update audio"));
    }
  }

  volume(channel: Channel, value: number): void {
    if (this.disposed) return;
    try {
      requireAudio(["music", "effects", "area"].includes(channel) && unit(value),
        "AUDIO_VOLUME", "Audio volume must be a finite channel gain between 0 and 1.");
      this.volumes[channel] = value;
      this.nativeMixer[channel] = sourceSliderToMixer(channel, Math.round(value * 100), this.masterPercent);
      this.trace("volume", { channel, value, nativeMixer: this.nativeMixer[channel] });
      if (channel === "music") {
        for (const voice of this.voices.values()) if (voice.channel === "music") {
          if (this.needsNativeGainInput(voice.asset, this.nativeMixer.music)) {
            this.stopVoice(voice, "native_gain_representation_required");
            if (voice.asset.kind === "jingle") {
              this.jingle = null;
              this.jinglePending = null;
              this.musicalToken++;
            }
            this.notifyError(new AudioFailure("AUDIO_NATIVE_GAIN_INPUT_REQUIRED",
              `Original ${voice.asset.kind} ${voice.asset.sourceId} cannot be raised to this native level using the frozen128 representation without extra clipping.`,
              true, { sourceId: voice.asset.sourceId, nativeMixer: this.nativeMixer.music }));
            continue;
          }
          voice.calibrationGain = sourceMixerToAssetGain(this.nativeMixer.music);
          if (!voice.musicFader) voice.gain.gain.setValueAtTime(voice.level * voice.asset.inputGain * voice.calibrationGain, this.context.currentTime);
        }
      }
      if (this.nativeMixer[channel] === 0) {
        if (channel === "music") this.stopMusical();
        // Original ordinary effects sample the native mixer at dispatch; their
        // already-playing streams are not rewritten by a preference change.
        else if (channel === "area") this.reconcileSpatial();
        this.cancelUnusedLoads();
      } else if (channel === "music") {
        this.musicFailed = false;
        this.backgroundPreparation = null;
        void this.ensureMusic().catch((error) => this.notifyError(failure(error, "AUDIO_PLAYBACK", "Cannot restore music")));
      }

      this.publish();
    } catch (error) {
      this.notifyError(failure(error, "AUDIO_VOLUME", "Cannot change audio volume"));
    }
  }

      sourceMaster(percent: number): void {
        requireAudio(integer(percent, 0, 100), "AUDIO_SOURCE_VOLUME", "Invalid original master slider percentage.");
        this.masterPercent = percent;
        for (const channel of ["music", "effects", "area"] as const) this.volume(channel, this.volumes[channel]);
      }

      sourceMusicDriver(selector: SourceMusicSelector | null, areaMode: "modern" | "classic"): void {
        this.musicSelector = selector;
        this.nativeAreaMode = areaMode;
        this.musicAreaName = null;
        if (selector && this.musicExhausted && this.music.groups[this.music.cursor] !== undefined) {
          void this.requestSourceNext(this.music, this.music.groups[this.music.cursor]!, this.musicToken);
        }
      }

      setSourceScene(scene: SourceAudioScene | null): void {
        if (scene === null) {
          this.sourceScene = null;
          this.ambientPending.clear();
          this.ambientIntervals.clear();
          this.stopWhere((voice) => voice.ambient, "source_scene_removed");
          this.cancelUnusedLoads();
          return;
        }
        requireAudio(integer(scene.plane, 0, 3) && Number.isFinite(scene.listener.x) &&
          Number.isFinite(scene.listener.y) && scene.emitters.length <= 10_000,
        "AUDIO_SOURCE_SCENE", "Invalid original audio scene projection.");
        const emitters = scene.emitters.map((entry) => {
          requireAudio(stableId(entry.id) && validTile(entry.tile) && integer(entry.orientation, 0, 3) &&
            integer(entry.objectId, 0, 1_000_000) && typeof entry.present === "boolean",
          "AUDIO_SOURCE_SCENE", "Invalid source object emitter.");
          return Object.freeze({ ...entry, tile: Object.freeze({ ...entry.tile }), owner: entry.owner ? Object.freeze({ ...entry.owner }) : null });
        });
        this.sourceScene = Object.freeze({
          ...scene, listener: Object.freeze({ ...scene.listener }),
          owner: scene.owner ? Object.freeze({ ...scene.owner }) : null,
          varps: new Map(scene.varps), emitters: Object.freeze(emitters),
        });
        const presentIds = new Set(emitters.filter((entry) => entry.present).map((entry) => entry.id));
        for (const id of this.ambientIntervals.keys()) if (!presentIds.has(id)) this.ambientIntervals.delete(id);
        for (const [key, pending] of this.ambientPending) {
          if (!presentIds.has(pending.spatial?.actorId ?? "")) this.ambientPending.delete(key);
        }
        this.sourceSceneErrors.clear();
        this.reconcileSpatial();
        this.reconcileSourceEmitters();
      }

      private reconcileSourceEmitters(advanceRandom = false): void {
        if (!this.sourceScene || !this.listener || !this.canPlay() || this.muted || this.nativeMixer.area === 0) return;
        for (const emitter of this.sourceScene.emitters) {
          try {
          if (!emitter.present || emitter.instance !== this.listener.instance) continue;
          const definition = resolveSourceObject(emitter.objectId, this.sourceScene.varps);
          if (!definition?.sound) continue;
          if (!sourceAmbientVisible(this.sourceScene.plane, emitter.tile.plane, this.sourceScene.owner,
            emitter.owner, definition.sound.visibility)) continue;
          const mix = sourceAmbientSpatial(this.sourceScene.listener,
            sourceObjectBounds(emitter.tile, definition.sizeX, definition.sizeY, emitter.orientation),
            definition.sound.range, definition.sound.retain, this.nativeMixer.area);
          if (!mix.audible) continue;
          const key = `ambient/${emitter.id}/${definition.sound.id}`;
          if (definition.sound.id >= 0 && !this.ambientPending.has(key) &&
            !Array.from(this.voices.values()).some((v) => v.ambient && !v.ambientRandom && v.spatial?.actorId === emitter.id)) {
          const event: AudioEvent = {
            id: `source-scene/${++this.sceneSequence}`, kind: "sound", sourceId: definition.sound.id,
            assetId: null, actorId: emitter.id, tile: emitter.tile, sourceCycle: null,
            payload: { committed: true, ambient: true, active: true, objectId: emitter.objectId,
              orientation: emitter.orientation, repeatCount: 1, delayCycles: 0 },
          };
          this.enqueue(event, this.asset("sfx", definition.sound.id, null), key, 0, 1,
            definition.sound.range, definition.sound.retain);
          }
          const random = definition.random;
          if (random?.ids?.length) {
            if (!this.ambientIntervals.has(emitter.id)) this.ambientIntervals.set(emitter.id,
              random.minCycles + (random.maxCycles > random.minCycles ? sourceRandomBelow(random.maxCycles - random.minCycles) : 0));
            if (!advanceRandom) continue;
            const randomKey = `ambient-random/${emitter.id}`;
            if (this.ambientPending.has(randomKey) || Array.from(this.voices.values()).some((v) => v.key === randomKey)) continue;
            const remaining = this.ambientIntervals.get(emitter.id)! - 1;
            this.ambientIntervals.set(emitter.id, remaining);
            if (remaining <= 0) {
              const id = random.ids[sourceRandomBelow(random.ids.length)]!;
              const event: AudioEvent = {
                id: `source-scene-random/${++this.sceneSequence}`, kind: "sound", sourceId: id,
                assetId: null, actorId: emitter.id, tile: emitter.tile, sourceCycle: null,
                payload: { committed: true, ambient: true, ambientRandom: true, active: true,
                  objectId: emitter.objectId, orientation: emitter.orientation, repeatCount: 1, delayCycles: 0 },
              };
              this.enqueue(event, this.asset("sfx", id, null), randomKey, 0, 1, definition.sound.range, definition.sound.retain);
              this.ambientIntervals.set(emitter.id, random.minCycles +
                (random.maxCycles > random.minCycles ? sourceRandomBelow(random.maxCycles - random.minCycles) : 0));
            }
          }
          } catch (error) {
            const problem = failure(error, "AUDIO_SOURCE_SCENE", "Cannot resolve source emitter");
            const key = `${emitter.id}/${problem.code}`;
            if (!this.sourceSceneErrors.has(key)) {
              this.sourceSceneErrors.add(key);
              this.notifyError(problem);
            }
          }
        }
      }
  mute(value: boolean): void {
    if (this.disposed) return;
    if (typeof value !== "boolean") {
      this.notifyError(new AudioFailure("AUDIO_VOLUME", "Mute requires a boolean."));
      return;
    }
    if (this.muted === value) return;
    this.muted = value;
    this.master.gain.setValueAtTime(value ? 0 : 1, this.context.currentTime);
    if (value) {
      this.resetPlaying();
      this.deferredLevels.length = 0;
    } else {
      void this.ensureMusic().catch((error) => this.notifyError(failure(error, "AUDIO_PLAYBACK", "Cannot unmute music")));
    }
    this.trace("mute", { value });
    this.publish();
  }

  disconnected(): void {
    if (this.disposed || !this.connected) return;
    this.connected = false;
    this.resetPlaying();
    this.stopClock();
    this.questScroll = null;
    this.deferredLevels.length = 0;
    this.cookRewardLevels = null;
    this.trace("disconnected", {});
    this.publish();
  }

  async dispose(): Promise<void> {
    if (this.disposed) return;
    this.disposed = true;
    this.connected = false;
    this.resetPlaying();
    this.stopClock();
    this.cache.dispose();
    this.context.removeEventListener("statechange", this.stateChanged);
    this.context.removeEventListener("sinkchange", this.sinkChanged);
    for (const bus of Object.values(this.buses)) bus.disconnect();
    this.master.disconnect();
    if (this.context.state !== "closed") {
      await deadline(this.context.close(), 2500, new AudioFailure(
        "AUDIO_CLOSE_TIMEOUT", "Audio nodes were disconnected, but the browser did not close AudioContext.", false,
      ));
    }
    this.trace("disposed", { state: this.context.state });
    this.publish();
    this.observers.clear();
  }

  private consume(event: AudioEvent, world: WorldView | null): void {
    if (event.kind === "music") {
      this.selectMusic(event);
      return;
    }
    requireAudio(world !== null && this.listener !== null, "AUDIO_EVENT",
      "Gameplay audio cannot be emitted by a title screen or absent world.");
    if (event.kind === "quest_complete") {
      const quest = event.payload.questId;
      requireAudio((quest === LEARNING || quest === COOK) && event.payload.committed === true &&
        (event.actorId === null || event.actorId === this.listener.id) &&
        world.player.quests.some((q) => q.id === quest && q.completed),
      "AUDIO_QUEST", "Quest audio requires a legitimate committed completion in the authoritative world.");
      if (this.completed.has(quest)) {
        this.trace("completion_already_seen", { quest, eventId: event.id });
        return;
      }
      requireAudio(event.sourceId === null || event.sourceId === 152,
        "AUDIO_QUEST", "The approved completion selector is original jingle 152.");
      const asset = this.asset("jingle", 152, event.assetId);
      this.completed.add(quest);
      if (quest === COOK) {
        this.questScroll = event.id;
        if (this.cookRewardLevels?.completionId !== event.id) {
          this.cookRewardLevels = observeRewardLevels(event.id, this.previousSkills, this.currentSkills);
        }
      }
      this.trace("binding", {
        eventId: event.id, sourceId: 152,
        classification: quest === LEARNING ? "approved_adaptation" : "dated_public_source_observation",
      });
      this.requestJingle(asset, event.id, this.jingleKey(event, asset.sourceId));
      return;
    }
    if (event.kind === "interface_closed") {
      requireAudio(event.sourceId === 153 && event.payload.questId === COOK,
        "AUDIO_INTERFACE", "Only the source quest-scroll closure releases deferred Cook reward audio.");
      if (this.questScroll !== null && event.payload.completionId === this.questScroll) {
        this.questScroll = null;
        for (const level of this.deferredLevels.splice(0)) this.playJingleEvent(level);
      }
      return;
    }
    if (event.kind === "jingle" || event.kind === "level_up") {
      requireAudio(event.payload.committed === true, "AUDIO_EVENT", "Jingles require committed source events.");
      requireAudio(event.actorId === null || event.actorId === this.listener.id,
        "AUDIO_EVENT", "A local music jingle cannot be selected by another actor's notification.");
      if (event.payload.causeQuestId === COOK) {
        const delta = committedRewardLevel(event, this.cookRewardLevels, this.currentSkills);
        if (delta === null) {
          this.trace("reward_no_base_level_gain", { eventId: event.id, skillId: String(event.payload.skillId) });
          return;
        }
        if (this.cookRewardLevels!.acceptedSkills.has(delta.skillId)) {
          this.trace("duplicate_reward_level", { eventId: event.id, skillId: delta.skillId });
          return;
        }
        if (event.sourceId !== -1) this.asset("jingle", event.sourceId, event.assetId);
        const committed = Object.freeze({
          ...event,
          payload: Object.freeze({ ...event.payload, skillId: delta.skillId,
            previousLevel: delta.before.baseLevel, level: delta.after.baseLevel,
            completionId: this.cookRewardLevels!.completionId }),
        });
        if (this.questScroll !== null || this.pendingCookCompletionId !== null) {
          requireAudio(this.deferredLevels.length < 50, "AUDIO_QUEST", "The deferred source level queue is full.");
          this.deferredLevels.push(committed);
          this.trace("jingle_deferred", { eventId: event.id, group: event.sourceId, skillId: delta.skillId,
            previousLevel: delta.before.baseLevel, level: delta.after.baseLevel });
        } else {
          this.playJingleEvent(committed);
        }
        this.cookRewardLevels!.acceptedSkills.add(delta.skillId);
        return;
      }
      this.playJingleEvent(event);
      return;
    }
    requireAudio(event.payload.committed === true, "AUDIO_EVENT",
      "Action sounds require an authoritative committed event, never an optimistic UI request.");
    if (event.kind === "animation") {
      this.animation(event);
      return;
    }
    requireAudio(event.kind === "sound", "AUDIO_EVENT", "Unsupported source audio event.");
    if (event.payload.selector === "ordinary_food") {
      requireAudio(event.sourceId === 2393, "AUDIO_BINDING", "Ordinary food uses original cue 2393.");
      this.animation({ ...event, kind: "animation", sourceId: 12526 });
      return;
    }
    if (event.payload.selector !== undefined) {
      requireAudio(typeof event.payload.selector === "string" &&
        OBSERVED_SELECTORS[event.payload.selector] === event.sourceId,
      "AUDIO_BINDING", "This action-to-sound selector is not bound by the approved reference.");
      this.trace("binding", { eventId: event.id, sourceId: event.sourceId, classification: "dated_public_source_observation" });
    }
    const id = event.sourceId;
    requireAudio(integer(id, 0, 65534), "AUDIO_EVENT", "A sound event requires its original numeric ID.");
    const asset = this.asset("sfx", id, event.assetId);
    const phase = event.payload.phase ?? "start";
    if (phase === "prepare") {
      this.prepare(event, [asset]);
      return;
    }
    if (phase === "stop" || phase === "update") {
      this.updateEmitter(event, asset, phase);
      return;
    }
    requireAudio(phase === "start" && integer(event.payload.delayCycles, 0, 65535) &&
      integer(event.payload.repeatCount, 0, 255),
    "AUDIO_EVENT", "Received effects require explicit native repeatCount and delayCycles; no guessed server delay is added.");
    if (id === 2266) requireAudio(event.payload.binding === "source_packet",
      "AUDIO_BINDING", "UI 2266 is allowed only for its explicit bound source event, never every click.");
    const requestedKey = event.payload.cueId ?? event.id;
    requireAudio(stableId(requestedKey), "AUDIO_EVENT", "Invalid sound correlation ID.");
    let key = requestedKey;
    if (integer(event.payload.sequenceId, 0, 65534) && integer(event.payload.frame, 0, 65535)) {
      const cues = this.catalog.sequences.get(event.payload.sequenceId)?.filter((cue) => cue.frame === event.payload.frame);
      requireAudio(cues?.some((cue) => cue.sourceId === id), "AUDIO_BINDING", "The declared sound is not on this source frame.");
      if (event.payload.sequenceId === 12526) this.foodBinding(event);
      key = cueKey(event, event.payload.sequenceId, event.payload.frame);
    }
    this.enqueue(event, asset, key, event.payload.delayCycles, event.payload.repeatCount);
  }

  private animation(event: AudioEvent): void {
    const sequence = event.sourceId;
    requireAudio(sequence !== null && this.catalog.sequences.has(sequence),
      "AUDIO_BINDING", "This sequence has no bound frame sounds. Do not invent bow, NPC, smelt, walk, or silent-829 cues.");
    if (sequence === 12526) this.foodBinding(event);
    const phase = event.payload.phase ?? "start";
    if (phase === "prepare") {
      this.prepare(event, [...new Set(this.catalog.sequences.get(sequence)!.map((cue) => cue.sourceId))]
        .map((id) => this.asset("sfx", id, null)));
      return;
    }
    if (phase === "cancel") {
      const action = actionId(event);
      this.queue.remove((effect) => effect.actionId === action);
      this.stopWhere((voice) => voice.actionId === action, "animation_cancelled");
      this.prepared.delete(action);
      this.cancelUnusedLoads();
      return;
    }
    requireAudio(phase === "start" || phase === "frame", "AUDIO_EVENT", "Invalid source animation phase.");
    requireAudio(event.actorId !== null && (event.actorId === this.listener!.id || this.listener!.entities.has(event.actorId)),
      "AUDIO_EVENT", "A source animation must identify a current authoritative actor.");
    if (phase === "frame") requireAudio(integer(event.payload.frame, 0, 65535),
      "AUDIO_EVENT", "A frame callback must identify the source frame.");
    const groups = new Map<number, FrameCue[]>();
    requireAudio(phase === "frame" || event.assetId === null, "AUDIO_EVENT",
      "Animation starts address a sequence, not one asset across potentially different frame cues.");
    for (const cue of this.catalog.sequences.get(sequence)!) {
      if (phase === "frame" && cue.frame !== event.payload.frame) continue;
      const group = groups.get(cue.frame) ?? [];
      group.push(cue);
      groups.set(cue.frame, group);
    }
    requireAudio(groups.size > 0, "AUDIO_BINDING", "No sound exists at the requested source frame.");
    for (const [frame, cues] of groups) {
      const key = cueKey(event, sequence, frame);
      if (this.ledger.hasCue(key)) {
        this.trace("duplicate_cue", { eventId: event.id, key });
        continue;
      }
      const roll = event.payload.weightRoll ?? (cues.length === 1 ? 0 : sourceRoll());
      requireAudio(typeof roll === "number", "AUDIO_EVENT", "Invalid source weight roll.");
      const cue = selectWeighted(cues, roll);
      if (cue.sourceId === 2411) {
        this.trace("source_silence_selected", { eventId: event.id, sourceId: 2411, roll, weight: 74, sequence, frame });
      }
      // A start event counts the current frame's source cycles. A frame callback
      // is already at that boundary. Neither adds the waveform's leading offset.
      const delay = phase === "start" ? Math.max(0, cue.cycle - 1) : 0;
      const tile = event.tile ?? (event.actorId === this.listener!.id
        ? this.listener!.tile : this.listener!.entities.get(event.actorId!)!.tile);
      this.enqueue({ ...event, tile }, this.asset("sfx", cue.sourceId, event.assetId), key,
        delay, cue.repeats, cue.range, cue.retain);
    }
  }

  private foodBinding(event: AudioEvent): void {
    requireAudio(event.payload.committed === true && event.actorId === this.listener?.id &&
      (event.payload.itemId === 315 || event.payload.itemId === 2309),
    "AUDIO_BINDING", "The ordinary-food adaptation is only for committed shrimp 315 / bread 2309 consumption.");
    this.trace("binding", {
      eventId: event.id, sourceId: 2393, classification: "approved_adaptation", sourceSequence: 12526,
    });
  }

  private prepare(event: AudioEvent, assets: readonly SourceAsset[]): void {
    const action = actionId(event);
    requireAudio(this.prepared.has(action) || this.prepared.size < 50,
      "AUDIO_PREPARE_LIMIT", "Too many concurrent audio activities to prepare.");
    const needed = this.prepared.get(action) ?? new Set<string>();
    this.prepared.set(action, needed);
    for (const asset of assets) {
      if (!asset.playable) continue;
      needed.add(asset.id);
      const epoch = this.epoch;
      void this.cache.load(asset).then(() => {
        if (epoch === this.epoch) this.trace("prepared", { eventId: event.id, actionId: action, sourceId: asset.sourceId });
      }, (error) => {
        if (epoch === this.epoch && error?.code !== "AUDIO_CANCELLED") {
          this.notifyError(failure(error, "AUDIO_LOAD", "Cannot prepare activity audio"));
        }
      });
    }
  }

  private spatial(event: AudioEvent, range?: number, retain?: number): { spatial: Spatial | null; gain: number } {
    if (event.tile === null) {
      requireAudio(event.payload.sourceGain === undefined || unit(event.payload.sourceGain),
        "AUDIO_EVENT", "Invalid source gain.");
      return { spatial: null, gain: event.payload.sourceGain ?? 1 } as { spatial: null; gain: number };
    }
    const listener = this.listener!;
    const actor = event.actorId === listener.id ? listener : this.listener!.entities.get(event.actorId ?? "");
    const sourceRange = range ?? event.payload.range;
    const sourceRetain = retain ?? event.payload.retain;
    requireAudio((range === undefined || event.payload.range === undefined || event.payload.range === range) &&
      (retain === undefined || event.payload.retain === undefined || event.payload.retain === retain),
    "AUDIO_SPATIAL_POLICY", "Supplied range/retention disagrees with the original bound cue or emitter.");
    requireAudio(integer(sourceRange, 0, 255) && integer(sourceRetain, 0, 255),
      "AUDIO_SPATIAL_POLICY", "Positional events must retain their source range and retention fields.");
    requireAudio(event.payload.instance === undefined || typeof event.payload.instance === "string",
      "AUDIO_EVENT", "Invalid source emitter instance.");
    const instance = typeof event.payload.instance === "string"
      ? event.payload.instance : this.sourceScene?.emitters.find((entry) => entry.id === event.actorId)?.instance
        ?? actor?.instance ?? null;
    const spatial: Spatial = {
      tile: { ...event.tile }, actorId: event.actorId, instance,
      range: sourceRange, retain: sourceRetain, distance: 0, listenerStamp: stamp(listener),
      nativeComputed: event.payload.sourceGain === undefined,
      ambient: event.payload.ambient === true,
      objectId: integer(event.payload.objectId, 0, 1_000_000) ? event.payload.objectId : null,
      orientation: integer(event.payload.orientation, 0, 3) ? event.payload.orientation : 0,
    };
    const result = this.nativePosition(spatial);
    spatial.distance = result.distance;
    if (event.payload.sourceGain !== undefined) {
      requireAudio(unit(event.payload.sourceGain), "AUDIO_SOURCE_POSITION", "Invalid explicit source gain.");
      return { gain: event.payload.sourceGain, spatial };
    }
    return { gain: this.nativeMixer.area === 0 ? 0 : result.volume / this.nativeMixer.area, spatial };
  }

  private nativePosition(spatial: Spatial): ReturnType<typeof sourcePacketSpatial> {
    const listener = this.sourceScene?.listener ?? {
      x: this.listener!.tile.x * 128 + 64, y: this.listener!.tile.y * 128 + 64,
    };
    if (spatial.ambient) {
      requireAudio(spatial.objectId !== null, "AUDIO_SOURCE_OBJECT", "A native ambient sound needs its original object identity.");
      const definition = resolveSourceObject(spatial.objectId, this.sourceScene?.varps ?? new Map());
      if (definition === null || definition.sound === null) {
        return { distance: Infinity, radius: 0, retainedRadius: 0, volume: 0, audible: false };
      }
      return sourceAmbientSpatial(listener,
        sourceObjectBounds(spatial.tile, definition.sizeX, definition.sizeY, spatial.orientation),
        definition.sound.range, definition.sound.retain, this.nativeMixer.area);
    }
    return sourcePacketSpatial(listener, { x: spatial.tile.x * 128, y: spatial.tile.y * 128 },
      spatial.range, spatial.retain, this.nativeMixer.area);
  }

  private enqueue(
    event: AudioEvent, asset: SourceAsset, key: string, delay: number, repeats: number,
    range?: number, retain?: number,
  ): void {
    if (this.ledger.hasCue(key)) {
      this.trace("duplicate_cue", { eventId: event.id, key });
      return;
    }
    requireAudio(!asset.id.startsWith("reference.audio.") || repeats <= 1,
      "AUDIO_REPEAT_POLICY", "These original reference WAVs bind one observed pass, not an unverified raw-sample repeat boundary.");
    const ambient = event.payload.ambient === true;
    const ambientRandom = event.payload.ambientRandom === true;
    if (ambient) {
      const originalId = event.payload.objectId as number;
      const definition = resolveSourceObject(originalId, this.sourceScene?.varps ?? new Map());
      const scene = this.sourceScene?.emitters.find((entry) => entry.id === event.actorId);
      const world = this.listener!.entities.get(event.actorId ?? "");
      requireAudio(definition?.sound && (ambientRandom ? definition.random?.ids?.includes(asset.sourceId) : definition.sound.id === asset.sourceId) &&
        event.payload.active === true &&
        event.actorId !== null &&
        ((scene?.present && scene.objectId === originalId) || (world?.available && world.sourceId === originalId)) &&
        (ambientRandom || asset.loopEnd > asset.loopStart),
      "AUDIO_AMBIENT", "Ambient playback needs the active original object/morph and actual sample loop, not a cache candidate alone.");
      range = definition.sound.range;
      retain = definition.sound.retain;
      key = ambientRandom ? `ambient-random/${event.actorId}` : `ambient/${event.actorId}/${asset.sourceId}`;
      if (Array.from(this.voices.values()).some((voice) => voice.key === key) ||
        this.ambientPending.has(key)) {
        this.trace("duplicate_emitter", { eventId: event.id, key });
        return;
      }
    }
    const position = this.spatial(event, range, retain);
    const channel = position.spatial === null ? "effects" : "area";
    if (!ambient) this.ledger.cue(key);
    if (!this.canPlay() || this.muted || this.nativeMixer[channel] === 0 || repeats === 0 ||
      (position.spatial !== null && !this.inRange(position.spatial))) {
      this.trace("effect_gated", { eventId: event.id, sourceId: asset.sourceId, channel, repeats });
      if (!this.hasUnlocked || this.context.state !== "running") this.gestureFeedback();
      return;
    }
    const now = this.context.currentTime;
    const effect: Effect = {
      eventId: event.id, actionId: actionId(event), key, asset, channel, gain: position.gain,
      repeats, ambient, ambientRandom, spatial: position.spatial,
      buffer: null, error: null, lateReported: false, requestedAt: now,
      dueAt: this.nextCycleAt + delay * SOURCE_CYCLE_SECONDS,
      enqueuedCycle: this.processingCycle, epoch: this.epoch,
    };
    if (ambient) {
      this.ambientPending.set(key, effect);
      void this.cache.load(asset).then((buffer) => {
        if (this.ambientPending.get(key) !== effect || effect.epoch !== this.epoch || !this.canPlay() ||
          this.muted || !effect.spatial || !this.inRange(effect.spatial)) return;
        if (effect.eventId.startsWith("source-scene")) {
          const current = this.sourceScene?.emitters.find((entry) => entry.id === effect.spatial?.actorId);
          if (!current?.present || current.objectId !== effect.spatial.objectId) return;
          const resolved = resolveSourceObject(current.objectId, this.sourceScene!.varps);
          if (ambientRandom ? !resolved?.random?.ids?.includes(asset.sourceId) : resolved?.sound?.id !== asset.sourceId) return;
        }
        const calculated = this.nativePosition(effect.spatial);
        const level = effect.spatial.nativeComputed ? calculated.volume / this.nativeMixer.area : effect.gain;
        this.startVoice(asset, buffer, "area", effect.eventId, effect.actionId, key,
          level, this.context.currentTime, effect.spatial, true, 0, undefined, false, undefined, ambientRandom);
      }).catch((error) => {
        if (effect.epoch === this.epoch && error?.code !== "AUDIO_CANCELLED") {
          this.notifyError(failure(error, "AUDIO_AMBIENT", "Cannot load native object sound"));
        }
      }).finally(() => { if (this.ambientPending.get(key) === effect) this.ambientPending.delete(key); });
      return;
    }
    if (!this.queue.enqueue(effect, delay)) {
      this.trace("queue_overflow", { eventId: event.id, capacity: 50, sourceId: asset.sourceId });
      this.notifyError(new AudioFailure("AUDIO_QUEUE_FULL", "The native 50-entry FIFO is full; the new request was dropped."));
      return;
    }
    this.trace("queued", { eventId: event.id, sourceId: asset.sourceId, delay, dueAt: effect.dueAt, key });
    if (!asset.playable) return;
    void this.cache.load(asset).then((buffer) => { effect.buffer = buffer; }, (error) => {
      effect.error = failure(error, "AUDIO_LOAD", `Cannot load effect ${asset.sourceId}`);
    });
  }

  private updateEmitter(event: AudioEvent, asset: SourceAsset, phase: "stop" | "update"): void {
    const key = event.payload.ambient === true
      ? `ambient/${event.actorId}/${asset.sourceId}` : (event.payload.cueId ?? actionId(event));
    requireAudio(typeof key === "string", "AUDIO_EVENT", "Invalid emitter identity.");
    if (phase === "stop") {
      this.queue.remove((effect) => effect.key === key && effect.asset.id === asset.id);
      this.prepared.delete(actionId(event));
      this.ambientPending.delete(key);
      this.stopWhere((voice) => voice.key === key && voice.asset.id === asset.id, "emitter_stopped");
      this.cancelUnusedLoads();
      return;
    }
    const definition = event.payload.ambient === true
      ? resolveSourceObject(event.payload.objectId as number, this.sourceScene?.varps ?? new Map()) : null;
    if (event.payload.ambient === true) {
      requireAudio(definition?.sound?.id === asset.sourceId,
      "AUDIO_AMBIENT", "An ambient update must identify the same original emitter definition.");
    }
    const position = this.spatial(event, definition?.sound?.range, definition?.sound?.retain);
    requireAudio(position.spatial !== null, "AUDIO_SPATIAL_POLICY", "An emitter update needs an explicit position.");
    for (const effect of this.queue.values()) if (effect.key === key && effect.asset.id === asset.id) {
      effect.spatial = position.spatial;
      effect.gain = position.gain;
    }
    for (const voice of this.voices.values()) if (voice.key === key && voice.asset.id === asset.id) {
      if (!this.inRange(position.spatial)) this.stopVoice(voice, "outside_retention");
      else {
        voice.spatial = position.spatial;
        voice.level = position.gain;
        voice.gain.gain.setValueAtTime(position.gain * asset.inputGain, this.context.currentTime);
        this.trace("emitter_gain", { eventId: event.id, voiceId: voice.id, gain: position.gain });
      }
    }
  }

  private inRange(spatial: Spatial): boolean {
    const listener = this.listener;
    if (listener === null || spatial.instance !== listener.instance) return false;
    if (spatial.ambient) {
      const definition = spatial.objectId === null ? null : sourceObjectDefinition(spatial.objectId);
      const source = this.sourceScene?.emitters.find((emitter) => emitter.id === spatial.actorId);
      if (!sourceAmbientVisible(this.sourceScene?.plane ?? listener.tile.plane, spatial.tile.plane,
        this.sourceScene?.owner ?? null, source?.owner ?? null, definition?.sound?.visibility ?? 2)) return false;
    } else if (spatial.tile.plane !== listener.tile.plane) return false;
    const position = this.nativePosition(spatial);
    spatial.distance = position.distance;
    return position.audible;
  }

  private reconcileSpatial(): void {
    if (!this.listener) return;
    for (const voice of this.voices.values()) {
      const spatial = voice.spatial;
      // Native queued one-shots keep the gain sampled at dispatch. Original
      // continuous object sounds, unlike those packets, are updated each cycle.
      if (!spatial || !voice.ambient) continue;
      try {
      const actor = this.listener.entities.get(spatial.actorId ?? "");
      const sceneEmitter = this.sourceScene?.emitters.find((entry) => entry.id === spatial.actorId);
      if (sceneEmitter) {
        spatial.tile = { ...sceneEmitter.tile };
        spatial.instance = sceneEmitter.instance;
        spatial.orientation = sceneEmitter.orientation;
      }
      else if (actor) spatial.tile = { ...actor.tile };
      const definition = spatial.objectId === null ? null
        : resolveSourceObject(spatial.objectId, this.sourceScene?.varps ?? new Map());
      const present = sceneEmitter?.present ?? (actor?.available === true);
      const selected = voice.ambientRandom ? definition?.random?.ids?.includes(voice.asset.sourceId)
        : definition?.sound?.id === voice.asset.sourceId;
      const audible = present && selected && this.inRange(spatial);
      const position = this.nativePosition(spatial);
      const target = audible ? (spatial.nativeComputed ? position.volume
        : Math.ceil(voice.level * this.nativeMixer.area)) : 0;
      const hardIneligible = this.nativeMixer.area === 0 || spatial.tile.plane !== (this.sourceScene?.plane ?? this.listener.tile.plane) ||
        spatial.instance !== this.listener.instance;
      this.fadeAmbient(voice, target, hardIneligible ? 150 : definition?.sound?.fadeOutMs ?? 300, !audible);
      } catch (error) {
        this.stopVoice(voice, "source_emitter_policy_failure");
        this.notifyError(failure(error, "AUDIO_SOURCE_SCENE", "Cannot update original emitter state"));
      }
    }
  }

  private fadeAmbient(voice: Voice, target: number, configuredMs: number, stop: boolean): void {
    if (voice.ambientFade?.target === target && voice.ambientFade.stop === stop) return;
    const now = this.context.currentTime;
    let current = Math.round(voice.gain.gain.value / voice.asset.inputGain * 128);
    if (voice.ambientFade) {
      const f = voice.ambientFade;
      current = sourceAmbientFadeVolume(f.from, f.target, f.durationMs, (now - f.start) * 1000);
    }
    if (current === target && !stop) return;
    const base = this.nativeMixer.area || Math.max(1, current);
    const durationMs = sourceAmbientFadeDuration(current, target, base, configuredMs);
    voice.ambientFade = { from: current, target, start: now, durationMs, stop };
    this.trace("native_ambient_fade", { voiceId: voice.id, from: current, target, durationMs, stop });
    this.advanceAmbientFade(voice, now);
  }

  private advanceAmbientFade(voice: Voice, now: number): void {
    const fade = voice.ambientFade;
    if (!fade) return;
    const level = sourceAmbientFadeVolume(fade.from, fade.target, fade.durationMs, (now - fade.start) * 1000);
    voice.gain.gain.setValueAtTime(sourceMixerToAssetGain(level) * voice.asset.inputGain, now);
    if (fade.durationMs <= 0 || (now - fade.start) * 1000 >= fade.durationMs) {
      voice.ambientFade = null;
      voice.calibrationGain = sourceMixerToAssetGain(this.nativeMixer.area);
      voice.level = this.nativeMixer.area ? fade.target / this.nativeMixer.area : 0;
      if (fade.stop && fade.target === 0) this.stopVoice(voice, "native_ambient_fade_finished");
    }
  }

  private selectMusic(event: AudioEvent): void {
    if (event.sourceId === -1) {
      this.stopMusical();
      this.music = { groups: [], cursor: 0, mode: "once", regionBound: false };
      this.cancelUnusedLoads();
      return;
    }
    const selected = this.asset("music", event.sourceId, event.assetId);
    const mode = event.payload.mode ?? "area";
    requireAudio(["area", "single", "playlist", "shuffle"].includes(String(mode)), "AUDIO_MUSIC", "Invalid music mode.");
    let groups = [selected.sourceId];
    let repeatMode: MusicPlan["mode"] = "once";
    if (mode !== "area") {
      requireAudio(event.payload.unlocked === true, "AUDIO_MUSIC", "Manual music selection requires authoritative unlocked-track data.");
      requireAudio(event.payload.boundary === undefined || event.payload.boundary === "native_duration",
        "AUDIO_MUSIC_POLICY", "Source replays use the native duration/timer policy, not a seamless MIDI-end buffer loop.");
      if (mode === "single") repeatMode = "single";
      else {
        requireAudio(typeof event.payload.playlist === "string", "AUDIO_MUSIC", "A playlist must be an explicit list of unlocked source groups.");
        const parsed: unknown = JSON.parse(event.payload.playlist);
        requireAudio(Array.isArray(parsed) && parsed.length > 0 && parsed.length <= 100 &&
          parsed.every((group) => integer(group, 0, 65534)) && new Set(parsed).size === parsed.length &&
          parsed.includes(selected.sourceId),
        "AUDIO_MUSIC", "Invalid playlist; source controls allow at most 100 distinct unlocked tracks.");
        groups = parsed as number[];
        for (const group of groups) this.asset("music", group, null);
        if (mode === "shuffle" || event.payload.shuffle === true) {
          groups = [...groups];
          for (let i = groups.length - 1; i > 0; i--) {
            const j = sourceRandomBelow(i + 1);
            [groups[i], groups[j]] = [groups[j]!, groups[i]!];
          }
        }
        repeatMode = "playlist";
      }
    }
    const transition: SourceMusicTransition = {
      fadeOutDelayCycles: this.transitionNumber(event, "fadeOutDelayCycles", 0),
      fadeOutCycles: this.transitionNumber(event, "fadeOutCycles", 60),
      fadeInDelayCycles: this.transitionNumber(event, "fadeInDelayCycles", 60),
      fadeInCycles: this.transitionNumber(event, "fadeInCycles", 0),
    };
    this.music = { groups: Object.freeze(groups), cursor: groups.indexOf(selected.sourceId),
      mode: repeatMode, regionBound: mode === "area", transition };
    this.musicToken++;
    this.musicExhausted = false;
    this.musicFailed = false;
    this.musicPending = null;
    this.cancelUnusedLoads();
    this.trace("music_plan", { eventId: event.id, group: selected.sourceId, mode: String(mode), count: groups.length });
  }

  private transitionNumber(event: AudioEvent, key: string, fallback: number): number {
    const value = event.payload[key] ?? fallback;
    requireAudio(integer(value, 0, 65535), "AUDIO_SOURCE_MUSIC", `Invalid native ${key}.`);
    return value;
  }

  private playJingleEvent(event: AudioEvent): void {
    if (event.sourceId === -1) {
      this.trace("jingle_sentinel", { eventId: event.id });
      return;
    }
    const asset = this.asset("jingle", event.sourceId, event.assetId);
    this.requestJingle(asset, event.id, this.jingleKey(event, asset.sourceId));
  }

  private jingleKey(event: AudioEvent, sourceId: number): string {
    if (event.payload.cueId !== undefined) {
      requireAudio(stableId(event.payload.cueId), "AUDIO_EVENT", "Invalid jingle correlation ID.");
      return event.payload.cueId;
    }
    return `${event.actorId ?? this.listener?.id ?? "local"}/${actionId(event)}/jingle/${sourceId}`;
  }

  private requestJingle(asset: SourceAsset, eventId: string, key: string): void {
    if (!this.ledger.cue(key)) {
      this.trace("duplicate_cue", { eventId, key });
      return;
    }
    if (!this.canPlay() || this.muted || this.nativeMixer.music === 0) {
      this.trace("jingle_gated", { eventId, group: asset.sourceId });
      if (!this.hasUnlocked || this.context.state !== "running") this.gestureFeedback();
      return;
    }
    this.stopMusical();
    this.jingle = asset;
    const token = this.musicalToken;
    this.trace("jingle_accepted", { eventId, group: asset.sourceId });
    this.jinglePending = (async () => {
      try {
        const buffer = await this.cache.load(asset);
        if (token !== this.musicalToken || !this.canPlay() || this.muted) return;
        this.startVoice(asset, buffer, "music", eventId, eventId, eventId, 1,
          this.context.currentTime, null, false, token, asset.endFrame! / SOURCE_RATE, false, () => {
            if (token !== this.musicalToken) return;
            this.jingle = null;
            this.jinglePending = null;
            this.music.transition = SOURCE_JINGLE_TRANSITION;
            this.trace("jingle_finished", { eventId, group: asset.sourceId });
            void this.ensureMusic().catch((error) => this.notifyError(failure(error, "AUDIO_PLAYBACK", "Cannot resume remembered music")));
          });
      } catch (error) {
        if (token !== this.musicalToken) return;
        this.jingle = null;
        this.jinglePending = null;
        this.notifyError(failure(error, "AUDIO_JINGLE", "Cannot play requested jingle"));
        void this.ensureMusic().catch((problem) => this.notifyError(failure(problem, "AUDIO_PLAYBACK", "Cannot recover background music")));
      }
    })();
    this.cancelUnusedLoads();
  }

  private async ensureMusic(): Promise<void> {
    if (!this.canPlay() || this.muted || this.nativeMixer.music === 0 || this.music.groups.length === 0 ||
      this.musicExhausted || this.musicFailed) return;
    if (this.jingle !== null || this.jinglePending !== null) {
      const asset = this.asset("music", this.music.groups[this.music.cursor]!, null);
      if (this.backgroundPreparation?.id !== asset.id) {
        const preparation = { id: asset.id };
        this.backgroundPreparation = preparation;
        void this.cache.load(asset).catch((error) => {
          if (this.backgroundPreparation === preparation && error?.code !== "AUDIO_CANCELLED") {
            this.musicFailed = true;
            this.notifyError(failure(error, "AUDIO_MUSIC", "Cannot prepare the remembered background during a jingle"));
          }
        });
      }
      return;
    }
    if (this.musicPending !== null) return this.musicPending;
    if (Array.from(this.voices.values()).some((voice) => voice.asset.kind === "music" &&
      voice.musicToken === this.musicToken && !voice.retiring)) return;
    const token = this.musicToken;
    const plan = this.music;
    this.musicPending = this.startBackground(plan, plan.cursor, this.context.currentTime, token)
      .finally(() => { if (token === this.musicToken) this.musicPending = null; });
    return this.musicPending;
  }

  private async startBackground(plan: MusicPlan, cursor: number, when: number, token: number): Promise<void> {
    const asset = this.asset("music", plan.groups[cursor]!, null);
    let buffer: AudioBuffer;
    try {
      buffer = await this.cache.load(asset);
    } catch (error) {
      if (token !== this.musicToken || this.disposed || !this.connected) return;
      this.musicFailed = true;
      throw error;
    }
    if (token !== this.musicToken || !this.canPlay() || this.muted || this.nativeMixer.music === 0 ||
      this.jingle !== null || this.jinglePending !== null) return;
    const transition = plan.transition ?? (this.listener === null ? SOURCE_TITLE_TRANSITION : SOURCE_BACKGROUND_TRANSITION);
    const readyAt = Math.max(this.context.currentTime, when);
    const actualWhen = readyAt + transition.fadeInDelayCycles * SOURCE_CYCLE_SECONDS;
    for (const old of [...this.voices.values()]) if (old.asset.kind === "music" && !old.retiring) {
      this.musicFade(old, "out", transition.fadeOutCycles, readyAt + transition.fadeOutDelayCycles * SOURCE_CYCLE_SECONDS, true);
    }
    const voice = this.startVoice(asset, buffer, "music", `music/${token}/${cursor}`, "",
      `music/${token}/${cursor}`, transition.fadeInCycles ? 0 : 1, actualWhen, null, false, token, undefined,
      false, () => {
        if (token !== this.musicToken) return;
        if (plan.mode === "once") {
          this.musicExhausted = true;
          if (!this.sourceRequestDue) void this.requestSourceNext(plan, asset.sourceId, token);
        } else if (plan.mode === "playlist" || plan.mode === "single") {
          plan.cursor = (cursor + 1) % plan.groups.length;
        }
      });
    if (transition.fadeInCycles) this.musicFade(voice, "in", transition.fadeInCycles, actualWhen, false);
    this.trace("music_started", { group: asset.sourceId, voiceId: voice.id, mode: plan.mode, when: actualWhen });
    if (plan.mode === "once") {
      const duration = sourceMusicDurationSeconds(asset.sourceId);
      this.sourceRequestDue = duration === null ? null : { plan, group: asset.sourceId, token, at: actualWhen + duration };
    }
    if (plan.mode === "playlist" || plan.mode === "single") {
      const next = (cursor + 1) % plan.groups.length;
      const interval = sourceMusicDurationSeconds(asset.sourceId);
      requireAudio(interval !== null, "AUDIO_SOURCE_MUSIC", "This source track has no native player duration row.");
      const nextWhen = actualWhen + interval;
      void this.prepareNextBackground({ ...plan, transition: SOURCE_JINGLE_TRANSITION }, next, nextWhen, token).catch((error) => {
        if (token === this.musicToken) this.notifyError(failure(error, "AUDIO_PLAYLIST", "Cannot prepare the next source track"));
      });
    }

  }

    private async requestSourceNext(plan: MusicPlan, group: number, token: number): Promise<void> {
      if (!this.listener) {
        this.trace("source_title_pass_finished", { group });
        return;
      }
      const region = sourceMusicRegion(this.listener.tile, this.nativeAreaMode);
      if (!this.musicSelector) {
        this.notifyError(new AudioFailure("AUDIO_SOURCE_MUSIC_SELECTION_REQUIRED",
          "The native player requests its next source selection. Connect SourceMusicSelector; do not fabricate a playlist or leave an exhausted track as success.",
          true, { previousGroup: group, nativeArea: region?.areaId ?? -1,
            requiredGroups: (region?.groups ?? []).join(",") }));
        return;
      }
      try {
        const selection = await deadline(this.musicSelector({
          previousGroup: group, mode: plan.mode === "once" ? "area" : plan.mode,
          region, durationTicks: sourceMusicDurationSeconds(group) === null ? null : sourceMusicDurationSeconds(group)! / 0.6,
        }), 15_000, new AudioFailure("AUDIO_SOURCE_MUSIC_SELECTION_TIMEOUT", "The source music selector did not provide its next request."));
        if (token !== this.musicToken || !this.canPlay()) return;
        this.asset("music", selection.group, null);
        this.music = { groups: [selection.group], cursor: 0, mode: "once", regionBound: true,
          transition: selection.transition ?? SOURCE_BACKGROUND_TRANSITION };
        this.musicToken++;
        this.musicExhausted = false;
        this.musicFailed = false;
        await this.ensureMusic();
      } catch (error) {
        this.notifyError(failure(error, "AUDIO_SOURCE_MUSIC", "Cannot play the next original native selection"));
      }
    }

    private musicFade(voice: Voice, direction: "in" | "out", cycles: number, when: number, retire: boolean): void {
      if (retire && cycles === 0 && when <= this.context.currentTime) {
        this.stopVoice(voice, "native_zero_fade_replacement");
        return;
      }
      const nativeVolume = Math.round(voice.calibrationGain * 128);
      const current = voice.musicFader?.current ?? (direction === "in" ? 0
        : Math.round(voice.gain.gain.value / voice.asset.inputGain * 128));
      voice.musicFader = { direction, cycles, current, nextAt: when + SOURCE_CYCLE_SECONDS, retire };
      voice.level = direction === "in" ? 1 : 0;
      this.trace("native_music_fade", { voiceId: voice.id, direction, cycles, when, nativeVolume,
        startLevel: current });
      if (retire) voice.retiring = true;
    }

    private advanceMusicFade(voice: Voice, cycleAt: number): void {
      const fade = voice.musicFader;
      if (!fade || cycleAt + 0.000001 < fade.nextAt) return;
      const target = Math.round(voice.calibrationGain * 128);
      const active = fade.direction === "in" ? fade.current < target : fade.current > 0;
      if (!active) {
        voice.musicFader = null;
        if (fade.retire) this.stopVoice(voice, "native_music_fade_finished");
        return;
      }
      const step = fade.cycles === 0 ? target : Math.fround(target / fade.cycles);
      fade.current = fade.direction === "in"
        ? Math.min(target, Math.fround(fade.current + (step === 0 ? target : step)))
        : Math.max(0, Math.fround(fade.current - (step === 0 ? target : step)));
      voice.gain.gain.setValueAtTime(sourceMixerToAssetGain(Math.trunc(fade.current)) * voice.asset.inputGain,
        Math.max(this.context.currentTime, fade.nextAt));
      fade.nextAt += SOURCE_CYCLE_SECONDS;
      if (fade.retire && fade.current <= 0) this.stopVoice(voice, "native_music_fade_finished");
    }
  private async prepareNextBackground(plan: MusicPlan, cursor: number, when: number, token: number): Promise<void> {
    const asset = this.asset("music", plan.groups[cursor]!, null);
    try {
      await this.cache.load(asset);
    } catch (error) {
      if (token !== this.musicToken || this.disposed || !this.connected) return;
      throw error;
    }
    if (token !== this.musicToken || !this.canPlay()) return;
    // Do not recursively schedule the whole playlist. The source-cycle clock
    // only schedules its next entry shortly before the current MIDI end.
    this.nextBackground = { plan, cursor, when, token };
  }

  private nextBackground: { plan: MusicPlan; cursor: number; when: number; token: number } | null = null;

  private asset(kind: AssetKind, sourceId: number | null, assetId: string | null): SourceAsset {
    if (kind === "music" && sourceId !== null &&
      (SOURCE_UNPUBLISHED_M1_MUSIC as readonly number[]).includes(sourceId)) {
      throw new AudioFailure("AUDIO_SOURCE_MUSIC_ASSET_REQUIRED",
        `Native music table44 requires index6 group${sourceId}; it has no approved playable publication. No FLAC alias or substituted song is created.`,
        true, { sourceIndex: 6, sourceGroup: sourceId });
    }
    const asset = this.catalog.groups.get(`${kind}:${sourceId}`);
    requireAudio(asset && (assetId === null || assetId === asset.id), "AUDIO_ASSET_ID",
      `Unknown or mismatched original ${kind} identity ${sourceId}. No replacement is permitted.`);
    return asset;
  }

  private needsNativeGainInput(asset: SourceAsset, volume: number): boolean {
    return asset.kind !== "sfx" && SOURCE_MIX_REPRESENTATION_NEEDS.some((need) =>
      need.index === (asset.kind === "music" ? 6 : 11) && need.group === asset.sourceId) &&
      asset.peak * sourceMixerToAssetGain(volume) > 1;
  }

  private startVoice(
    asset: SourceAsset, buffer: AudioBuffer, channel: Channel, eventId: string, action: string, key: string,
    level: number, when: number, spatial: Spatial | null, ambient: boolean, musicToken: number,
    duration?: number, musicalLoop = false, ended?: () => void, ambientRandom = false,
  ): Voice {
    this.requireOpen();
    if (this.needsNativeGainInput(asset, this.nativeMixer.music)) {
      throw new AudioFailure("AUDIO_NATIVE_GAIN_INPUT_REQUIRED",
        `Original ${asset.kind} ${asset.sourceId} needs a native mixer-level representation at volume ${this.nativeMixer.music}; scaling the frozen128 PCM would add clipping. No limiter or normalization is substituted.`,
        true, { sourceId: asset.sourceId, nativeMixer: this.nativeMixer.music });
    }

    const source = this.context.createBufferSource();
    const gain = this.context.createGain();
    source.buffer = buffer;
    const calibrationGain = sourceMixerToAssetGain(this.nativeMixer[channel]);
    gain.gain.setValueAtTime(level * asset.inputGain * calibrationGain, when);
    if (ambient && !ambientRandom) {
      source.loop = true;
      source.loopStart = asset.loopStart / SOURCE_RATE;
      source.loopEnd = asset.loopEnd / SOURCE_RATE;
    } else if (musicalLoop) {
      source.loop = true;
      source.loopStart = 0;
      source.loopEnd = asset.endFrame! / SOURCE_RATE;
    }
    source.connect(gain);
    gain.connect(this.buses[channel]);
    const voice: Voice = {
      id: ++this.voiceId, asset, source, gain, channel, eventId, actionId: action, key,
      when, level, spatial, ambient, ambientRandom, musicToken, stopped: false, calibrationGain,
      ambientFade: null, retiring: false, musicFader: null,
    };
    this.voices.set(voice.id, voice);
    source.onended = () => {
      if (voice.stopped) return;
      voice.stopped = true;
      source.disconnect();
      gain.disconnect();
      this.voices.delete(voice.id);
      this.trace("ended", { voiceId: voice.id, sourceId: asset.sourceId, eventId, natural: !voice.retiring });
      ended?.();
      this.publish();
    };
    try {
      if (duration === undefined) source.start(when);
      else source.start(when, 0, duration);
      this.trace("started", {
        voiceId: voice.id, sourceId: asset.sourceId, kind: asset.kind, channel, eventId, when,
        duration: duration ?? buffer.duration, loop: source.loop,
        loopEnd: source.loopEnd, gain: level * asset.inputGain * calibrationGain,
      });
      if (asset.kind !== "sfx") this.publish();
      return voice;
    } catch (error) {
      this.stopVoice(voice, "start_failed");
      throw failure(error, "AUDIO_START", `AudioBufferSourceNode.start failed for ${asset.id}`);
    }
  }

  private stopVoice(voice: Voice, reason: string): void {
    if (voice.stopped) return;
    voice.stopped = true;
    voice.source.onended = null;
    try { voice.source.stop(); } catch (error) {
      this.trace("stop_error", { voiceId: voice.id, message: error instanceof Error ? error.message : String(error) });
    }
    voice.source.disconnect();
    voice.gain.disconnect();
    this.voices.delete(voice.id);
    this.trace("stopped", { voiceId: voice.id, sourceId: voice.asset.sourceId, eventId: voice.eventId, reason });
  }

  private stopWhere(predicate: (voice: Voice) => boolean, reason: string): void {
    for (const voice of [...this.voices.values()]) if (predicate(voice)) this.stopVoice(voice, reason);
  }

  private stopMusical(): void {
    this.musicToken++;
    this.musicExhausted = false;
    this.musicFailed = false;
    this.musicalToken++;
    this.musicPending = null;
    this.backgroundPreparation = null;
    this.jinglePending = null;
    this.jingle = null;
    this.nextBackground = null;
    this.sourceRequestDue = null;
    this.stopWhere((voice) => voice.channel === "music", "musical_replacement");
  }

  private resetPlaying(): void {
    this.epoch++;
    this.queue.clear();
    this.ambientPending.clear();
    this.ambientIntervals.clear();
    this.prepared.clear();
    this.stopMusical();
    this.stopWhere(() => true, "reset");
    this.cancelUnusedLoads();
  }

  private cancelUnusedLoads(): void {
    const wanted = new Set(this.queue.values().map((effect) => effect.asset.id));
    for (const effect of this.ambientPending.values()) wanted.add(effect.asset.id);
    for (const assets of this.prepared.values()) for (const id of assets) wanted.add(id);
    if (this.connected && !this.muted && this.nativeMixer.music > 0) {
      for (const group of [this.music.groups[this.music.cursor], this.nextBackground?.plan.groups[this.nextBackground.cursor]]) {
        const asset = this.catalog.groups.get(`music:${group}`);
        if (asset) wanted.add(asset.id);
      }
      if (this.jingle !== null) wanted.add(this.jingle.id);
    }
    this.cache.cancelUnused(wanted);
  }

  private canPlay(): boolean {
    return !this.disposed && this.connected && this.hasUnlocked &&
      !this.pendingGesture && !this.outputDisabled && this.context.state === "running";
  }

  private startClock(): void {
    if (this.timer !== null || !this.canPlay()) return;
    this.nextCycleAt = this.context.currentTime + SOURCE_CYCLE_SECONDS;
    this.timer = setInterval(this.processCycle, 5);
  }
  private stopClock(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
  }

  private processCycle = (): void => {
    if (!this.canPlay()) return;
    const now = this.context.currentTime;
    let changed = false;
    if (now - this.nextCycleAt > 0.1) {
      if (this.queue.size > 0) {
        this.notifyError(new AudioFailure("AUDIO_CLOCK_LATE",
          "The browser stopped servicing source cycles; stale effects were discarded rather than burst after a stalled tab.",
          true, { lateMs: (now - this.nextCycleAt) * 1000 }));
        this.queue.clear();
        changed = true;
        this.cancelUnusedLoads();
      }
      this.nextCycleAt = now + SOURCE_CYCLE_SECONDS;
    }
    // Process 50 Hz source cycles with a short scheduling lookahead. A 20 ms
    // JS interval drifts against 128-sample audio quanta; scheduling start(when)
    // ahead of the audio deadline avoids adding an accidental fourth/fifth tick.
    while (this.nextCycleAt <= now + 0.01) {
      const cycleAt = this.nextCycleAt;
      this.nextCycleAt += SOURCE_CYCLE_SECONDS;
      this.processingCycle++;
      this.queue.process((effect) => {
        changed = true;
        if (effect.epoch !== this.epoch || this.muted || this.nativeMixer[effect.channel] === 0) return true;
        if (!effect.asset.playable) {
          this.trace("source_silence", {
            eventId: effect.eventId, sourceId: 2411, when: Math.max(now, cycleAt), frames: 110,
            processingCalls: this.processingCycle - effect.enqueuedCycle,
          });
          return true;
        }
        if (effect.error !== null) {
          if (effect.error.code !== "AUDIO_CANCELLED") this.notifyError(effect.error);
          return true;
        }
        if (effect.buffer === null) {
          if (!effect.lateReported) {
            effect.lateReported = true;
            this.notifyError(new AudioFailure("AUDIO_LOADING_LATE",
              `Original cue ${effect.asset.sourceId} was not decoded for its scheduled source-cycle dispatch.`,
              true, { eventId: effect.eventId, dueAt: effect.dueAt }));
          }
          return false;
        }
        if (effect.spatial !== null && !this.inRange(effect.spatial)) {
          this.trace("effect_out_of_range", { eventId: effect.eventId });
          return true;
        }
        try {
          if (effect.spatial?.nativeComputed) {
            const position = this.nativePosition(effect.spatial);
            effect.gain = this.nativeMixer.area ? position.volume / this.nativeMixer.area : 0;
          }
          const buffer = effect.ambient ? effect.buffer : repeatedEffect(this.context, effect.buffer, effect.asset, effect.repeats);
          const when = Math.max(now, cycleAt, effect.dueAt);
          if (when - effect.dueAt > SOURCE_CYCLE_SECONDS + 1 / SOURCE_RATE) {
            this.notifyError(new AudioFailure("AUDIO_TIMING_LATE",
              `Cue ${effect.asset.sourceId} missed its source-cycle timing tolerance.`,
              true, { eventId: effect.eventId, lateMs: (when - effect.dueAt) * 1000 }));
          }
          this.startVoice(effect.asset, buffer, effect.channel, effect.eventId, effect.actionId, effect.key,
            effect.gain, when, effect.spatial, effect.ambient, 0);
          this.trace("effect_dispatched", {
            eventId: effect.eventId, sourceId: effect.asset.sourceId, dueAt: effect.dueAt, when,
            sourceOnset: effect.asset.firstNonzeroFrame === null ? null
              : when + effect.asset.firstNonzeroFrame / SOURCE_RATE,
            lateMs: Math.max(0, when - effect.dueAt) * 1000,
            processingCalls: this.processingCycle - effect.enqueuedCycle,
          });
        } catch (error) {
          this.notifyError(failure(error, "AUDIO_PLAYBACK", "Cannot start source effect"));
        }
        return true;
      }, (effect) => {
        this.notifyError(new AudioFailure("AUDIO_SOURCE_QUEUE_EXPIRED",
          `Original source queue expired undecoded cue ${effect.asset.sourceId} after its -10-cycle grace.`,
          true, { eventId: effect.eventId, sourceId: effect.asset.sourceId }));
      });
      this.reconcileSpatial();
      for (const voice of [...this.voices.values()]) if (voice.ambientFade) this.advanceAmbientFade(voice, Math.max(now, cycleAt));
      for (const voice of [...this.voices.values()]) if (voice.musicFader) this.advanceMusicFade(voice, cycleAt);
      this.reconcileSourceEmitters(true);
    }
    if (this.nextBackground && this.nextBackground.when - now <= 0.1) {
      const next = this.nextBackground;
      this.nextBackground = null;
      if (next.token === this.musicToken) {
        void this.startBackground(next.plan, next.cursor, next.when, next.token)
          .catch((error) => this.notifyError(failure(error, "AUDIO_PLAYLIST", "Cannot advance source playlist")));
      }
      if (this.sourceRequestDue && this.sourceRequestDue.at <= now) {
        const request = this.sourceRequestDue;
        this.sourceRequestDue = null;
        if (request.token === this.musicToken) void this.requestSourceNext(request.plan, request.group, request.token);
      }
    }
    if (changed) this.publish();
  };

  private stateChanged = (): void => {
    if (this.disposed) return;
    this.trace("context_state", { state: this.context.state });
    if (this.context.state !== "running") {
      this.pendingGesture = true;
      this.stopClock();
      this.queue.clear();
      this.stopWhere((voice) => voice.channel !== "music", "context_suspended");
      if (this.jingle !== null && !Array.from(this.voices.values()).some((voice) => voice.asset.kind === "jingle")) {
        this.stopMusical();
      }
      this.cancelUnusedLoads();
      this.notifyError(new AudioFailure(
        this.context.state === "closed" ? "AUDIO_CONTEXT_CLOSED" : "AUDIO_CONTEXT_SUSPENDED",
        `AudioContext is ${this.context.state}; playback is not running.${this.context.state === "closed"
          ? " Recreate the audio handle." : " Use a real gesture to resume."}`,
        this.context.state !== "closed",
      ));
      if (this.context.state === "closed") this.stopWhere(() => true, "context_closed");
    } else if (this.hasUnlocked && !this.pendingGesture) {
      this.startClock();
    }
    this.publish();
  };

  private sinkChanged = (): void => {
    if (this.disposed) return;
    const sink = (this.context as AudioContext & {
      readonly sinkId?: string | { readonly type: string };
    }).sinkId;
    this.outputDisabled = typeof sink === "object" && sink.type === "none";
    this.trace("output_sink", { enabled: !this.outputDisabled });
    if (this.outputDisabled) {
      this.resetPlaying();
      this.stopClock();
      this.notifyError(new AudioFailure("AUDIO_OUTPUT_DISABLED",
        "The browser selected a silent AudioContext output sink. A running clock alone is not enabled playback."));
    } else {
      this.startClock();
      void this.ensureMusic().catch((error) => this.notifyError(failure(error, "AUDIO_DEVICE", "Cannot restore output playback")));
    }
    this.publish();
  };

  private requireOpen(): void {
    requireAudio(!this.disposed && this.context.state !== "closed", "AUDIO_DISPOSED",
      "The audio context is closed; create a new handle.");
  }
  private gestureFeedback(): void {
    this.notifyError(new AudioFailure("AUDIO_GESTURE_REQUIRED",
      "Source events were not queued behind autoplay permission. Enable sound with a real gesture.",
      true, { state: this.context.state }));
  }
  private unbound(key: string, message: string): void {
    if (this.policyLimits.has(key)) return;
    this.policyLimits.add(key);
    this.notifyError(new AudioFailure("AUDIO_POLICY_UNBOUND", message, true, { policy: key }));
  }
  private trace(type: string, data: Record<string, TraceValue>): void {
    this.traces.push(Object.freeze({
      type, audioTime: this.context.currentTime, wallTime: performance.now(), data: Object.freeze({ ...data }),
    }));
    if (this.traces.length > 512) this.traces.shift();
  }
  private notifyError(error: AudioFailure): void {
    this.trace("error", { code: error.code, message: error.message });
    try { this.report(error); } catch { /* The host's reporting callback cannot leak audio resources. */ }
    this.publish();
  }
  private publish(): void {
    if (this.observers.size === 0) return;
    const state = this.snapshot();
    for (const observer of this.observers) {
      try { observer(state); } catch { /* Observers are diagnostic, never audio authority. */ }
    }
  }
}

/** Wire this exact factory into app orchestration; it never owns game outcomes. */
export const createAudio: CreateAudio = async (assets, report): Promise<AudioHandle> => {
  let context: AudioContext | null = null;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(new AudioFailure(
    "AUDIO_LOAD_TIMEOUT", "Timed out loading the approved audio metadata.",
  )), 15_000);
  try {
    requireAudio(typeof AudioContext !== "undefined" && globalThis.isSecureContext &&
      crypto.subtle !== undefined,
    "AUDIO_CAPABILITY", "Source audio requires WebAudio and secure-context integrity checking.");
    const catalog = await loadCatalog(assets, controller.signal);
    context = new AudioContext({ sampleRate: SOURCE_RATE, latencyHint: "interactive" });
    requireAudio(context.sampleRate === SOURCE_RATE, "AUDIO_DEVICE_FORMAT",
      "The device context did not accept the original 22050 Hz rendering rate.");
    const runtime = new Runtime(context, catalog, assets, report);
    runtimes.set(runtime, runtime);
    return runtime;
  } catch (error) {
    controller.abort(error);
    if (context && context.state !== "closed") await context.close().catch(() => undefined);
    const problem = failure(error, "AUDIO_INITIALIZATION", "Cannot initialize original browser audio");
    try { report(problem); } catch { /* Preserve the actual initialization failure. */ }
    throw problem;
  } finally {
    clearTimeout(timeout);
  }
};

/** Observation only; this never manufactures a running/ready state. */
export function readAudioState(handle: AudioHandle): AudioSnapshot {
  const runtime = runtimes.get(handle);
  requireAudio(runtime, "AUDIO_HANDLE", "Not a ClubScape source-audio handle.");
  return runtime.snapshot();
}

export function observeAudioState(handle: AudioHandle, observer: (state: AudioSnapshot) => void): () => void {
  const runtime = runtimes.get(handle);
  requireAudio(runtime, "AUDIO_HANDLE", "Not a ClubScape source-audio handle.");
  return runtime.subscribe(observer);
}

export function setSourceMasterVolume(handle: AudioHandle, percent: number): void {
  const runtime = runtimes.get(handle);
  requireAudio(runtime, "AUDIO_HANDLE", "Not a ClubScape source-audio handle.");
  runtime.sourceMaster(percent);
}

export function setSourceAudioScene(handle: AudioHandle, scene: SourceAudioScene | null): void {
  const runtime = runtimes.get(handle);
  requireAudio(runtime, "AUDIO_HANDLE", "Not a ClubScape source-audio handle.");
  runtime.setSourceScene(scene);
}

export function setSourceMusicSelector(
  handle: AudioHandle, selector: SourceMusicSelector | null, areaMode: "modern" | "classic" = "modern",
): void {
  const runtime = runtimes.get(handle);
  requireAudio(runtime, "AUDIO_HANDLE", "Not a ClubScape source-audio handle.");
  runtime.sourceMusicDriver(selector, areaMode);
}
