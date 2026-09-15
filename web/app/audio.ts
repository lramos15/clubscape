import {
  AudioFailure, createAudio, observeAudioState, readAudioState, setSourceAudioScene, setSourceMusicState,
  sourceAudioDefaults, sourceSliderToMixer, sourceMixerToAssetGain,
  applySourceAudioPreferences, readSourceAudioPreferences, setSourceMusicPreferences, selectSourcePlaylist,
  setSourceSavedPlaylist, editSourceSavedPlaylist, toggleSourceAudioMute, setSourceAudioPercent, requestSourceMusicSkip,
} from "../audio/index.ts";
import type { AudioSnapshot, AudioTrace, SourceAudioScene, SourceMusicState } from "../audio/index.ts";
import type { AudioEvent, AudioHandle, ClientAssets, CreateAudio, WorldView } from "../shared/contracts.ts";
import type { AssetObservation } from "./assets.ts";
import type { AssetRecord } from "./manifest.ts";
import { AppError, deepFreeze } from "./errors.ts";
import type { AudioChannel } from "./settings.ts";
import type { NativeAudioPreferences } from "./player-audio.ts";

export interface AudioAdapter {
  create: CreateAudio;
  read(handle: AudioHandle): AudioSnapshot;
  observe(handle: AudioHandle, listener: (state: AudioSnapshot) => void): () => void;
  scene(handle: AudioHandle, scene: SourceAudioScene | null): void;
  musicState(handle: AudioHandle, state: SourceMusicState): void;
  readMusicState?(handle: AudioHandle): Readonly<SourceMusicState> | null;
}
export const sourceAudioAdapter: AudioAdapter = {
  create: createAudio, read: readAudioState, observe: observeAudioState,
  scene: setSourceAudioScene, musicState: setSourceMusicState,
};

export function sourceControlState(state: AudioSnapshot) {
  const control = (channel: AudioChannel) => {
    const percent = Math.round(state.volumes[channel] * 100);
    return Object.freeze({
      normalizedPosition: state.volumes[channel], percent,
      nativeMixer: state.nativeMixer[channel],
      lookupMixer: sourceSliderToMixer(channel, percent, state.masterPercent),
      referenceGains: Object.freeze({
        native128: sourceMixerToAssetGain(state.nativeMixer[channel], 128),
        native255: sourceMixerToAssetGain(state.nativeMixer[channel], 255),
      }),
    });
  };
  return Object.freeze({ semantics: "native-source-slider-v1" as const, masterPercent: state.masterPercent,
    channels: Object.freeze({ music: control("music"), effects: control("effects"), area: control("area") }),
    defaults: sourceAudioDefaults(),
    playback: Object.freeze({ ...state.background, groups: Object.freeze([...state.background.groups]) }),
    voices: Object.freeze(state.voices.map((voice) => Object.freeze({
      id: voice.id, sourceId: voice.sourceId, kind: voice.kind, channel: voice.channel,
      assetId: voice.assetId, renderedNativeLevel: voice.renderedNativeLevel,
      appliedNativeLevel: voice.appliedNativeLevel, gain: voice.gain,
    }))) });
}

export function audioProblem(error: unknown): AppError {
  if (error instanceof AppError) return error;
  if (error instanceof AudioFailure) {
    return new AppError(`[${error.code}] ${error.message}`, {
      kind: "audio", errorId: error.code, recoverable: error.code === "AUDIO_GESTURE_REQUIRED" || error.recoverable,
    });
  }
  return new AppError("The source audio adapter failed.", { kind: "audio", errorId: "audio.adapter.failure" });
}

export function playbackEnabled(state: AudioSnapshot): boolean {
  return state.unlocked && !state.pendingGesture && state.contextState === "running"
    && state.outputEnabled && state.connected && !state.muted && !state.disposed;
}

/** Composition and observation only. No decoding, queue, source gain or playback policy lives here. */
export class SourceAudioSession {
  #handle: AudioHandle;
  #api: AudioAdapter;
  #stop: () => void;
  #report: (error: AppError) => void;
  #observations = new Map<string, AssetObservation>();
  #seen = new WeakSet<AudioTrace>();
  #hasScene = false;
  #sceneUnavailableReported = false;
  #actor: string | null = null;
  #musicState: Readonly<SourceMusicState> | null = null;

