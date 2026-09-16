import { UiAudioPreferencePersistence } from "../audio-preference-storage.ts";
import { mountAudio } from "./audio-fixture.ts";
import { fixtureWorld } from "./component-fixture.ts";

class StorageFailure extends Error {
  readonly errorId: string;
  constructor(message: string, errorId: string) { super(message); this.errorId = errorId; }
}
interface PendingSave { player: string; serialized: string; resolve: () => void; reject: (error: Error) => void }

export async function mountPreferenceAudio(stored: string | null, loadFailure = false): Promise<void> {
  const player = fixtureWorld().player.id;
  const records = new Map<string, string>();
  if (stored !== null) records.set(player, stored);
  const calls: Array<{ kind: "load" | "save"; player: string; serialized?: string }> = [];
  const pending: PendingSave[] = [];
  const state = {
    records, calls, pending, holdSaves: false, failNextSave: false,
    failure: (message = "Actual component storage failure.", id = "preference.storage.actual") => new StorageFailure(message, id),
  };
  const persistence = new UiAudioPreferencePersistence({
    async load(id) {
      calls.push({ kind: "load", player: id });
      if (loadFailure) throw state.failure("Actual component read failure.", "preference.read.actual");
      return records.has(id) ? records.get(id)! : null;
    },
    async save(id, serialized) {
      calls.push({ kind: "save", player: id, serialized });
      if (state.failNextSave) { state.failNextSave = false; throw state.failure(); }
      if (state.holdSaves) await new Promise<void>((resolve, reject) => pending.push({ player: id, serialized, resolve, reject }));
      records.set(id, serialized);
    },
  });
  Object.assign(window, { preferenceFixture: { ...state, state, persistence, player, ready: false } });
  await mountAudio("world", { persistence, unlockedGroups: [2, 62, 64, 76, 144, 145, 163, 327] });
  Object.assign(window, { preferenceFixture: { ...state, state, persistence, player, ready: true } });
}
