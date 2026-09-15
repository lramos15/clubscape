import type { CreateAudio, CreateRenderer, CreateUi } from "../shared/contracts.ts";
import { createUi } from "../ui/index.ts";
import { createAudio } from "../audio/index.ts";
import { createShellRenderer } from "./renderer.ts";

export interface Components { createRenderer: CreateRenderer | null; createUi: CreateUi; createAudio: CreateAudio }

export async function loadComponents(): Promise<Components> {
  return { createRenderer: createShellRenderer, createUi, createAudio };
}
