import type { SourceAudioPreferenceBinding, SourceAudioScene, SourceMusicState } from "../audio/index.ts";
import type { AudioEvent, WorldView } from "../shared/contracts.ts";
import { audioProblem } from "./audio.ts";
import { AppError, deepFreeze } from "./errors.ts";
import { PlayerAudioPreferences } from "./player-audio.ts";
import type { PlayerAudioControls, PlayerAudioPreferenceStatus, PlayerAudioRuntime } from "./player-audio.ts";
import type { PlayerAudioPreferenceStore } from "./player-audio-store.ts";

export interface PlayerAudioSources {
  scene?(world: WorldView): SourceAudioScene | undefined;
  unlockedGroups?(world: WorldView): readonly number[] | undefined;
  /** Existing authority producer; only its unlocks are used, never inferred preferences. */
  music?(world: WorldView): SourceMusicState | undefined;
}
export interface PlayerAudioUi {
  bindAudio(): Promise<() => void>;
  bindPreferences?: (controls: PlayerAudioControls) => () => void;
  project(binding: SourceAudioPreferenceBinding): void;
}
export interface PlayerAudioCompositionStatus {
  preferences: Readonly<PlayerAudioPreferenceStatus>;
  uiPreferencesBound: boolean;
  sourceUnlocksSupplied: boolean;
  appliedWorld: { playerId: string; revision: string; tick: string } | null;
  issues: readonly { errorId: string; message: string }[];
}
interface PreparedAudio {
  world: WorldView;
  scene: SourceAudioScene | undefined;
  unlocked: readonly number[];
}

function cancelled(): AppError {
  return new AppError("The audio UI binding belongs to a superseded character entry.",
    { kind: "cancelled", errorId: "audio.preferences.entry_superseded" });
}
const missingUnlocks = () => new AppError(
  "Authoritative source music unlocks are unavailable. No empty list or region-derived grant was substituted.",
  { kind: "audio_preferences", errorId: "audio.preferences.source_unlocks_required" });
const missingUi = () => new AppError(
  "The real UI has not bound the native player-preference controls. World audio controls remain disconnected; legacy mute memory, guessed playlists and mode-flip Skip are not substitutes.",
  { kind: "audio_preferences", errorId: "audio.preferences.ui_adapter_required" });

/** Entry/load/UI lifetime coordination, separate from source audio policy and world authority. */
export class PlayerAudioComposition {
  #runtime: PlayerAudioRuntime;
  #ui: PlayerAudioUi;
  #sources: PlayerAudioSources;
  #preferences: PlayerAudioPreferences;
  #report: (error: AppError) => void;
  #generation = 0;
  #abort = new AbortController();
  #player: string | null = null;
  #prepared: PreparedAudio | null = null;
  #stopAudio: (() => void) | null = null;
  #stopPreferences: (() => void) | null = null;
  #binding: Promise<void> = Promise.resolve();
  #sourceUnlocks = false;
  #applied: PlayerAudioCompositionStatus["appliedWorld"] = null;
  #issues = new Map<string, AppError>();
  #reported = new Set<string>();
  #disposed = false;

  constructor(store: PlayerAudioPreferenceStore, runtime: PlayerAudioRuntime, ui: PlayerAudioUi,
    sources: PlayerAudioSources, report: (error: AppError) => void) {
    this.#runtime = runtime; this.#ui = ui; this.#sources = sources; this.#report = report;
    this.#preferences = new PlayerAudioPreferences(store, runtime, (binding) => ui.project(binding), report);
  }