  private constructor(handle: AudioHandle, api: AudioAdapter, records: readonly AssetRecord[],
    report: (error: AppError) => void, observed: (state: AudioSnapshot) => void) {
    this.#handle = handle;
    this.#api = api;
    this.#report = report;
    const assets = new Map(records.map((record) => [record.id, record]));
    this.#stop = api.observe(handle, (state) => {
      for (const trace of state.traces) {
        if (this.#seen.has(trace)) continue;
        this.#seen.add(trace);
        const id = trace.data.assetId;
        if (typeof id !== "string") continue;
        const asset = assets.get(id);
        if (!asset) continue;
        // A fetch trace is only an attempt; the actual factory emits decoded after
        // successful hash verification, native decoding and signal validation.
        if (trace.type === "decoded") this.#observations.set(id, {
          id, sha256: asset.sha256, fetched: true, decoded: true, bytes: asset.bytes,
          fetchMs: null, decodeMs: null,
        });
        else if (trace.type === "evicted") this.#observations.delete(id);
      }
      if (state.disposed) this.#observations.clear();
      observed(state);
    });
  }

  static async create(assets: ClientAssets, records: readonly AssetRecord[],
    report: (error: AppError) => void, observed: (state: AudioSnapshot) => void,
    api: AudioAdapter = sourceAudioAdapter): Promise<SourceAudioSession> {
    let handle: AudioHandle;
    try { handle = await api.create(assets, (error) => report(audioProblem(error))); }
    catch (error) { throw audioProblem(error); }
    try { return new SourceAudioSession(handle, api, records, report, observed); }
    catch (error) { await handle.dispose(); throw audioProblem(error); }
  }

  snapshot(): AudioSnapshot { return this.#api.read(this.#handle); }
  observations(): AssetObservation[] { return Array.from(this.#observations.values(), (value) => ({ ...value })); }
  enabled(): boolean { return playbackEnabled(this.snapshot()); }
  controls() {
    const snapshot = this.snapshot();
    const source = this.#api.readMusicState ? this.#api.readMusicState(this.#handle) : this.#musicState;
    const musicState = source === null ? null : deepFreeze(structuredClone(source));
    return Object.freeze({ ...sourceControlState(snapshot),
      sourceSceneSupplied: this.#hasScene, sourceMusicStateSupplied: musicState !== null,
      providedMusicState: musicState, musicContinuation: "native-bound" as const,
      preferences: snapshot.preferences });
  }

  preferenceApi(): NativeAudioPreferences {
    return {
      read: () => readSourceAudioPreferences(this.#handle),
      apply: (player, record, unlocked) => applySourceAudioPreferences(this.#handle, player, record, unlocked),
      music: (player, value) => setSourceMusicPreferences(this.#handle, player, value),
      playlist: (player, slot) => selectSourcePlaylist(this.#handle, player, slot),
      replace: (player, slot, entries) => setSourceSavedPlaylist(this.#handle, player, slot, entries),
      edit: (player, slot, edit) => editSourceSavedPlaylist(this.#handle, player, slot, edit),
      toggle: (player, channel) => toggleSourceAudioMute(this.#handle, player, channel),
      percent: (player, channel, value) => setSourceAudioPercent(this.#handle, player, channel, value),
      skip: (player) => requestSourceMusicSkip(this.#handle, player),
    };
  }

  bindUi(bind: (handle: AudioHandle) => Promise<() => void>): Promise<() => void> {
    return bind(this.#handle);
  }

  unlock(): Promise<void> {
    // Both calls happen in the trusted handler stack, before the first await.
    this.#handle.mute(false);
    return this.#handle.unlock().catch((error: unknown) => { throw audioProblem(error); });
  }
  mute(value: boolean): void { this.#handle.mute(value); }
  volume(channel: AudioChannel, value: number): void { this.#handle.volume(channel, value); }
  setMusicState(state: SourceMusicState): void {
    try {
      this.#api.musicState(this.#handle, state);
      this.#musicState = deepFreeze(structuredClone(state));
    } catch (error) {
      this.#musicState = null;
      throw audioProblem(error);
    }
  }

  #applyScene(world: WorldView | null, scene: SourceAudioScene | null | undefined): void {
    try {
      if (world === null) {
        if (this.#hasScene) this.#api.scene(this.#handle, null);
        this.#hasScene = false;
        this.#sceneUnavailableReported = false;
      } else if (scene !== undefined) {
        this.#api.scene(this.#handle, scene);
        this.#hasScene = scene !== null;
        this.#sceneUnavailableReported = false;
      } else {
        if (this.#hasScene) this.#api.scene(this.#handle, null);
        this.#hasScene = false;
        if (!this.#sceneUnavailableReported) {
          this.#sceneUnavailableReported = true;
          this.#report(audioProblem(new AudioFailure("AUDIO_SOURCE_SCENE_REQUIRED",
            "Native audio scene input is unavailable: provide the real 128-unit listener, plane/instance/owner, placed emitters and bound original varps. No tile-centre, empty-scene or gain/distance substitute was made.")));
        }
      }
    } catch (error) {
      this.#report(audioProblem(error));
      this.#hasScene = false;
      try { this.#api.scene(this.#handle, null); }
      catch (error) { this.#report(audioProblem(error)); }
    }
  }

  update(world: WorldView | null, events: readonly AudioEvent[], scene?: SourceAudioScene | null, musicState?: SourceMusicState): void {
    const actor = world === null ? null : world.player.id;
    const changedActor = actor !== this.#actor;
    if (changedActor || world === null) this.#musicState = null;
    if (!changedActor && world !== null) this.#applyScene(world, scene);
    // One coherent batch preserves the audio-owned before/after Cook delta. A new
    // actor resets native scene/music inputs, so bind those after that first update.
    try { this.#handle.update(world, events); }
    catch (error) { throw audioProblem(error); }
    this.#actor = actor;
    if (changedActor || world === null) this.#applyScene(world, scene);
    if (world !== null && musicState !== undefined) {
      try { this.setMusicState(musicState); }
      catch (error) { this.#report(audioProblem(error)); }
    }
  }
  disconnected(): void { this.#handle.disconnected(); }
  async dispose(): Promise<void> {
    try { await this.#handle.dispose(); }
    finally { this.#stop(); this.#observations.clear(); this.#musicState = null; this.#hasScene = false; }
  }
}
