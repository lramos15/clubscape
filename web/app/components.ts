import type { CreateAudio, CreateRenderer, CreateUi } from "../shared/contracts.ts";
import { bindUiAudioPreferences, bindUiMinimapProjection, createUi, setUiMapIconSprites, setUiMinimap } from "../ui/index.ts";
import { createAudio } from "../audio/index.ts";
import { createShellRenderer } from "./renderer.ts";
import type { MapIconSprite, MinimapIconPlacements, MinimapSurface } from "../renderer/src/index.ts";
import type { UiHandle } from "../shared/contracts.ts";
import type { PlayerAudioControls } from "./player-audio.ts";

export interface Components {
  createRenderer: CreateRenderer | null; createUi: CreateUi; createAudio: CreateAudio;
  setUiMinimap?: (ui: UiHandle, surface: MinimapSurface) => void;
  setUiMapIconSprites?: (ui: UiHandle, sprites: ReadonlyMap<number, MapIconSprite>) => void;
  bindUiMinimapProjection?: (ui: UiHandle,
    project: (width: number, height: number, scale: number) => MinimapIconPlacements) => () => void;
  bindUiAudioPreferences?: (ui: UiHandle, controls: PlayerAudioControls) => () => void;
}

export async function loadComponents(): Promise<Components> {
  return { createRenderer: createShellRenderer, createUi, createAudio,
    setUiMinimap, setUiMapIconSprites, bindUiMinimapProjection, bindUiAudioPreferences };
}
