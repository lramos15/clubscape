import { integer, requireAudio } from "./errors.ts";

export interface QueueEntry<T> {
  readonly value: T;
  delay: number;
  dispatched: boolean;
}

/** The capacity includes dispatched entries until the following source cycle. */
export class SourceQueue<T> {
  readonly capacity = 50;
  private entries: QueueEntry<T>[] = [];

  get size(): number { return this.entries.length; }

  enqueue(value: T, delay: number): boolean {
    requireAudio(integer(delay, 0, 65535), "AUDIO_EVENT", "Invalid source client-cycle delay.");
    if (this.entries.length === this.capacity) return false;
    this.entries.push({ value, delay, dispatched: false });
    return true;
  }

  process(dispatch: (value: T) => boolean, expired?: (value: T) => void): void {
    this.entries = this.entries.filter((entry) => !entry.dispatched);
    for (const entry of this.entries) {
      entry.delay--;
      if (entry.delay < -10) {
        entry.dispatched = true;
        expired?.(entry.value);
        continue;
      }
      if (entry.delay < 0 && dispatch(entry.value)) {
        entry.dispatched = true;
        entry.delay = -100;
      }
    }
    this.entries = this.entries.filter((entry) => entry.delay >= -10 || entry.delay === -100);
  }

  values(): readonly T[] { return this.entries.filter((e) => !e.dispatched).map((e) => e.value); }
  readyForNextCycle(ready: (value: T) => boolean): boolean {
    return this.entries.every((entry) => entry.dispatched || entry.delay > 0 || ready(entry.value));
  }
  remove(predicate: (value: T) => boolean): void {
    this.entries = this.entries.filter((entry) => !predicate(entry.value));
  }
  clear(): void { this.entries = []; }
}

export class EventLedger {
  private readonly events = new Set<string>();
  private readonly cues = new Set<string>();
  private readonly limit = 100_000;

  event(id: string): boolean { return this.insert(this.events, id); }
  cue(id: string): boolean { return this.insert(this.cues, id); }
  hasEvent(id: string): boolean { return this.events.has(id); }
  hasCue(id: string): boolean { return this.cues.has(id); }
  get size(): number { return this.events.size + this.cues.size; }

  private insert(set: Set<string>, id: string): boolean {
    if (set.has(id)) return false;
    // Never evict IDs and then play old, acknowledged events as if they were new.
    requireAudio(this.size < this.limit, "AUDIO_EVENT_LEDGER_FULL",
      "Audio replay protection is full; establish a fresh authoritative event floor before replacing the handle.");
    set.add(id);
    return true;
  }
}
