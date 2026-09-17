import { mountAudio } from "./audio-fixture.ts";
import { AppError } from "../../app/errors.ts";
import type { PlayerAudioStorage } from "../../app/player-audio-store.ts";

interface PendingSave { player: string; serialized: string; resolve: () => void; reject: (error: Error) => void }

export async function mountPreferenceAudio(stored: string | null, loadFailure = false): Promise<void> {
  const player = "actor.ui_preferences_fixture";
  const records = new Map<string, string>();
  if (stored !== null) records.set(player, stored);
  const calls: Array<{ kind: "load" | "save"; player: string; serialized?: string }> = [];
  const pending: PendingSave[] = [];
  const state = {
    records, calls, pending, holdSaves: false, failNextSave: false,
    failure: (message = "Actual component storage failure.", id = "preference.storage.actual", kind = "audio_preferences_save") =>
      new AppError(message, { errorId: id, kind }),
  };
  const storage: PlayerAudioStorage = {
    async read(id) {
      calls.push({ kind: "load", player: id });
      if (loadFailure) throw state.failure("Actual component read failure.", "preference.read.actual", "audio_preferences_read");
      return records.has(id) ? records.get(id)! : null;
    },
    async write(id, serialized) {
      calls.push({ kind: "save", player: id, serialized });
      if (state.failNextSave) { state.failNextSave = false; throw state.failure(); }
      if (state.holdSaves) await new Promise<void>((resolve, reject) => pending.push({ player: id, serialized, resolve, reject }));
      records.set(id, serialized);
    },
  };
  Object.assign(window, { preferenceFixture: { ...state, state, player, ready: false } });
  await mountAudio("world", { playerId: player, storage, unlockedGroups: [2, 62, 64, 76, 144, 145, 163, 327] });
  Object.assign(window, { preferenceFixture: { ...state, state, player, ready: true } });
}
