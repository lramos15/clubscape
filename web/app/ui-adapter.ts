import { forwardWorldPointer, onUiCameraRequest, setUiCamera } from "../ui/index.ts";
import type { WorldPointer } from "../ui/index.ts";
import type { RenderCamera, UiHandle } from "../shared/contracts.ts";

export interface UiWorldAdapter {
  pointer(handle: UiHandle, event: WorldPointer): boolean;
  camera(handle: UiHandle, camera: RenderCamera): void;
  cameraRequests(handle: UiHandle, listener: (yaw: number) => void): () => void;
}

export const sourceUiAdapter: UiWorldAdapter = {
  pointer: forwardWorldPointer,
  camera: setUiCamera,
  cameraRequests: onUiCameraRequest,
};
