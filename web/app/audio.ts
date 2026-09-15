import {
  AudioFailure, createAudio, observeAudioState, readAudioState, setSourceAudioScene, setSourceMusicSelector,
  sourceAudioDefaults, sourceSliderToMixer, sourceMixerToAssetGain,
} from "../audio/index.ts";
import type { AudioSnapshot, AudioTrace, SourceAudioScene, SourceMusicSelector } from "../audio/index.ts";
import type { AudioEvent, AudioHandle, ClientAssets, CreateAudio, WorldView } from "../shared/contracts.ts";
import type { AssetObservation } from "./assets.ts";
import type { AssetRecord } from "./manifest.ts";
import { AppError } from "./errors.ts";
import type { AudioChannel } from "./settings.ts";

export interface AudioAdapter {
  create: CreateAudio;
  read(handle: AudioHandle): AudioSnapshot;
  observe(handle: AudioHandle, listener: (state: AudioSnapshot) => void): () => void;
  scene(handle: AudioHandle, scene: SourceAudioScene | null): void;
  music(handle: AudioHandle, selector: SourceMusicSelector | null, areaMode: "modern" | "classic"): void;
}
export const sourceAudioAdapter: AudioAdapter = {
  create: createAudio, read: readAudioState, observe: observeAudioState,
  scene: setSourceAudioScene, music: setSourceMusicSelector,
};

export function sourceControlState(state: AudioSnapshot) {
  const control = (channel: AudioChannel) => {
    const percent = Math.round(state.volumes[channel] * 100);
    return Object.freeze({
      normalizedPosition: state.volumes[channel], percent,
      nativeMixer: state.nativeMixer[channel],
      lookupMixer: sourceSliderToMixer(channel, percent, state.masterPercent),
      assetCalibrationGain: sourceMixerToAssetGain(state.nativeMixer[channel]),
    });
  };
  return Object.freeze({ semantics: "native-source-slider-v1" as const, masterPercent: state.masterPercent,
    channels: Object.freeze({ music: control("music"), effects: control("effects"), area: control("area") }),
    defaults: sourceAudioDefaults() });
}

export function audioProblem(error: unknown): AppError {
  if (error instanceof AppError) return error;
  if (error instanceof AudioFailure) {
    return new AppError(`[${error.code}] ${error.message}`, {
      kind: "audio", recoverable: error.code === "AUDIO_GESTURE_REQUIRED" || error.recoverable,
    });
  }
  return new AppError("The source audio adapter failed.", { kind: "audio" });
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
  #musicSelectorBound = false;

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
    return Object.freeze({ ...sourceControlState(this.snapshot()),
      sourceSceneSupplied: this.#hasScene, musicSelectorBound: this.#musicSelectorBound });
  }

  unlock(): Promise<void> {
    // Both calls happen in the trusted handler stack, before the first await.
    this.#handle.mute(false);
    return this.#handle.unlock().catch((error: unknown) => { throw audioProblem(error); });
  }
  mute(value: boolean): void { this.#handle.mute(value); }
  volume(channel: AudioChannel, value: number): void { this.#handle.volume(channel, value); }
  musicSelector(selector: SourceMusicSelector | null, areaMode: "modern" | "classic"): void {
    this.#api.music(this.#handle, selector, areaMode);
    this.#musicSelectorBound = selector !== null;
  }
  update(world: WorldView | null, events: readonly AudioEvent[], scene?: SourceAudioScene | null): void {
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
    // Preserve each coherent committed world/event batch for the audio-owned
    // before/after Cook skill-delta gate. Never generate a completion or jingle.
    try { this.#handle.update(world, events); }
    catch (error) { this.#report(audioProblem(error)); }
  }
  disconnected(): void { this.#handle.disconnected(); }
  async dispose(): Promise<void> {
    try { await this.#handle.dispose(); }
    finally { this.#stop(); this.#observations.clear(); }
  }
}
