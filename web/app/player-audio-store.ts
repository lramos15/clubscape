import {
  deserializeSourceAudioPreferences, serializeSourceAudioPreferences, sourceAudioPreferenceDefaults,
} from "../audio/index.ts";
import type { SourceAudioPreferences } from "../audio/index.ts";
import { AppError } from "./errors.ts";

export const PLAYER_AUDIO_PREFERENCE_PREFIX = "clubscape.player-audio.v1.";

export interface PlayerAudioStorage {
  read(playerId: string): string | null | Promise<string | null>;
  write(playerId: string, record: string): void | Promise<void>;
}
export interface LoadedAudioPreferences {
  preferences: SourceAudioPreferences;
  origin: "stored" | "confirmed_absent";
}
export interface AudioPreferenceSave {
  status: "stored" | "superseded";
  sequence: number;
  persistedSequence: number;
}
interface Waiter {
  sequence: number;
  resolve(value: AudioPreferenceSave): void;
  reject(error: AppError): void;
}
interface WriteBatch { text: string; sequence: number; waiters: Waiter[] }
interface PlayerQueue {
  next: number;
  pending: WriteBatch | null;
  drain: Promise<void> | null;
  failure: AppError | null;
}

function playerKey(playerId: string): string {
  if (playerId.length > 160 || !/^actor\.[a-z0-9_-]+(?:\.[a-z0-9_-]+)*$/.test(playerId)) {
    throw new AppError("Invalid player identity for audio preference storage.", { kind: "audio_preferences" });
  }
  return `${PLAYER_AUDIO_PREFERENCE_PREFIX}${playerId}`;
}

export function browserPlayerAudioStorage(access: () => Pick<Storage, "getItem" | "setItem"> = () => localStorage): PlayerAudioStorage {
  return {
    read(playerId) {
      const key = playerKey(playerId);
      try { return access().getItem(key); }
      catch { throw new AppError("Player audio preferences could not be read from browser storage. No defaults were applied.",
        { kind: "audio_preferences_read", errorId: "audio.preferences.storage_read" }); }
    },
    write(playerId, record) {
      const key = playerKey(playerId);
      try { access().setItem(key, record); }
      catch { throw new AppError("Player audio preferences were applied but could not be saved to browser storage.",
        { kind: "audio_preferences_save", errorId: "audio.preferences.storage_write" }); }
    },
  };
}

/** Serial/coalesced storage only; native record validation and defaults stay with audio. */
export class PlayerAudioPreferenceStore {
  #storage: PlayerAudioStorage;
  #queues = new Map<string, PlayerQueue>();

  constructor(storage: PlayerAudioStorage) { this.#storage = storage; }

  async load(playerId: string): Promise<LoadedAudioPreferences> {
    playerKey(playerId);
    await this.flush(playerId);
    let text: string | null;
    try { text = await this.#storage.read(playerId); }
    catch (error) {
      throw error instanceof AppError ? error : new AppError("Player audio preference storage failed to return a record. No defaults were applied.",
        { kind: "audio_preferences_read", errorId: "audio.preferences.storage_read" });
    }
    if (text === null) return { preferences: sourceAudioPreferenceDefaults(), origin: "confirmed_absent" };
    if (typeof text !== "string") throw new AppError("Player audio storage returned an invalid result; absence was not confirmed.",
      { kind: "audio_preferences_read", errorId: "audio.preferences.storage_read" });
    try { return { preferences: deserializeSourceAudioPreferences(text), origin: "stored" }; }
    catch {
      // A parse error can quote stored bytes; never echo a corrupt/foreign record.
      throw new AppError("Stored player audio preferences are corrupt or unsupported. They were retained and no defaults were applied.",
        { kind: "audio_preferences_read", errorId: "audio.preferences.invalid_record" });
    }
  }

  async save(playerId: string, preferences: SourceAudioPreferences): Promise<AudioPreferenceSave> {
    playerKey(playerId);
    const text = serializeSourceAudioPreferences(preferences);
    let queue = this.#queues.get(playerId);
    if (!queue) {
      queue = { next: 0, pending: null, drain: null, failure: null };
      this.#queues.set(playerId, queue);
    }
    const sequence = ++queue.next;
    const result = new Promise<AudioPreferenceSave>((resolve, reject) => {
      const waiter: Waiter = { sequence, resolve, reject };
      if (queue.pending) {
        queue.pending.text = text;
        queue.pending.sequence = sequence;
        queue.pending.waiters.push(waiter);
      } else queue.pending = { text, sequence, waiters: [waiter] };
    });
    if (queue.drain === null) {
      this.#start(playerId, queue);
    }
    return result;
  }

  #start(playerId: string, queue: PlayerQueue): void {
    queue.failure = null;
    queue.drain = Promise.resolve().then(() => this.#drain(playerId, queue)).finally(() => {
      queue.drain = null;
      if (queue.pending !== null) this.#start(playerId, queue);
    });
  }

  async #drain(playerId: string, queue: PlayerQueue): Promise<void> {
    for (;;) {
      const batch = this.#takePending(queue);
      if (batch === null) return;
      try { await this.#storage.write(playerId, batch.text); }
      catch (error) {
        const failure = error instanceof AppError ? error : new AppError(
          "Player audio preferences were applied but their asynchronous save failed.",
          { kind: "audio_preferences_save", errorId: "audio.preferences.storage_write" });
        queue.failure = failure;
        for (const waiter of batch.waiters) waiter.reject(failure);
        const pending = this.#takePending(queue);
        if (pending !== null) for (const waiter of pending.waiters) waiter.reject(failure);
        return;
      }
      for (const waiter of batch.waiters) waiter.resolve({
        status: waiter.sequence === batch.sequence ? "stored" : "superseded",
        sequence: waiter.sequence, persistedSequence: batch.sequence,
      });
    }
  }

  #takePending(queue: PlayerQueue): WriteBatch | null {
    const batch = queue.pending;
    queue.pending = null;
    return batch;
  }

  async flush(playerId?: string): Promise<void> {
    const queues = playerId === undefined ? [...this.#queues.values()] : [this.#queues.get(playerId)].filter((queue) => queue !== undefined);
    while (queues.some((queue) => queue.drain !== null)) {
      await Promise.all(queues.map((queue) => queue.drain));
    }
    const failure = queues.find((queue) => queue.failure)?.failure;
    if (failure) throw failure;
  }
}
