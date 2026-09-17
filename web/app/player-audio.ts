import {
  parseSourceAudioPreferences, serializeSourceAudioPreferences, sourcePreferenceUnlocks, validateSourcePreferenceUnlocks,
} from "../audio/index.ts";
import type {
  SourceAudioPreferenceBinding, SourceAudioPreferences, SourceAudioScene, SourceMusicPreferences, SourceMusicSkipResult,
  SourcePlaylistEdit, SourcePlaylistSelection, SourcePlaylistSlot, SourceSavedPlaylist, SourceVolumeChannel,
} from "../audio/index.ts";
import type { AudioEvent, WorldView } from "../shared/contracts.ts";
import { AppError, deepFreeze } from "./errors.ts";
import { audioProblem } from "./audio.ts";
import type { AudioPreferenceSave, PlayerAudioPreferenceStore } from "./player-audio-store.ts";

export interface NativeAudioPreferences {
  read(): SourceAudioPreferenceBinding | null;
  apply(playerId: string, record: SourceAudioPreferences, unlocked: readonly number[]): SourceAudioPreferenceBinding;
  music(playerId: string, value: SourceMusicPreferences): SourceAudioPreferenceBinding;
  playlist(playerId: string, slot: SourcePlaylistSelection): SourceAudioPreferenceBinding;
  replace(playerId: string, slot: SourcePlaylistSlot, entries: SourceSavedPlaylist): SourceAudioPreferenceBinding;
  edit(playerId: string, slot: SourcePlaylistSlot, edit: SourcePlaylistEdit): SourceAudioPreferenceBinding;
  toggle(playerId: string, channel: SourceVolumeChannel): SourceAudioPreferenceBinding;
  percent(playerId: string, channel: SourceVolumeChannel, value: number): SourceAudioPreferenceBinding;
  skip(playerId: string): Promise<SourceMusicSkipResult>;
}

export interface PlayerAudioRuntime {
  update(world: WorldView | null, events: readonly AudioEvent[], scene?: SourceAudioScene | null): void;
  disconnected(): void;
  preferences: NativeAudioPreferences;
}
export interface PlayerAudioPreferenceStatus {
  playerId: string | null;
  phase: "unbound" | "loading" | "loaded" | "ready" | "disconnected" | "waiting_unlocks" | "failed";
  origin: "stored" | "confirmed_absent" | null;
  save: "unchanged" | "pending" | "stored" | "failed";
  errorId: string | null;
}
export type PlayerAudioControls = Pick<PlayerAudioPreferences, "observe" | "read" | "setMusic" | "selectPlaylist"
  | "setSavedPlaylist" | "editSavedPlaylist" | "toggleMute" | "setPercent" | "skip" | "persistCurrent">;

const superseded = () => new AppError("The audio preference operation belongs to a superseded character entry.",
  { kind: "cancelled", errorId: "audio.preferences.entry_superseded" });

/** Orders storage and the existing native APIs; it never computes source defaults, slots, gains or songs. */
export class PlayerAudioPreferences {
  #store: PlayerAudioPreferenceStore;
  #runtime: PlayerAudioRuntime;
  #project: (binding: SourceAudioPreferenceBinding) => void;
  #report: (error: AppError) => void;
  #epoch = 0;
  #playerId: string | null = null;
  #record: SourceAudioPreferences | null = null;
  #loading: Promise<void> | null = null;
  #abort: AbortController | null = null;
  #failure: AppError | null = null;
  #applying = false;
  #enabled = false;
  #lastRequested = "";
  #lastBinding = "";
  #saveSequence = 0;
  #status: PlayerAudioPreferenceStatus = { playerId: null, phase: "unbound", origin: null, save: "unchanged", errorId: null };

  constructor(store: PlayerAudioPreferenceStore, runtime: PlayerAudioRuntime,
    project: (binding: SourceAudioPreferenceBinding) => void, report: (error: AppError) => void) {
    this.#store = store; this.#runtime = runtime; this.#project = project; this.#report = report;
  }

