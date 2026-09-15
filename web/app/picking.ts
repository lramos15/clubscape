import type { ScenePick, WorldView } from "../shared/contracts.ts";

function actorHash(id: string, player: boolean): string {
  let hash = player ? 0x100000000000n : 0x200000000000n;
  for (const byte of new TextEncoder().encode(id)) hash = BigInt.asIntN(64, hash * 31n + BigInt(byte));
  return hash.toString();
}

/** Exact initial renderer actor-hash ABI; ambiguous/unmapped source object hashes never become intents. */
export function canonicalPick(pick: ScenePick | null, world: WorldView | null): ScenePick | null {
  if (!pick || pick.kind === "tile") return pick;
  if (!world) return null;
  if (world.entities.some((entity) => entity.id === pick.id) || world.player.id === pick.id) return pick;
  const candidates = [
    { id: world.player.id, player: true },
    ...world.entities.filter((entity) => entity.kind === "npc" || entity.kind === "player")
      .map((entity) => ({ id: entity.id, player: entity.kind === "player" })),
  ].filter((entry) => actorHash(entry.id, entry.player) === pick.id);
  if (candidates.length !== 1) return null;
  return { ...pick, id: candidates[0]!.id };
}
