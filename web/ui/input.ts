import type { Rect } from "./assets.ts";

export interface UiAction { label: string; run: () => void; disabled?: string }
export interface Control extends Rect {
  id: string; label: string; actions: UiAction[]; disabled?: string; pressed?: boolean;
  tooltip?: string; draggableSlot?: number; focusable?: boolean;
  shiftAction?: UiAction;
  bankEntryId?: string; bankTab?: number; bankCreate?: boolean;
  productionRecipe?: string; shortcut?: string;
}
export interface InputField extends Rect {
  id: string; label: string; type: "text" | "password"; value: string; autocomplete: string;
  inputMode?: "text" | "numeric"; maximum?: number; disabled?: boolean;
  readOnly?: boolean;
  change: (value: string) => void; submit: () => void;
}
export interface InputCallbacks {
  pointer: (event: PointerEvent, control: Control | null, phase: "down" | "move" | "up") => void;
  menu: (event: MouseEvent, control: Control | null) => void;
  focus: (id: string | null) => void;
  key: (event: KeyboardEvent) => void;
  suppressClick: () => boolean;
  hover: (control: Control | null) => void;
  blocked: (reason: string) => void;
}

export class InputSurface {
  readonly root: HTMLDivElement;
  readonly live: HTMLDivElement;
  private readonly buttons = new Map<string, HTMLButtonElement>();
  private readonly fields = new Map<string, HTMLInputElement>();
  private readonly controls = new Map<string, Control>();
  private readonly abort = new AbortController();
  private readonly oldCanvasTabIndex: string | null;
  readonly canvas: HTMLCanvasElement;
  private readonly callbacks: InputCallbacks;

  constructor(canvas: HTMLCanvasElement, callbacks: InputCallbacks) {
    this.canvas = canvas; this.callbacks = callbacks;
    if (!canvas.parentElement) throw new Error("Mount the UI canvas before createUi().");
    this.oldCanvasTabIndex = canvas.getAttribute("tabindex");
    canvas.tabIndex = 0;
    this.root = document.createElement("div");
    this.root.setAttribute("role", "group");
    this.root.setAttribute("aria-label", "ClubScape interface");
    this.root.dataset.clubscapeUi = "";
    Object.assign(this.root.style, { position: "fixed", pointerEvents: "none", zIndex: "1", overflow: "hidden" });
    canvas.parentElement.append(this.root);
    this.live = document.createElement("div");
    this.live.setAttribute("role", "status"); this.live.setAttribute("aria-live", "polite"); this.live.setAttribute("aria-atomic", "true");
    Object.assign(this.live.style, { position: "absolute", width: "1px", height: "1px", overflow: "hidden", clipPath: "inset(50%)" });
    this.root.append(this.live);
    const options = { signal: this.abort.signal };
    for (const target of [canvas, this.root]) {
      target.addEventListener("pointerdown", event => callbacks.pointer(event as PointerEvent, this.controlFor(event), "down"), options);
      target.addEventListener("pointermove", event => callbacks.pointer(event as PointerEvent, this.controlFor(event), "move"), options);
      target.addEventListener("pointerup", event => callbacks.pointer(event as PointerEvent, this.controlFor(event), "up"), options);
      target.addEventListener("contextmenu", event => callbacks.menu(event as MouseEvent, this.controlFor(event)), options);
      target.addEventListener("keydown", event => callbacks.key(event as KeyboardEvent), options);
    }
    window.addEventListener("scroll", () => this.align(), { ...options, passive: true });
    window.addEventListener("resize", () => this.align(), options);
    this.align();
  }