  observe(): Readonly<PlayerAudioPreferenceStatus> { return deepFreeze({ ...this.#status }); }

  read(): SourceAudioPreferenceBinding | null {
    if (!this.#enabled) return null;
    const binding = this.#runtime.preferences.read();
    return binding?.playerId === this.#playerId ? binding : null;
  }

  controls(): PlayerAudioControls {
    const epoch = this.#epoch, playerId = this.#playerId;
    const current = (): void => {
      if (epoch !== this.#epoch || playerId !== this.#playerId) throw superseded();
    };
    return Object.freeze({
      observe: () => { current(); return this.observe(); },
      read: () => { current(); return this.read(); },
      setMusic: (player, value) => { current(); return this.setMusic(player, value); },
      selectPlaylist: (player, slot) => { current(); return this.selectPlaylist(player, slot); },
      setSavedPlaylist: (player, slot, entries) => { current(); return this.setSavedPlaylist(player, slot, entries); },
      editSavedPlaylist: (player, slot, edit) => { current(); return this.editSavedPlaylist(player, slot, edit); },
      toggleMute: (player, channel) => { current(); return this.toggleMute(player, channel); },
      setPercent: (player, channel, value) => { current(); return this.setPercent(player, channel, value); },
      skip: async (player) => { current(); return this.skip(player); },
      persistCurrent: async (player) => { current(); await this.persistCurrent(player); },
    });
  }

  async prepare(playerId: string): Promise<void> {
    if (this.#playerId !== playerId) {
      this.invalidate(true);
      this.#playerId = playerId;
      this.#status = { playerId, phase: "unbound", origin: null, save: "unchanged", errorId: null };
    }
    if (this.#failure) throw this.#failure;
    if (this.#record !== null) return;
    if (this.#loading !== null) return this.#loading;
    const epoch = this.#epoch;
    const abort = new AbortController();
    this.#abort = abort;
    this.#status.phase = "loading";
    let cancel: () => void = () => {};
    const cancelled = new Promise<never>((_resolve, reject) => {
      cancel = () => reject(superseded());
      abort.signal.addEventListener("abort", cancel, { once: true });
    });
    const load = Promise.race([this.#store.load(playerId), cancelled]).then((loaded) => {
      if (epoch !== this.#epoch || this.#playerId !== playerId) throw superseded();
      this.#record = loaded.preferences;
      this.#status.origin = loaded.origin;
      this.#status.phase = "loaded";
    }).catch((error: unknown) => {
      if (epoch !== this.#epoch) throw superseded();
      const failure = error instanceof AppError ? error : new AppError("The player's source audio preferences could not be loaded.",
        { kind: "audio_preferences_read" });
      this.#failure = failure;
      this.#status.phase = "failed"; this.#status.errorId = failure.errorId;
      throw failure;
    }).finally(() => {
      abort.signal.removeEventListener("abort", cancel);
      if (epoch === this.#epoch) { this.#loading = null; this.#abort = null; }
    });
    this.#loading = load;
    return load;
  }

  readyFor(playerId: string, unlocked: readonly number[] | undefined): boolean {
    if (this.#playerId !== playerId || this.#record === null || this.#failure) return false;
    if (unlocked === undefined) {
      this.block(new AppError("Authoritative source music unlocks are unavailable. No empty list or region-derived grant was substituted.",
        { kind: "audio_preferences", errorId: "audio.preferences.source_unlocks_required" }), "waiting_unlocks");
      return false;
    }
    try {
      const actual = sourcePreferenceUnlocks(unlocked);
      validateSourcePreferenceUnlocks(this.#record, actual);
    } catch (error) {
      const problem = audioProblem(error);
      this.block(problem);
      throw problem;
    }
    return true;
  }

  commit(world: WorldView, events: readonly AudioEvent[], scene: SourceAudioScene | null | undefined,
    unlocked: readonly number[]): SourceAudioPreferenceBinding {
    if (!this.readyFor(world.player.id, unlocked) || this.#record === null) throw superseded();
    const epoch = this.#epoch;
    this.#applying = true;
    let binding: SourceAudioPreferenceBinding;
    try {
      // All three operations are synchronous; no storage/network await may split this boundary.
      this.#runtime.update(world, events, scene);
      binding = this.#runtime.preferences.apply(world.player.id, this.#record, unlocked);
      if (epoch !== this.#epoch) throw superseded();
      this.#project(binding);
      if (epoch !== this.#epoch) throw superseded();
      this.#enabled = true;
      this.#status.phase = "ready";
      if (this.#status.save !== "failed") this.#status.errorId = null;
      this.#accept(binding);
      return binding;
    } catch (error) {
      const problem = audioProblem(error);
      if (epoch === this.#epoch) this.block(problem);
      throw problem;
    } finally { this.#applying = false; }
  }

  changed(binding: SourceAudioPreferenceBinding | null): void {
    if (this.#applying || !this.#enabled || binding?.playerId !== this.#playerId) return;
    if (JSON.stringify(binding) === this.#lastBinding) return;
    const epoch = this.#epoch;
    this.#applying = true;
    try {
      this.#project(binding);
      if (epoch !== this.#epoch) throw superseded();
      this.#accept(binding);
    } catch (error) {
      if (epoch === this.#epoch) this.block(audioProblem(error));
      throw error;
    }
    finally { this.#applying = false; }
  }

  #accept(binding: SourceAudioPreferenceBinding): void {
    if (binding.playerId !== this.#playerId) throw superseded();
    this.#record = parseSourceAudioPreferences(binding.preferences);
    this.#lastBinding = JSON.stringify(binding);
    const serialized = serializeSourceAudioPreferences(this.#record);
    if (serialized === this.#lastRequested) return;
    this.#lastRequested = serialized;
    void this.#save(binding).catch((error: unknown) => this.#report(error instanceof AppError ? error
      : new AppError("Player audio preferences could not be saved.", { kind: "audio_preferences_save" })));
  }

  #save(binding: SourceAudioPreferenceBinding): Promise<AudioPreferenceSave> {
    const epoch = this.#epoch, sequence = ++this.#saveSequence;
    this.#status.save = "pending";
    return this.#store.save(binding.playerId, binding.preferences).then((receipt) => {
      if (epoch === this.#epoch && sequence === this.#saveSequence) {
        this.#status.save = "stored";
        if (this.#status.phase === "ready") this.#status.errorId = null;
      }
      return receipt;
    }, (error: unknown) => {
      if (epoch === this.#epoch && sequence === this.#saveSequence) {
        this.#status.save = "failed";
        this.#status.errorId = error instanceof AppError ? error.errorId : "audio.preferences.storage_write";
      }
      throw error;
    });
  }

  async persistCurrent(playerId: string): Promise<void> {
    const epoch = this.#require(playerId);
    const binding = this.#runtime.preferences.read();
    if (binding?.playerId !== playerId) throw superseded();
    await this.#save(binding);
    if (epoch !== this.#epoch) throw superseded();
    this.#require(playerId);
  }

  #require(playerId: string): number {
    if (!this.#enabled || playerId !== this.#playerId || this.#record === null || this.#status.phase !== "ready") {
      throw new AppError("Player audio controls are blocked until the current entry's genuine preferences and source unlocks are bound.",
        { kind: "audio_preferences", errorId: "audio.preferences.binding_required" });
    }
    return this.#epoch;
  }

  #control(playerId: string, apply: () => SourceAudioPreferenceBinding): SourceAudioPreferenceBinding {
    const epoch = this.#require(playerId);
    this.#applying = true;
    try {
      const binding = apply();
      if (epoch !== this.#epoch || binding.playerId !== playerId) throw superseded();
      try {
        this.#project(binding);
        if (epoch !== this.#epoch) throw superseded();
        this.#accept(binding);
      } catch (error) {
        if (epoch === this.#epoch) this.block(audioProblem(error));
        throw error;
      }
      return binding;
    } catch (error) { throw audioProblem(error); }
    finally { this.#applying = false; }
  }

  setMusic(playerId: string, value: SourceMusicPreferences): SourceAudioPreferenceBinding {
    return this.#control(playerId, () => this.#runtime.preferences.music(playerId, value));
  }
  selectPlaylist(playerId: string, slot: SourcePlaylistSelection): SourceAudioPreferenceBinding {
    return this.#control(playerId, () => this.#runtime.preferences.playlist(playerId, slot));
  }
  setSavedPlaylist(playerId: string, slot: SourcePlaylistSlot, entries: SourceSavedPlaylist): SourceAudioPreferenceBinding {
    return this.#control(playerId, () => this.#runtime.preferences.replace(playerId, slot, entries));
  }
  editSavedPlaylist(playerId: string, slot: SourcePlaylistSlot, edit: SourcePlaylistEdit): SourceAudioPreferenceBinding {
    return this.#control(playerId, () => this.#runtime.preferences.edit(playerId, slot, edit));
  }
  toggleMute(playerId: string, channel: SourceVolumeChannel): SourceAudioPreferenceBinding {
    return this.#control(playerId, () => this.#runtime.preferences.toggle(playerId, channel));
  }
  setPercent(playerId: string, channel: SourceVolumeChannel, value: number): SourceAudioPreferenceBinding {
    return this.#control(playerId, () => this.#runtime.preferences.percent(playerId, channel, value));
  }
  async skip(playerId: string): Promise<SourceMusicSkipResult> {
    const epoch = this.#require(playerId);
    // The native command starts real unlock/resume in this trusted call stack before its await.
    let result: SourceMusicSkipResult;
    try { result = await this.#runtime.preferences.skip(playerId); }
    catch (error) {
      if (epoch !== this.#epoch) throw superseded();
      throw audioProblem(error);
    }
    if (epoch !== this.#epoch) throw superseded();
    this.#require(playerId);
    const binding = this.#runtime.preferences.read();
    if (binding?.playerId !== playerId) throw superseded();
    this.changed(binding);
    return result;
  }

  block(error: AppError, phase: "waiting_unlocks" | "failed" = "failed"): void {
    this.invalidate(false);
    this.#status.phase = phase;
    this.#status.errorId = error.errorId;
  }

  invalidate(forget: boolean): void {
    this.#epoch++;
    this.#enabled = false;
    this.#abort?.abort();
    this.#abort = null; this.#loading = null;
    this.#runtime.disconnected();
    if (this.#status.save === "pending") {
      // A reconnect must attach its own receipt, not keep an old entry's pending status forever.
      this.#lastRequested = "";
      this.#status.save = "unchanged";
    }
    if (forget) {
      this.#playerId = null; this.#record = null; this.#failure = null; this.#lastRequested = ""; this.#lastBinding = "";
      this.#status = { playerId: null, phase: "unbound", origin: null, save: "unchanged", errorId: null };
    } else if (this.#failure === null) this.#status.phase = "disconnected";
  }

  async flush(): Promise<void> { await this.#store.flush(); }
}
