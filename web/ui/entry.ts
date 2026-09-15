import type { AppState } from "../shared/contracts.ts";
import type { Control, InputField } from "./input.ts";
import type { Rect } from "./assets.ts";
import { SourceRaster, escapeText } from "./raster.ts";

export interface EntryModel {
  state: Readonly<AppState>; name: string; password: string; confirmation: string;
  focus: string | null; hideName: boolean; busy: boolean;
  error: { message: string; errorId: string | null; recoverable: boolean } | null;
  dismissedError: boolean;
}
export interface EntryActions {
  screen: (screen: "title" | "login" | "register") => void;
  submit: () => void; retry: () => void; dismiss: () => void;
  change: (field: "name" | "password" | "confirmation", value: string) => void;
  unavailable: (name: string) => void; audio: () => void; hideName: () => void;
}

export function paintTitleBackground(raster: SourceRaster, width: number): void {
  const image = raster.assets.images.get(raster.assets.catalogue.titleBackground)!;
  const pad = Math.floor((width - 765) / 2), context = raster.context;
  raster.fill({ x: 0, y: 0, width: raster.canvas.width, height: raster.canvas.height }, 0);
  context.drawImage(image, pad, 0);
  context.save(); context.translate(pad + 382 + image.naturalWidth, 0); context.scale(-1, 1);
  context.drawImage(image, 0, 0); context.restore();
  const logo = raster.assets.catalogue.sprites[498]!.frames[0]!;
  raster.sprite(498, pad + 382 - Math.floor(logo.width / 2), 18);
}