  observe(): Readonly<PlayerAudioCompositionStatus> {
    return deepFreeze({
      preferences: this.#preferences.observe(), uiPreferencesBound: this.#stopPreferences !== null,
      sourceUnlocksSupplied: this.#sourceUnlocks, appliedWorld: this.#applied === null ? null : { ...this.#applied },
      issues: [...this.#issues.values()].map(({ errorId, message }) => ({ errorId, message })),
    });
  }

  controls(): PlayerAudioControls { return this.#preferences.controls(); }

  #issue(error: AppError): void {
    this.#issues.set(error.errorId, error);
    const identity = `${error.errorId}:${error.message}`;
    if (this.#reported.has(identity)) return;
    this.#reported.add(identity);
    this.#report(error);
  }

  #unbind(): void {
    this.#generation++;
    this.#abort.abort();
    this.#abort = new AbortController();
    this.#prepared = null;
    this.#stopPreferences?.();
    this.#stopPreferences = null;
    this.#stopAudio?.();
    this.#stopAudio = null;
  }

  disconnected(): void {
    this.#unbind();
    this.#preferences.invalidate(false);
  }

  async title(): Promise<void> {
    if (this.#disposed) throw cancelled();
    this.#unbind();
    this.#preferences.invalidate(true);
    this.#player = null; this.#sourceUnlocks = false; this.#applied = null;
    this.#issues.clear(); this.#reported.clear();
    this.#runtime.update(null, []);
    await this.#bind(false);
  }

  async prepare(world: WorldView): Promise<void> {
    if (this.#disposed) throw cancelled();
    this.#prepared = null;
    if (this.#player !== world.player.id) {
      this.#unbind();
      this.#preferences.invalidate(true);
      this.#player = world.player.id;
      this.#issues.clear(); this.#reported.clear(); this.#applied = null; this.#sourceUnlocks = false;
    }
    const generation = this.#generation;
    try {
      await this.#preferences.prepare(world.player.id);
      if (generation !== this.#generation || this.#disposed) throw cancelled();
      this.#issues.clear();
      const unlocked = this.#sources.unlockedGroups ? this.#sources.unlockedGroups(world)
        : this.#sources.music?.(world)?.unlockedGroups;
      this.#sourceUnlocks = unlocked !== undefined;
      const ready = this.#preferences.readyFor(world.player.id, unlocked);
      if (!ready && unlocked === undefined) this.#issue(missingUnlocks());
      if (!this.#ui.bindPreferences) this.#issue(missingUi());
      if (!ready || unlocked === undefined || !this.#ui.bindPreferences) {
        this.#unbind();
        if (ready) this.#preferences.block(missingUi());
        return;
      }
      const scene = this.#sources.scene?.(world);
      if (this.#stopPreferences === null) await this.#bind(true);
      if (generation !== this.#generation || this.#disposed) throw cancelled();
      this.#prepared = { world, scene, unlocked: Object.freeze([...unlocked]) };
      this.#reported.clear();
    } catch (error) {
      const problem = audioProblem(error);
      if (problem.kind === "cancelled" || generation !== this.#generation) throw cancelled();
      this.#unbind();
      this.#preferences.block(problem);
      this.#issue(problem);
    }
  }

  events(world: WorldView, events: readonly AudioEvent[]): void {
    const prepared = this.#prepared;
    this.#prepared = null;
    if (prepared === null) return;
    if (prepared.world !== world) {
      const error = new AppError("The prepared audio inputs do not belong to this exact authoritative world snapshot.",
        { kind: "audio_preferences", errorId: "audio.preferences.world_superseded" });
      this.#unbind(); this.#preferences.block(error); this.#issue(error);
      return;
    }
    try {
      this.#preferences.commit(world, events, prepared.scene, prepared.unlocked);
      this.#applied = { playerId: world.player.id, revision: world.revision, tick: world.tick };
    } catch (error) {
      this.#unbind();
      this.#issue(audioProblem(error));
    }
  }

  changed(binding: SourceAudioPreferenceBinding | null): void {
    try { this.#preferences.changed(binding); }
    catch (error) { this.#unbind(); this.#issue(audioProblem(error)); }
  }

  async #bind(player: boolean): Promise<void> {
    const generation = this.#generation, signal = this.#abort.signal;
    const active = () => generation === this.#generation && !this.#disposed;
    const binding = this.#binding.then(async () => {
      if (!active()) throw cancelled();
      const stopAudio = await this.#ui.bindAudio();
      if (!active()) { stopAudio(); throw cancelled(); }
      try {
        const stopPreferences = player ? this.#ui.bindPreferences?.(this.#preferences.controls()) : null;
        if (player && !stopPreferences) throw missingUi();
        if (!active()) { stopPreferences?.(); throw cancelled(); }
        this.#stopAudio = stopAudio;
        this.#stopPreferences = stopPreferences ?? null;
      } catch (error) { stopAudio(); throw error; }
    });
    // Serialize actual UI bindings: a late old dynamic import cannot replace the new entry's observer.
    this.#binding = binding.catch((error: unknown) => {
      if (active() && !(error instanceof AppError && error.kind === "cancelled")) this.#issue(audioProblem(error));
    });
    let cancel: () => void = () => {};
    const aborted = new Promise<never>((_resolve, reject) => {
      cancel = () => reject(cancelled());
      signal.addEventListener("abort", cancel, { once: true });
    });
    try { await Promise.race([binding, aborted]); }
    finally { signal.removeEventListener("abort", cancel); }
  }

  async dispose(): Promise<void> {
    if (this.#disposed) return;
    this.#disposed = true;
    this.disconnected();
    await this.#binding;
    await this.#preferences.flush();
  }
}
