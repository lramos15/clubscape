import type { RenderAssetManifest, PlayerPreviewRequest } from "../renderer/src/index.ts";
import type { UiPreviewRequest } from "../ui/index.ts";
import type { WorldView } from "../shared/contracts.ts";
import { AppError, invariant } from "./errors.ts";
import { canonicalJson } from "./identity.ts";

function unavailable(message: string): never {
  throw new AppError(message, { kind: "renderer_preview_unavailable" });
}

/** Exact coordinate/parameter adaptation only; actor selection and rendering stay native-owned. */
export function nativeUiPreviewRequest(
  request: Readonly<UiPreviewRequest>, world: WorldView | null, manifest: RenderAssetManifest,
): PlayerPreviewRequest {
  if (request.base === null || request.equipment === null || world === null) {
    unavailable("The UI preview's authoritative base/equipment metadata is unavailable. No empty loadout, guessed base, or prior actor preview was substituted.");
  }
  const base = world.ui?.appearance.base;
  if (!base || canonicalJson(base) !== canonicalJson(request.base) || base.sourceNpc !== 2063) {
    unavailable("The current renderer preview does not have the requested authoritative penguin base binding.");
  }
  if (canonicalJson(request.equipment) !== canonicalJson(world.player.equipment)) {
    unavailable("The requested preview equipment is not the current renderer actor's equipment; the world view was not rewritten.");
  }
  if (canonicalJson(request.appearance) !== canonicalJson(world.player.appearance)) {
    unavailable("The current renderer preview ABI cannot apply this local approved appearance selection independently of the world actor.");
  }
  const widget = manifest.model_widgets?.find((widget) => widget.id === request.sourceWidget);
  if (!widget) unavailable(`The renderer has no published native model parameters for UI widget ${request.sourceWidget}.`);
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
