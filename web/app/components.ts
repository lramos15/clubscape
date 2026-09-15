import type { CreateAudio, CreateRenderer, CreateUi } from "../shared/contracts.ts";
import { createUi } from "../ui/index.ts";
import { createAudio } from "../audio/index.ts";
import { AppError } from "./errors.ts";

export interface Components { createRenderer: CreateRenderer | null; createUi: CreateUi; createAudio: CreateAudio }

const renderers = import.meta.glob<{ createRenderer?: CreateRenderer }>(["../renderer/index.ts"]);

export async function loadComponents(): Promise<Components> {
  const load = renderers["../renderer/index.ts"];
  const module = load ? await load() : null;
  if (module !== null && typeof module.createRenderer !== "function") {
    throw new AppError("The renderer entry point does not implement createRenderer.", { kind: "integration", recoverable: false });
  }
  return { createRenderer: module?.createRenderer ?? null, createUi, createAudio };
}