export function paintEntry(raster: SourceRaster, model: EntryModel, actions: EntryActions,
  controls: Control[], inputs: InputField[]): void {
  const width = raster.canvas.width, pad = Math.floor((width - 765) / 2), center = pad + 382;
  const box = { x: pad + 202, y: 171, width: 360, height: 200 };
  paintTitleBackground(raster, width);
  const addButton = (id: string, label: string, cx: number, top: number, run: () => void, disabled?: string) => {
    const rect = { x: cx - 73, y: top, width: 146, height: 40 };
    raster.sprite(500, rect.x, rect.y);
    raster.center(label, cx, top + 25, 496, disabled ? 0x9f9f9f : 0xffffff);
    controls.push({ ...rect, id, label, actions: [{ label, run }], ...(disabled ? { disabled } : {}) });
  };
  const sourceInput = (id: "name" | "password" | "confirmation", label: string, value: string,
    rect: Rect, baseline: number, hidden: boolean, max: number) => {
    let shown = hidden ? "*".repeat(value.length) : escapeText(value);
    while (shown && raster.measure(shown, 495) > rect.width - 8) shown = shown.slice(1);
    const focused = model.focus === id;
    raster.clip(rect, () => raster.text(shown + (focused ? "<col=ffff00>|</col>" : ""), rect.x, baseline, 495));
    inputs.push({ ...rect, id, label, value, type: id === "name" ? "text" : "password", disabled: model.busy,
      autocomplete: id === "name" ? "username" : model.state.phase === "register" ? "new-password" : "current-password",
      maximum: max, change: value => actions.change(id, value), submit: actions.submit });
  };
  const composition = (name: string, errorText?: string) => {
    const proposal = raster.assets.catalogue.proposals[name];
    if (!proposal) throw new Error(`Approved composition ${name} is missing.`);
    raster.sprite(499, box.x, box.y);
    raster.center(proposal.content.heading, center, box.y + 31, 496, 0xffff00);
    if (errorText) {
      raster.textBox(escapeText(errorText), { x: box.x + 14, y: box.y + 45, width: 332, height: 76 },
        { font: 495, lineHeight: 18, xAlign: 1, yAlign: 1 });
    } else proposal.content.lines.forEach((line, i) => raster.center(line, center, box.y + 59 + i * 21, 495));
    return proposal;
  };
  const error = model.dismissedError ? null : model.error;
  const phase = model.state.phase;
  if (phase === "login" && !error) {
    raster.sprite(499, box.x, box.y);
    const responses = model.busy ? ["Connecting to server...", "", ""] : ["", "", ""];
    responses.forEach((line, i) => raster.center(line, center, 201 + i * 15, 496, 0xffff00));
    raster.text("Login: ", center - 110, 253, 496);
    raster.text("Password: ", center - 108, 268, 496);
    sourceInput("name", "Login name", model.name, { x: center - 70, y: 240, width: 210, height: 17 }, 253, model.hideName, 255);
    sourceInput("password", "Password", model.password, { x: center - 50, y: 257, width: 190, height: 17 }, 268, true, 255);
    const remember = { x: center - 117, y: 277, width: 13, height: 13 };
    raster.sprite(697, remember.x, remember.y); raster.text("Remember username", remember.x + 18, 290, 494, 0xffff00);
    controls.push({ ...remember, width: 142, id: "remember", label: "Remember username",
      actions: [{ label: "Remember username", run: () => actions.unavailable("Persistent username storage") }] });
    const hide = { x: center + 24, y: 277, width: 13, height: 13 };
    raster.sprite(model.hideName ? 699 : 697, hide.x, hide.y); raster.text("Hide username", hide.x + 18, 290, 494, 0xffff00);
    controls.push({ ...hide, width: 122, id: "hide-name", label: "Hide username", pressed: model.hideName,
      actions: [{ label: "Hide username", run: actions.hideName }] });
    addButton("login-submit", "Login", center - 80, 301, actions.submit, model.busy ? "Waiting for the server" : undefined);
    addButton("entry-cancel", "Cancel", center + 80, 301, () => actions.screen("title"));
    raster.center("Can't login? Click here.", center, 357, 495, 0xffffff);
    controls.push({ x: center - 100, y: 346, width: 200, height: 16, id: "login-help", label: "Can't login? Click here.",
      actions: [{ label: "Login help", run: () => actions.unavailable("Account recovery") }] });
  } else if (phase === "register" && !error) {
    const proposal = raster.assets.catalogue.proposals.registration!;
    raster.sprite(499, box.x, box.y);
    raster.center(proposal.content.heading, center, 202, 496, 0xffff00);
    for (const [id, prefix, baseline, value] of [
      ["name", "Login name: ", 230, model.name],
      ["password", "Password: ", 251, model.password],
      ["confirmation", "Confirm:  ", 272, model.confirmation],
    ] as const) {
      const hidden = id !== "name", shown = hidden ? "*".repeat(value.length) : escapeText(value);
      const total = Math.min(332, raster.measure(prefix + shown, 495));
      const start = center - Math.floor(total / 2);
      raster.text(prefix, start, baseline, 495);
      sourceInput(id, id === "name" ? "Login name" : id === "password" ? "Password" : "Confirm password",
        value, { x: start + raster.measure(prefix, 495), y: baseline - 13,
          width: Math.max(140, box.x + box.width - 14 - start - raster.measure(prefix, 495)), height: 18 }, baseline, hidden, 255);
    }
    addButton("register-submit", "Create account", center - 80, 303, actions.submit, model.busy ? "Waiting for the server" : undefined);
    addButton("entry-cancel", "Cancel", center + 80, 303, () => actions.screen("title"));
  } else if (error) {
    const idText = error.errorId ? `\nError ID: ${error.errorId}` : "";
    const name = phase === "register" ? "registration-rejected" : phase === "error" && !model.state.world ? "capability" : "runtime-error";
    composition(name, `${error.message}${idText}`);
    addButton("entry-retry", phase === "register" ? "Try again" : "Retry", center - 80, 303,
      error.recoverable ? actions.retry : actions.dismiss, error.recoverable ? undefined : "The required capability is unavailable");
    addButton("entry-back", "Back", center + 80, 303, () => actions.screen("login"));
  } else if (model.state.loading && phase === "capability_check") {
    raster.center("ClubScape is loading - please wait...", center, 225, 496, 0xffffff, null);
    const progress = model.state.loading;
    if (progress.total > 0 && progress.completed >= 0 && progress.completed <= progress.total) {
      raster.border({ x: center - 152, y: 233, width: 304, height: 34 }, 9179409);
      raster.border({ x: center - 151, y: 234, width: 302, height: 32 }, 0);
      raster.fill({ x: center - 150, y: 235, width: 300, height: 30 }, 0);
      raster.fill({ x: center - 150, y: 235, width: Math.floor(300 * progress.completed / progress.total), height: 30 }, 9179409);
      raster.center(escapeText(progress.label), center, 256, 496, 0xffffff, null);
    } else raster.center(escapeText(progress.label), center, 256, 496);
  } else if (phase === "connecting" || phase === "reconnecting" || phase === "capability_check") {
    composition("connecting", model.state.loading
      ? `${model.state.loading.label}\n${model.state.loading.completed} / ${model.state.loading.total}`
      : phase === "reconnecting" ? "Connection lost.\nPlease wait - attempting to reestablish."
        : phase === "capability_check" ? "Checking required browser capabilities..." : "Connecting to the game server...");
    addButton("entry-cancel", "Cancel", center, 303, () => actions.screen("login"));
  } else {
    composition("branding");
    addButton("new-account", "New account", center - 80, 303, () => actions.screen("register"));
    addButton("existing-user", "Existing user", center + 80, 303, () => actions.screen("login"));
  }
  if (phase !== "capability_check") {
    const worldSprite = raster.assets.catalogue.namedSprites.worldSwitch;
    if (worldSprite !== undefined) raster.sprite(worldSprite, pad + 5, 463);
    raster.center("World", pad + 55, 478, 496);
    raster.center("Click to switch", pad + 55, 492, 494);
    controls.push({ x: pad + 5, y: 463, width: 100, height: 35, id: "world-switch", label: "World switch",
      actions: [{ label: "World switch", run: () => actions.unavailable("World switching") }] });
  }
  raster.sprite(811, pad + 725, 463, { frame: model.state.soundEnabled ? 0 : 1 });
  controls.push({ x: pad + 725, y: 463, width: 40, height: 40, id: "title-audio", label: model.state.soundEnabled ? "Mute sound" : "Enable sound",
    actions: [{ label: "Sound", run: actions.audio }] });
}
