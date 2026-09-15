import type { AppState } from "../shared/contracts.ts";
import type { Control, InputField } from "./input.ts";
import type { FontAsset, Rect } from "./assets.ts";
import { SourceRaster, escapeText, sourceLines, textAdvance } from "./raster.ts";
import type { TitleFlames } from "./flames.ts";

export interface EntryModel {
  state: Readonly<AppState>; name: string; password: string; confirmation: string;
  focus: string | null; hideName: boolean; busy: boolean;
  error: { message: string; errorId: string | null; recoverable: boolean } | null;
  dismissedError: boolean;
  errorKind?: "capability" | "runtime-error";
  cursorVisible?: boolean;
  errorPage?: number;
}
export interface EntryActions {
  screen: (screen: "title" | "login" | "register") => void;
  submit: () => void; retry: () => void; dismiss: () => void;
  change: (field: "name" | "password" | "confirmation", value: string) => void;
  unavailable: (name: string) => void; audio: () => void; hideName: () => void;
  nextErrorPage?: () => void;
}

export function entryErrorLines(text: string, width: number, font: FontAsset): string[] {
  const result: string[] = [];
  for (const line of sourceLines(escapeText(text), width, font)) {
    let part = "", advance = 0;
    for (const glyph of line.match(/<lt>|<gt>|./gu) ?? []) {
      const next = textAdvance(glyph, font);
      if (part && advance + next > width) { result.push(part); part = ""; advance = 0; }
      part += glyph; advance += next;
    }
    result.push(part);
  }
  return result;
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

/** Original client draw-loading-message path: lu.bz, native p11 font and four-pixel frame. */
export function paintReconnect(raster: SourceRaster): void {
  const text = "Connection lost<br>Please wait - attempting to reestablish";
  const lines = sourceLines(text, 250, raster.assets.catalogue.fonts[494]!);
  const width = Math.max(...lines.map(line => raster.measure(line, 494))), height = lines.length * 13;
  raster.fill({ x: 6, y: 6, width: width + 8, height: height + 8 }, 0);
  raster.border({ x: 6, y: 6, width: width + 8, height: height + 8 }, 0xffffff);
  raster.textBox(text, { x: 10, y: 10, width, height }, { font: 494, color: 0xffffff, shadow: null, xAlign: 1, yAlign: 1 });
}

export function paintEntry(raster: SourceRaster, model: EntryModel, actions: EntryActions,
  controls: Control[], inputs: InputField[], flames?: TitleFlames): void {
  const width = raster.canvas.width, pad = Math.floor((width - 765) / 2), center = pad + 382;
  const box = { x: pad + 202, y: 171, width: 360, height: 200 };
  paintTitleBackground(raster, width);
  if (!(model.state.phase === "capability_check" && model.state.loading)) flames?.paint(raster, pad);
  const addButton = (id: string, label: string, cx: number, top: number, run: () => void, disabled?: string) => {
    const rect = { x: cx - 73, y: top, width: 146, height: 40 };
    raster.sprite(500, rect.x, rect.y);
    raster.center(label, cx, top + 25, 496, disabled ? 0x9f9f9f : 0xffffff);
    controls.push({ ...rect, id, label, actions: [{ label, run }], ...(disabled ? { disabled } : {}) });
  };
  const sourceInput = (id: "name" | "password" | "confirmation", label: string, value: string,
    rect: Rect, baseline: number, hidden: boolean, max: number) => {
    let visibleValue = hidden ? "*".repeat(value.length) : value;
    while (visibleValue && raster.measure(escapeText(visibleValue), 495) > rect.width - 8) visibleValue = visibleValue.slice(1);
    const shown = escapeText(visibleValue);
    const focused = model.focus === id;
    raster.clip(rect, () => raster.text(shown + (focused && model.cursorVisible !== false ? "<col=ffff00>|</col>" : ""), rect.x, baseline, 495));
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
      const lines = entryErrorLines(errorText, 332, raster.assets.catalogue.fonts[495]!);
      const page = Math.min(model.errorPage ?? 0, Math.max(0, Math.ceil(lines.length / 3) - 1));
      lines.slice(page * 3, page * 3 + 3).forEach((line, i) => raster.center(line, center, box.y + 59 + i * 21, 495));
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
    const name = phase === "register" ? "registration-rejected" : model.errorKind ?? "runtime-error";
    const proposal = composition(name, `${error.message}${idText}`);
    const lines = entryErrorLines(`${error.message}${idText}`, 332, raster.assets.catalogue.fonts[495]!);
    const more = ((model.errorPage ?? 0) + 1) * 3 < lines.length;
    addButton(more ? "entry-error-next" : "entry-retry", more ? "Continue" : proposal.content.buttons[0]!, center - 80, 303,
      more ? () => actions.nextErrorPage?.() : actions.retry, more || error.recoverable ? undefined : "Retry is unavailable for this error.");
    addButton("entry-back", proposal.content.buttons[1]!, center + 80, 303, () => actions.screen(phase === "register" ? "title" : "login"));
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
