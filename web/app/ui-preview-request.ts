import type { RenderAssetManifest, PlayerPreviewRequest } from "../renderer/src/index.ts";
import type { UiPreviewRequest } from "../ui/index.ts";
import type { GameplayUiView, WorldView } from "../shared/contracts.ts";
import { AppError, deepFreeze, invariant } from "./errors.ts";
import { canonicalJson } from "./identity.ts";

export const PREVIEW_READBACK_SUPERSEDED = "preview.readback_superseded";

export interface PreviewWorld {
  revision: string;
  tick: string;
  player: Pick<WorldView["player"], "id" | "region" | "instance" | "appearance" | "equipment">;
  ui?: { version: 1; appearance: Pick<GameplayUiView["appearance"], "base"> };
}

interface PlayerModelMetadata {
  playerId: string;
  region: string;
  instance: string | null;
  revision: string;
  tick: string;
  appearance: WorldView["player"]["appearance"];
  equipment: WorldView["player"]["equipment"];
  base: GameplayUiView["appearance"]["base"];
}

function unavailable(message: string, errorId: string): never {
  throw new AppError(message, { kind: "renderer_preview_unavailable", errorId });
}

function metadata(world: PreviewWorld | null): PlayerModelMetadata | null {
  if (world === null) return null;
  return deepFreeze(structuredClone({
    playerId: world.player.id, region: world.player.region, instance: world.player.instance,
    revision: world.revision, tick: world.tick,
    appearance: world.player.appearance, equipment: world.player.equipment,
    base: world.ui?.version === 1 ? world.ui.appearance.base : null,
  }));
}

function modelIdentity({ playerId, region, instance, appearance, equipment, base }: PlayerModelMetadata): string {
  return canonicalJson({ playerId, region, instance, appearance, equipment, base });
}

/** Exact coordinate/parameter adaptation only; actor selection and rendering stay native-owned. */
export function nativeUiPreviewRequest(
  request: Readonly<UiPreviewRequest>, world: PreviewWorld | null, manifest: RenderAssetManifest,
): PlayerPreviewRequest {
  return sourceRequest(request, metadata(world), manifest);
}

function sourceRequest(
  request: Readonly<UiPreviewRequest>, applied: PlayerModelMetadata | null, manifest: RenderAssetManifest,
): PlayerPreviewRequest {
  if (applied === null) {
    unavailable("No authoritative player has been applied. PlayerPreviewRequest renders the current native body/gear; it has no independent draft actor/base/equipment input.",
      "preview.actor_context_required");
  }
  const base = applied.base;
  if (!base) {
    unavailable("WorldView.ui.appearance.base is unavailable for the applied player; no base or empty equipment was invented.",
      "preview.base_metadata_required");
  }
  const npc = manifest.npc_definitions?.find((entry) => entry.npc_id === base.sourceNpc);
  if (!manifest.gear_pose_fits || base.sourceNpc !== manifest.gear_pose_fits.body_npc
    || !manifest.files[manifest.gear_pose_fits.file] || !npc
    || !manifest.files[npc.base_model] || !manifest.player_reference
    || !manifest.files[manifest.player_reference.model]
    || (request.base !== null && canonicalJson(base) !== canonicalJson(request.base))) {
    unavailable("The requested source base does not match the renderer's published body/reference/pose metadata.",
      "preview.base_binding_mismatch");
  }
  if (request.equipment !== null && canonicalJson(request.equipment) !== canonicalJson(applied.equipment)) {
    unavailable("The requested preview equipment is not the applied renderer actor's equipment; the world view was not rewritten.",
      "preview.equipment_binding_mismatch");
  }
  for (const slot of applied.equipment) {
    if (slot.item === null) continue;
    const definition = manifest.equipment_items?.find((item) => item.item_id === slot.item?.sourceId);
    if (!definition || (definition.equip_model !== null && !manifest.files[definition.equip_model])) {
      unavailable(`No published equipment-model binding for ${slot.item.id} in ${slot.slot}.`,
        "preview.equipment_model_required");
    }
  }
  if (canonicalJson(request.appearance) !== canonicalJson(applied.appearance)) {
    unavailable("The current renderer preview ABI cannot apply this local approved appearance selection independently of the world actor.",
      "preview.draft_appearance_contract");
  }
  const widget = manifest.model_widgets?.find((widget) => widget.id === request.sourceWidget);
  if (!widget) unavailable(`The renderer has no published native model parameters for UI widget ${request.sourceWidget}.`,
    "preview.widget_metadata_required");
  invariant(request.modelRotation.length === 3 && request.modelRotation.every(Number.isFinite)
    && Number.isSafeInteger(request.modelZoom) && request.modelZoom > 0
    && Object.values(request.bounds).every(Number.isSafeInteger)
    && Object.values(request.modelBounds).every(Number.isSafeInteger)
    && request.modelBounds.width > 0 && request.modelBounds.height > 0,
  "The source UI preview has invalid native model parameters.", "renderer_preview");
  return {
    width: request.bounds.width, height: request.bounds.height,
    centerX: request.modelBounds.x - request.bounds.x + Math.trunc(request.modelBounds.width / 2),
    centerY: request.modelBounds.y - request.bounds.y + Math.trunc(request.modelBounds.height / 2),
    modelZoom: request.modelZoom, rotationX: request.modelRotation[0]!,
    rotationY: request.modelRotation[1]!, rotationZ: request.modelRotation[2]!,
    contentType: widget.content_type, rasterizerZoom: widget.rasterizer_zoom,
    offsetX: widget.offset_x2, offsetY: widget.offset_y2,
  };
}

/** The metadata commit follows, never precedes, the real renderer's accepted world update. */
export class PlayerModelPreviewProducer {
  #applied: PlayerModelMetadata | null = null;
  #modelIdentity: string | null = null;
  #generation = 0;
  #manifest: RenderAssetManifest;
  #frame: (request: PlayerPreviewRequest) => Promise<ImageData | null>;

  constructor(manifest: RenderAssetManifest, frame: (request: PlayerPreviewRequest) => Promise<ImageData | null>) {
    this.#manifest = manifest;
    this.#frame = frame;
  }

  accept(world: PreviewWorld, apply: () => void): void {
    const next = metadata(world);
    const identity = next === null ? null : modelIdentity(next);
    apply();
    if (identity !== this.#modelIdentity) this.#generation++;
    this.#modelIdentity = identity;
    this.#applied = next;
  }

  clear(): void {
    this.#generation++;
    this.#modelIdentity = null;
    this.#applied = null;
  }

  #requireCurrent(generation: number): void {
    if (generation !== this.#generation) {
      throw new AppError("The accepted player metadata changed during the preview readback.",
        { kind: "cancelled", errorId: PREVIEW_READBACK_SUPERSEDED });
    }
  }

  async frame(request: Readonly<UiPreviewRequest>, expectedWorld: PreviewWorld | null): Promise<ImageData | null> {
    const expected = metadata(expectedWorld);
    if (this.#applied === null || expected === null) {
      unavailable("No current accepted actor is available to the player-preview producer.", "preview.actor_context_required");
    }
    if (canonicalJson(this.#applied) !== canonicalJson(expected)) {
      unavailable("The UI preview does not belong to the renderer's accepted player snapshot.", "preview.actor_context_mismatch");
    }
    const generation = this.#generation;
    try {
      const image = await this.#frame(sourceRequest(request, this.#applied, this.#manifest));
      this.#requireCurrent(generation);
      return image;
    } catch (error) {
      this.#requireCurrent(generation);
      throw error;
    }
  }
}
