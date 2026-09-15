import type { ScenePick, WorldView } from "../shared/contracts.ts";

/** The current renderer resolves coverage to canonical IDs; opaque hashes are not world targets. */
export function canonicalPick(pick: ScenePick | null, world: WorldView | null): ScenePick | null {
  if (!pick) return null;
  const { x, y, plane } = pick.tile;
  if (![x, y, plane].every(Number.isSafeInteger) || x < 0 || x > 65535 || y < 0 || y > 65535 || plane < 0 || plane > 3) return null;
  if (pick.kind === "tile") return pick;
  if (!world) return null;
  if (world.entities.some((entity) => entity.id === pick.id) || world.player.id === pick.id) return pick;
  return null;
}