  private controlFor(event: Event): Control | null {
    const element = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-ui-control]") : null;
    return element ? this.controls.get(element.dataset.uiControl!) ?? null : null;
  }

  align(): void {
    const rect = this.canvas.getBoundingClientRect();
    Object.assign(this.root.style, { left: `${rect.left}px`, top: `${rect.top}px`,
      width: `${rect.width}px`, height: `${rect.height}px` });
  }

  coordinates(event: Pick<MouseEvent, "clientX" | "clientY">): { x: number; y: number } {
    const bounds = this.canvas.getBoundingClientRect();
    return { x: (event.clientX - bounds.left) * this.canvas.width / bounds.width,
      y: (event.clientY - bounds.top) * this.canvas.height / bounds.height };
  }

  private position(element: HTMLElement, bounds: Rect): void {
    const rect = this.canvas.getBoundingClientRect();
    const sx = rect.width / this.canvas.width, sy = rect.height / this.canvas.height;
    Object.assign(element.style, { left: `${bounds.x * sx}px`, top: `${bounds.y * sy}px`,
      width: `${Math.max(0, bounds.width * sx)}px`, height: `${Math.max(0, bounds.height * sy)}px` });
  }

  sync(controls: readonly Control[], inputs: readonly InputField[] = []): void {
    const hadFocus = document.activeElement === this.canvas || this.root.contains(document.activeElement);
    this.controls.clear();
    const buttonIds = new Set<string>(), inputIds = new Set<string>();
    for (const control of controls) {
      if (control.width <= 0 || control.height <= 0) continue;
      buttonIds.add(control.id); this.controls.set(control.id, control);
      let button = this.buttons.get(control.id);
      if (!button) {
        button = document.createElement("button"); button.type = "button";
        button.dataset.uiControl = control.id;
        Object.assign(button.style, { position: "absolute", opacity: "0", margin: "0", padding: "0", border: "0", pointerEvents: "auto" });
        button.addEventListener("click", event => {
          if (this.callbacks.suppressClick()) { event.preventDefault(); return; }
          const current = this.controls.get(control.id);
          const action = event.shiftKey && current?.shiftAction ? current.shiftAction : current?.actions[0];
          const reason = current?.disabled ?? action?.disabled;
          if (reason) this.callbacks.blocked(reason);
          else action?.run();
        });
        button.addEventListener("focus", () => this.callbacks.focus(control.id));
        button.addEventListener("blur", () => this.callbacks.focus(null));
        button.addEventListener("pointerenter", () => this.callbacks.hover(this.controls.get(control.id) ?? null));
        button.addEventListener("pointerleave", () => this.callbacks.hover(null));
        this.root.append(button); this.buttons.set(control.id, button);
      }
      button.textContent = control.label;
      button.setAttribute("aria-label", control.label);
      button.setAttribute("aria-description", control.disabled ?? control.actions[0]?.disabled ?? control.tooltip ?? "");
      button.setAttribute("aria-disabled", String(Boolean(control.disabled ?? control.actions[0]?.disabled)));
      button.disabled = Boolean(control.disabled);
      button.tabIndex = control.focusable === false ? -1 : 0;
      if (control.pressed !== undefined) button.setAttribute("aria-pressed", String(control.pressed));
      else button.removeAttribute("aria-pressed");
      this.position(button, control);
    }
    for (const field of inputs) {
      inputIds.add(field.id);
      let element = this.fields.get(field.id);
      if (!element) {
        element = document.createElement("input");
        element.dataset.uiInput = field.id; element.spellcheck = false;
        Object.assign(element.style, { position: "absolute", opacity: "0", background: "transparent",
          margin: "0", padding: "0", border: "0", pointerEvents: "auto" });
        element.addEventListener("focus", () => this.callbacks.focus(field.id));
        element.addEventListener("blur", () => this.callbacks.focus(null));
        this.root.append(element); this.fields.set(field.id, element);
      }
      element.type = field.type; element.setAttribute("aria-label", field.label);
      element.autocomplete = field.autocomplete as AutoFill;
      element.inputMode = field.inputMode ?? "text"; element.maxLength = field.maximum ?? 255;
      element.disabled = field.disabled ?? false;
      element.readOnly = field.readOnly ?? false;
      if (element.value !== field.value) element.value = field.value;
      element.oninput = () => field.change(element!.value);
      element.onselect = () => this.callbacks.focus(field.id);
      element.onkeydown = event => {
        if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); event.stopPropagation(); field.submit(); }
      };
      this.position(element, field);
    }
    for (const [id, button] of this.buttons) if (!buttonIds.has(id)) { button.remove(); this.buttons.delete(id); }
    for (const [id, field] of this.fields) if (!inputIds.has(id)) { field.value = ""; field.remove(); this.fields.delete(id); }
    if (hadFocus && document.activeElement === document.body) this.canvas.focus({ preventScroll: true });
  }

  announce(message: string): void { if (this.live.textContent !== message) this.live.textContent = message; }
  focus(id: string): void { (this.fields.get(id) ?? this.buttons.get(id))?.focus({ preventScroll: true }); }
  activeInput(): HTMLInputElement | null {
    return document.activeElement instanceof HTMLInputElement && this.root.contains(document.activeElement) ? document.activeElement : null;
  }
  selection(id: string): [number, number] {
    const element = this.fields.get(id);
    return [element?.selectionStart ?? 0, element?.selectionEnd ?? 0];
  }
  dispose(): void {
    this.abort.abort();
    for (const field of this.fields.values()) field.value = "";
    this.fields.clear(); this.buttons.clear(); this.controls.clear(); this.root.remove();
    if (this.oldCanvasTabIndex === null) this.canvas.removeAttribute("tabindex");
    else this.canvas.setAttribute("tabindex", this.oldCanvasTabIndex);
  }
}
