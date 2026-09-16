import {
  deserializeSourceAudioPreferences, serializeSourceAudioPreferences, sourceAudioPreferenceDefaults,
} from "../audio/preferences.ts";
import type { SourceAudioPreferences } from "../audio/preferences.ts";

export interface UiAudioPreferenceStorage {
  load(playerId: string): Promise<string | null>;
  save(playerId: string, serialized: string): Promise<void>;
}
export interface UiAudioPreferenceSaveResult { version: number; status: "saved" | "superseded" }
export interface UiAudioPreferenceSaveStatus {
  state: "idle" | "loading" | "saving" | "failed";
  dirty: boolean; desiredVersion: number; savedVersion: number; error: unknown | null;
}
interface Waiter<T> { resolve: (value: T) => void; reject: (error: unknown) => void }
interface SaveTask {
  kind: "save"; serialized: string; version: number;
  waiters: Array<Waiter<UiAudioPreferenceSaveResult> & { version: number }>;
}
type Task = SaveTask
  | ({ kind: "load" } & Waiter<SourceAudioPreferences>)
  | ({ kind: "flush" } & Waiter<void>);
interface PlayerQueue {
  tasks: Task[]; active: Task | null; running: boolean; nextVersion: number; savedVersion: number;
  desired: { serialized: string; version: number } | null; failed: boolean; error: unknown | null;
}

/** Application-lived, per-character IO ordering; storage and genuine character keys remain shell-owned. */
export class UiAudioPreferencePersistence {
  private readonly storage: UiAudioPreferenceStorage;
  private readonly players = new Map<string, PlayerQueue>();

  constructor(storage: UiAudioPreferenceStorage) { this.storage = storage; }

  private player(id: string): PlayerQueue {
    if (typeof id !== "string" || !id.length)
      throw Object.assign(new Error("A genuine player identity is required for audio preference storage."), { errorId: "ui.audio.storage.player" });
    let state = this.players.get(id);
    if (!state) {
      state = { tasks: [], active: null, running: false, nextVersion: 0, savedVersion: 0, desired: null, failed: false, error: null };
      this.players.set(id, state);
    }
    return state;
  }

  load(playerId: string): Promise<SourceAudioPreferences> {
    const state = this.player(playerId);
    if (state.failed && state.desired && state.savedVersion < state.desired.version) return Promise.reject(state.error);
    return new Promise((resolve, reject) => {
      state.tasks.push({ kind: "load", resolve, reject });
      void this.run(playerId, state);
    });
  }

  save(playerId: string, preferences: SourceAudioPreferences): Promise<UiAudioPreferenceSaveResult> {
    const serialized = serializeSourceAudioPreferences(preferences), state = this.player(playerId);
    const version = ++state.nextVersion;
    state.desired = { serialized, version };
    return this.enqueueSave(playerId, state, serialized, version);
  }

  retry(playerId: string): Promise<UiAudioPreferenceSaveResult> {
    const state = this.player(playerId);
    if (!state.desired || state.savedVersion >= state.desired.version)
      return Promise.reject(Object.assign(new Error("There are no unsaved audio preferences to retry."), { errorId: "ui.audio.storage.no_pending" }));
    return this.enqueueSave(playerId, state, state.desired.serialized, state.desired.version);
  }

  flush(playerId: string): Promise<void> {
    const state = this.player(playerId);
    if (state.failed) return Promise.reject(state.error);
    return new Promise((resolve, reject) => {
      state.tasks.push({ kind: "flush", resolve, reject });
      void this.run(playerId, state);
    });
  }

  status(playerId: string): UiAudioPreferenceSaveStatus {
    const state = this.player(playerId);
    return {
      state: state.failed ? "failed" : state.active?.kind === "load" ? "loading" : state.running ? "saving" : "idle",
      dirty: state.desired !== null && state.savedVersion < state.desired.version,
      desiredVersion: state.desired?.version ?? 0, savedVersion: state.savedVersion, error: state.error,
    };
  }

  private enqueueSave(playerId: string, state: PlayerQueue, serialized: string, version: number): Promise<UiAudioPreferenceSaveResult> {
    return new Promise((resolve, reject) => {
      const last = state.tasks.at(-1);
      if (last?.kind === "save") {
        last.serialized = serialized; last.version = version;
        last.waiters.push({ version, resolve, reject });
      } else state.tasks.push({ kind: "save", serialized, version, waiters: [{ version, resolve, reject }] });
      void this.run(playerId, state);
    });
  }

  private reject(task: Task, error: unknown): void {
    if (task.kind === "save") for (const waiter of task.waiters) waiter.reject(error);
    else task.reject(error);
  }

  private async run(playerId: string, state: PlayerQueue): Promise<void> {
    if (state.running) return;
    state.running = true;
    try {
      let task: Task | undefined;
      while ((task = state.tasks.shift())) {
        state.active = task; state.failed = false; state.error = null;
        try {
          if (task.kind === "save") {
            await this.storage.save(playerId, task.serialized);
            state.savedVersion = task.version;
            for (const waiter of task.waiters)
              waiter.resolve({ version: task.version, status: waiter.version === task.version ? "saved" : "superseded" });
          } else if (task.kind === "load") {
            const stored = await this.storage.load(playerId);
            if (stored !== null && typeof stored !== "string")
              throw Object.assign(new Error("Audio preference storage returned neither a serialized record nor confirmed absence."), { errorId: "ui.audio.storage.record" });
            task.resolve(stored === null ? sourceAudioPreferenceDefaults() : deserializeSourceAudioPreferences(stored));
          } else task.resolve();
        } catch (error) {
          state.failed = true; state.error = error;
          this.reject(task, error);
          for (const pending of state.tasks) this.reject(pending, error);
          state.tasks.length = 0;
          break;
        }
      }
    } finally { state.active = null; state.running = false; }
  }
}
