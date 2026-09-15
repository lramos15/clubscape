import type { CreateAudio, CreateRenderer, CreateUi } from "../shared/contracts.ts";
import { AppError } from "./errors.ts";

export interface Components { createRenderer: CreateRenderer; createUi: CreateUi; createAudio: CreateAudio }

const modules = import.meta.glob<{ createRenderer?: CreateRenderer; createUi?: CreateUi; createAudio?: CreateAudio }>([
  "../renderer/index.ts", "../ui/index.ts", "../audio/index.ts",
]);

export async function loadComponents(): Promise<Components> {
  const renderer = modules["../renderer/index.ts"];
  const ui = modules["../ui/index.ts"];
  const audio = modules["../audio/index.ts"];
  const missing = [!renderer && "renderer", !ui && "UI", !audio && "audio"].filter(Boolean);
  if (!renderer || !ui || !audio) {
    throw new AppError(`This build is missing the real ${missing.join(", ")} component adapter(s). No substitute game was started. Integrate web/{renderer,ui,audio}/index.ts and rebuild.`, {
      kind: "integration", recoverable: false,
    });
  }
  const [renderModule, uiModule, audioModule] = await Promise.all([renderer(), ui(), audio()]);
  if (typeof renderModule.createRenderer !== "function" || typeof uiModule.createUi !== "function" || typeof audioModule.createAudio !== "function") {
    throw new AppError("The component entry points do not implement createRenderer/createUi/createAudio.", { kind: "integration", recoverable: false });
  }
  return { createRenderer: renderModule.createRenderer, createUi: uiModule.createUi, createAudio: audioModule.createAudio };
}
