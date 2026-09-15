import { invariant } from "./errors.ts";

export function presentationOptions(search: string): { earlyScene: string | null; recordedCamera: string | null } {
  const values = new URLSearchParams(search);
  for (const [key, value] of values) {
    invariant(["presentation_scene", "presentation_camera"].includes(key) && values.getAll(key).length === 1
      && /^[a-z0-9-]{1,80}$/.test(value),
    "Only explicit named source presentation diagnostics are allowed in the URL; never credentials or tokens.", "presentation");
  }
  const earlyScene = values.get("presentation_scene"), recordedCamera = values.get("presentation_camera");
  invariant(earlyScene === null || recordedCamera === null,
    "Select either a source fixture or a recorded camera over real streamed blocks, not both.", "presentation");
  return { earlyScene, recordedCamera };
}
