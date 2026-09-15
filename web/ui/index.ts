import type {
  AppServices, AppState, ClientAssets, CreateUi, GameIntent, ItemTarget, ItemView,
  RenderCamera, ScenePick, UiHandle, WorldTarget, WorldView,
} from "../shared/contracts.ts";
import { UiAssets, contains } from "./assets.ts";
import type { NativeWidget, Rect } from "./assets.ts";
import { InputSurface } from "./input.ts";
import type { Control, InputField, UiAction } from "./input.ts";
import { SourceRaster, escapeText, plainText, sourceLines } from "./raster.ts";
import { paintEntry } from "./entry.ts";
import { frameRegions, TABS } from "./layout.ts";
import { MinimapPainter } from "./minimap.ts";
import { paintCharacter, paintGame } from "./world-view.ts";

export interface UiNotice { message: string; errorId: string | null; recoverable: boolean; scope: "error" | "unavailable" | "information" }
export interface AmountPrompt { label: string; value: string; confirm: (quantity: number) => void; pending?: boolean }
export interface ItemSelection { slot: number; id: string; instanceId: string | null; name: string }
export interface LocalUiState {
  tab: number; selectedItem: ItemSelection | null; selectedSpell: string | null;
  bankSearch: string; bankSearchOpen: boolean; bankAmount: number | "all"; bankNotes: boolean;
  shopAmount: number; shopValue: boolean; scroll: number; journal: string | null;
  modal: string | null; dialoguePage: number; amount: AmountPrompt | null;
  appearance: Record<string, number>; quickPrayer: boolean; magicFilter: boolean;
}
export interface WorldPointer {
  kind: "move" | "primary" | "context";
  x: number; y: number; pick: ScenePick | null; control?: boolean;
}
export interface GameViewContext {
  state: Readonly<AppState>; world: WorldView; local: LocalUiState;
  controls: Control[]; inputs: InputField[]; minimap: MinimapPainter;
  send: (intent: GameIntent) => void;
  openTab: (index: number) => void;
  change: (change: () => void) => void;
  notice: (message: string, scope?: UiNotice["scope"], id?: string) => void;
  unavailable: (name: string) => void;
  required: (name: string, field: string) => void;
  prompt: (label: string, confirm: (amount: number) => void) => void;
  inventoryActions: (slot: number) => UiAction[];
  equipmentActions: (slot: string) => UiAction[];
  bankActions: (slot: number) => UiAction[];
  shopActions: (index: number) => UiAction[];
  depositAll: () => void;
  logout: () => void;
  volume: (channel: "music" | "effects" | "area", value: number) => void;
  confirmAppearance: () => void;
  minimapClick: (widget: NativeWidget) => void;
  faceNorth: () => void;
}

function emptyLocal(): LocalUiState {
  return { tab: 3, selectedItem: null, selectedSpell: null, bankSearch: "", bankSearchOpen: false,
    bankAmount: 1, bankNotes: false, shopAmount: 1, shopValue: true, scroll: 0,
    journal: null, modal: null, dialoguePage: 0, amount: null, appearance: { body_type: 0 },
    quickPrayer: true, magicFilter: false };
}

function errorDetails(error: unknown): { message: string; errorId: string | null; recoverable: boolean } {
  const record = typeof error === "object" && error !== null ? error as Record<string, unknown> : {};
  return {
    message: typeof record.message === "string" ? record.message : String(error),
    errorId: typeof record.errorId === "string" ? record.errorId : typeof record.error_id === "string" ? record.error_id : null,
    recoverable: record.recoverable !== false,
  };
}

export function isInterfaceUnlocked(world: WorldView, id: string): boolean {
  if (world.player.unlockedInterfaces.includes(id)) return true;
  return !world.player.tutorialStage.startsWith("stage.tutorial.") || world.player.tutorialStage === "stage.tutorial.mainland";
}

export function readQuantity(value: string): number | null {
  if (!/^[1-9]\d*$/.test(value.trim())) return null;
  const amount = Number(value.trim());
  return Number.isSafeInteger(amount) ? amount : null;
}

const controllers = new WeakMap<UiHandle, UiController>();

/** Source UI overlay. All mutations of game/account state are requests through AppServices. */
export const createUi: CreateUi = async (canvas, services, clientAssets) => {
  let controller: UiController | null = null;
  const assets = await UiAssets.load(clientAssets, (error, id) => {
    services.report(error, id);
    controller?.show(error.message, "error", id);
  });
  controller = new UiController(canvas, services, assets);
  const handle: UiHandle = {
    update: state => controller!.update(state),
    resize: (width, height) => controller!.resize(width, height),
    capturesPointer: (x, y) => controller!.capturesPointer(x, y),
    dispose: () => { controller!.dispose(); controllers.delete(handle); },
  };
  controllers.set(handle, controller);
  return handle;
};

/** The shell forwards its actual renderer pick; UI selection never invents a world target. */
export function forwardWorldPointer(handle: UiHandle, pointer: WorldPointer): boolean {
  return controllers.get(handle)?.worldPointer(pointer) ?? false;
}

/** Call alongside RendererHandle.camera(). This is presentation state, not a contract fork. */
export function setUiCamera(handle: UiHandle, camera: RenderCamera): void {
  const controller = controllers.get(handle);
  if (controller) {
    controller.minimap.rotation = Math.round(camera.yaw * 16384 / camera.unitsPerTurn);
    controller.renderSoon();
  }
}

/** Camera commands are presentation-only; the shell applies them to its actual renderer camera. */
export function onUiCameraRequest(handle: UiHandle, listener: (yaw: number) => void): () => void {
  const controller = controllers.get(handle);
  if (!controller) throw new Error("UI handle has been disposed.");
  controller.cameraRequest = listener;
  return () => { if (controller.cameraRequest === listener) controller.cameraRequest = null; };
}

export function getUiPreviewBounds(handle: UiHandle): Readonly<Rect> | null {
  const bounds = controllers.get(handle)?.previewBounds;
  return bounds ? { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } : null;
}

/** Supply only an actual model-only renderer surface at the requested native dimensions. */
export function setUiPreview(handle: UiHandle, image: HTMLCanvasElement | OffscreenCanvas | ImageBitmap | null): void {
  const controller = controllers.get(handle);
  if (controller) { controller.preview = image; controller.renderSoon(); }
}

class UiController {
  state: Readonly<AppState>;
  local = emptyLocal();
  readonly raster: SourceRaster;
  readonly minimap: MinimapPainter;
  readonly canvas: HTMLCanvasElement;
  private readonly services: AppServices;
  readonly assets: UiAssets;
  cameraRequest: ((yaw: number) => void) | null = null;
  preview: HTMLCanvasElement | OffscreenCanvas | ImageBitmap | null = null;
  previewBounds: Rect | null = null;
  private readonly surface: InputSurface;
  private readonly unsubscribe: () => void;
  private readonly abort = new AbortController();
  private disposed = false;
  private scheduled = 0;
  private controls: Control[] = [];
  private focus: string | null = null;
  private hover: Control | null = null;
  private point = { x: 0, y: 0 };
  private menu: { x: number; y: number; width: number; actions: UiAction[] } | null = null;
  private notice: UiNotice | null = null;
  private dismissedError: AppState["error"] = null;
  private pending = new Set<string>();
  private awaitingSelectionRevision: string | null = null;
  private amountRevision: string | null = null;
  private drag: { slot: number; x: number; y: number; start: number; active: boolean; identity: ItemSelection } | null = null;
  private suppressNextClick = false;
  private name = "";
  private password = "";
  private confirmation = "";
  private hideName = false;
  private authGeneration = 0;

  constructor(canvas: HTMLCanvasElement, services: AppServices, assets: UiAssets) {
    this.canvas = canvas; this.services = services; this.assets = assets;
    this.state = services.state();
    if (this.state.world) this.local.appearance = { ...this.state.world.player.appearance };
    this.raster = new SourceRaster(canvas, assets);
    this.minimap = new MinimapPainter(this.raster);
    this.surface = new InputSurface(canvas, {
      pointer: (event, control, phase) => this.pointer(event, control, phase),
      menu: (event, control) => {
        if (!this.capturesPointer(...this.xy(event)) && !control) return;
        event.preventDefault(); event.stopPropagation();
        const actions = control?.disabled
          ? [{ label: control.label, disabled: control.disabled, run: () => {} }]
          : control?.actions ?? [{ label: "Cancel", run: () => this.cancel() }];
        this.openMenu(actions, ...this.xy(event));
      },
      focus: id => { this.focus = id; this.renderSoon(); },
      key: event => this.key(event),
      suppressClick: () => {
        const suppress = this.suppressNextClick;
        this.suppressNextClick = false;
        return suppress;
      },
      hover: control => { this.hover = control; this.renderSoon(); },
    });
    canvas.addEventListener("wheel", event => {
      const [x, y] = this.xy(event);
      if (!this.capturesPointer(x, y) || this.menu || this.notice || this.local.amount) return;
      event.preventDefault();
      this.local.scroll = Math.max(0, this.local.scroll + Math.sign(event.deltaY) * 36);
      this.renderSoon();
    }, { signal: this.abort.signal, passive: false });
    window.addEventListener("pointerup", event => {
      if (this.drag && !this.surface.root.contains(event.target as Node) && event.target !== canvas) this.pointer(event, null, "up");
    }, { signal: this.abort.signal });
    window.addEventListener("blur", () => { this.drag = null; this.menu = null; this.renderSoon(); }, { signal: this.abort.signal });
    assets.changed(() => this.renderSoon());
    this.unsubscribe = services.subscribe(state => this.update(state));
    const bounds = canvas.getBoundingClientRect();
    this.resize(bounds.width || canvas.width, bounds.height || canvas.height);
    this.update(this.state);
    requestAnimationFrame(() => {
      if (!this.disposed && (this.state.phase === "login" || this.state.phase === "register")) this.surface.focus("name");
    });
  }

  private xy(event: Pick<MouseEvent, "clientX" | "clientY">): [number, number] {
    const point = this.surface.coordinates(event);
    return [point.x, point.y];
  }

  update(state: Readonly<AppState>): void {
    if (this.disposed) return;
    const old = this.state;
    this.state = state;
    if (old.world?.player.id && old.world.player.id !== state.world?.player.id) this.local = emptyLocal();
    if (old.world?.dialogue?.id !== state.world?.dialogue?.id) this.local.dialoguePage = 0;
    if (old.world?.bank?.banker !== state.world?.bank?.banker) {
      this.local.bankSearch = ""; this.local.bankSearchOpen = false; this.local.scroll = 0; this.local.amount = null;
    }
    if (old.world?.shop?.id !== state.world?.shop?.id) { this.local.scroll = 0; this.local.amount = null; }
    const selected = this.local.selectedItem;
    if (selected && !this.sameItem(selected)) this.local.selectedItem = null;
    if (this.awaitingSelectionRevision !== null && state.world?.revision !== this.awaitingSelectionRevision) {
      this.local.selectedItem = null; this.local.selectedSpell = null; this.awaitingSelectionRevision = null;
    }
    if (this.local.amount?.pending && state.world?.revision !== this.amountRevision) {
      this.local.amount = null; this.amountRevision = null;
    }
    if (state.world && !isInterfaceUnlocked(state.world, TABS[this.local.tab]!.interface)) {
      const allowed = TABS.findIndex(tab => isInterfaceUnlocked(state.world!, tab.interface));
      this.local.tab = allowed < 0 ? 3 : allowed;
    }
    if (state.error && state.error !== old.error) {
      this.dismissedError = null;
      this.surface.announce(`${state.error.message}${state.error.errorId ? ` Error ID: ${state.error.errorId}` : ""}`);
    }
    if (state.phase === "world" || state.phase === "character") { this.password = ""; this.confirmation = ""; }
    this.renderSoon();
  }

  resize(width: number, height: number): void {
    if (this.disposed || !Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return;
    this.canvas.width = Math.round(width); this.canvas.height = Math.round(height);
    this.canvas.style.width = `${Math.round(width)}px`; this.canvas.style.height = `${Math.round(height)}px`;
    this.raster.context.imageSmoothingEnabled = false;
    this.surface.align();
    this.menu = null; this.drag = null; this.renderSoon();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true; this.authGeneration++;
    this.password = ""; this.confirmation = ""; this.name = "";
    cancelAnimationFrame(this.scheduled);
    this.unsubscribe(); this.abort.abort(); this.surface.dispose(); this.assets.dispose();
    this.local = emptyLocal(); this.pending.clear(); this.raster.dispose();
    this.controls = []; this.hover = null; this.menu = null; this.drag = null; this.notice = null;
    this.preview = null; this.previewBounds = null; this.cameraRequest = null;
  }

  renderSoon(): void {
    if (this.disposed || this.scheduled) return;
    this.scheduled = requestAnimationFrame(() => { this.scheduled = 0; if (!this.disposed) this.render(); });
  }

  show(message: string, scope: UiNotice["scope"] = "information", errorId: string | null = null): void {
    if (this.notice?.message === message && this.notice.errorId === errorId && this.notice.scope === scope) return;
    this.notice = { message, errorId, recoverable: true, scope };
    this.local.dialoguePage = 0;
    this.menu = null;
    this.surface.announce(message + (errorId ? ` Error ID: ${errorId}` : ""));
    this.renderSoon();
  }

  private unavailable(name: string): void {
    this.show(`${name} is not available in this slice. Its original control position is preserved.`, "unavailable");
  }

  private required(name: string, field: string): void {
    this.show(`${name} cannot be completed: the server interface does not supply ${field}. This is a required integration, not an out-of-scope feature.`,
      "error", `ui.contract.${field}`);
  }

  private async request(key: string, action: () => Promise<void>, selection = false): Promise<boolean> {
    if (this.disposed || this.pending.has(key)) return false;
    if (this.state.phase === "reconnecting") { this.show("Connection lost. Please wait - attempting to reestablish.", "error"); return false; }
    const revision = this.state.world?.revision ?? null;
    this.pending.add(key); this.renderSoon();
    try {
      await action();
      if (this.disposed) return false;
      if (selection) {
        if (this.state.world?.revision !== revision) { this.local.selectedItem = null; this.local.selectedSpell = null; }
        else this.awaitingSelectionRevision = revision;
      }
      return true;
    } catch (error) {
      if (!this.disposed) {
        const details = errorDetails(error);
        if (this.local.amount) this.local.amount.pending = false;
        this.show(details.message, "error", details.errorId);
      }
      return false;
    } finally { this.pending.delete(key); this.renderSoon(); }
  }

  private send(intent: GameIntent): void {
    void this.request(JSON.stringify(intent), () => this.services.send(intent),
      intent.kind === "use_item" || intent.kind === "cast");
  }

  private screen(screen: "title" | "register" | "login"): void {
    this.authGeneration++; this.notice = null; this.dismissedError = null; this.menu = null;
    this.services.setScreen(screen);
    this.renderSoon();
    requestAnimationFrame(() => { if (!this.disposed && screen !== "title") this.surface.focus("name"); });
  }

  private async submitAuth(): Promise<void> {
    if (this.pending.has("auth")) return;
    const phase = this.state.phase;
    if (phase !== "register" && phase !== "login") return;
    if (!this.name || !this.password) { this.show("Enter your login name and password.", "error", "ui.credentials.empty"); return; }
    if (phase === "register" && this.password !== this.confirmation) {
      this.show("Your passwords do not match.", "error", "ui.password.confirmation"); return;
    }
    const generation = ++this.authGeneration;
    this.pending.add("auth"); this.notice = null; this.renderSoon();
    try {
      await (phase === "register" ? this.services.register(this.name, this.password) : this.services.login(this.name, this.password));
      // The app owns the next phase. A fulfilled request never fabricates an account or a world.
    } catch (error) {
      if (!this.disposed && generation === this.authGeneration) {
        const details = errorDetails(error); this.show(details.message, "error", details.errorId);
      }
    } finally { this.pending.delete("auth"); this.renderSoon(); }
  }

  private audio(): void {
    void this.request("audio-unlock", async () => {
      await this.services.unlockAudio();
      const value = this.state.soundEnabled ? 0 : 1;
      for (const channel of ["music", "effects", "area"] as const) this.services.audioVolume(channel, value);
    });
  }

  private cancel(): void {
    if (this.menu) this.menu = null;
    else if (this.notice) this.notice = null;
    else if (this.state.error && this.dismissedError !== this.state.error) this.dismissedError = this.state.error;
    else if (this.local.amount) this.local.amount = null;
    else if (this.local.bankSearchOpen) this.local.bankSearchOpen = false;
    else if (this.local.selectedItem || this.local.selectedSpell) { this.local.selectedItem = null; this.local.selectedSpell = null; }
    else if (this.local.modal || this.local.journal || this.state.world?.bank || this.state.world?.shop || this.state.world?.recovery) {
      this.local.modal = null; this.local.journal = null; this.send({ kind: "close_interface" });
    } else if (this.state.world) this.send({ kind: "cancel_activity" });
    else this.screen("title");
    this.drag = null; this.renderSoon();
  }

  private key(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); this.cancel(); return; }
    if (this.surface.activeInput()) return;
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      const control = this.controls.find(c => c.id === this.focus);
      if (control) { event.preventDefault(); this.openMenu(control.actions, control.x, control.y + control.height); }
      return;
    }
    if (this.menu && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
      event.preventDefault();
      const current = this.controls.findIndex(c => c.id === this.focus && c.id.startsWith("menu-"));
      const entries = this.controls.filter(c => c.id.startsWith("menu-") && !c.disabled);
      const index = entries.findIndex(c => c.id === this.controls[current]?.id);
      const next = entries[(index + (event.key === "ArrowDown" ? 1 : entries.length - 1)) % entries.length];
      if (next) this.surface.focus(next.id);
      return;
    }
    if (this.notice || (this.state.error && this.state.error !== this.dismissedError)) {
      if (event.key === " " || event.key === "Enter") { event.preventDefault(); this.cancel(); }
      return;
    }
    const choice = this.state.world?.dialogue?.choices[Number(event.key) - 1];
    if (choice && /^[1-9]$/.test(event.key)) {
      event.preventDefault(); this.send({ kind: "select_dialogue", speaker: this.state.world!.dialogue!.speaker, choice: choice.id }); return;
    }
    if (event.key === " " && this.state.world?.dialogue?.choices.length === 1) {
      const continueControl = this.controls.find(c => c.id === "dialogue-continue");
      if (continueControl) { event.preventDefault(); continueControl.actions[0]?.run(); }
      return;
    }
    const tab = TABS.findIndex(tab => tab.key === event.key);
    if (tab >= 0 && this.state.world) { event.preventDefault(); this.openTab(tab); }
    else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey && this.state.world) {
      event.preventDefault(); this.required("Chat messages", "send_chat_intent");
    }
  }

  private openTab(index: number): void {
    const world = this.state.world, tab = TABS[index];
    if (!world || !tab || !isInterfaceUnlocked(world, tab.interface)) return;
    this.local.tab = index; this.local.scroll = 0; this.menu = null;
    if (this.assets.catalogue.presentation?.interfaces[tab.interface]) this.send({ kind: "open_interface", interface: tab.interface });
    else this.unavailable(tab.name);
    this.renderSoon();
  }

  private pointer(event: PointerEvent, control: Control | null, phase: "down" | "move" | "up"): void {
    const [x, y] = this.xy(event); this.point = { x, y };
    if (phase === "move") {
      if (this.drag && (event.buttons & 1) && performance.now() - this.drag.start >= 100 &&
          Math.max(Math.abs(x - this.drag.x), Math.abs(y - this.drag.y)) >= 5) this.drag.active = true;
      this.renderSoon(); return;
    }
    if (phase === "down") {
      this.suppressNextClick = false;
      if (this.menu && !control?.id.startsWith("menu-")) { this.menu = null; this.suppressNextClick = true; this.renderSoon(); return; }
      if (event.button !== 0 || this.notice || this.local.amount || control?.disabled) return;
      if (control?.draggableSlot !== undefined) {
        const item = this.inventory(control.draggableSlot);
        if (item) this.drag = { slot: control.draggableSlot, x, y, start: performance.now(), active: false,
          identity: { slot: control.draggableSlot, id: item.id, instanceId: item.instanceId, name: item.name } };
      }
    } else if (this.drag) {
      const drag = this.drag;
      this.drag = null;
      if (drag.active) {
        this.suppressNextClick = true;
        const target = this.controls.find(c => c.draggableSlot !== undefined && contains(c, x, y));
        if (target?.draggableSlot !== undefined && target.draggableSlot !== drag.slot && this.sameItem(drag.identity)) {
          this.send({ kind: "move_inventory", from: drag.slot, to: target.draggableSlot });
        }
        setTimeout(() => { this.suppressNextClick = false; }, 0);
      }
      this.renderSoon();
    }
  }

  private openMenu(actions: readonly UiAction[], x: number, y: number): void {
    const entries = actions.filter(action => plainText(action.label) !== "Cancel");
    entries.push({ label: "Cancel", run: () => this.cancel() });
    const width = Math.max(this.raster.measure("Choose Option", 496), ...entries.map(a => this.raster.measure(a.label, 496))) + 8;
    this.menu = { x: Math.max(0, Math.min(Math.floor(x), this.canvas.width - width)),
      y: Math.max(0, Math.min(Math.floor(y), this.canvas.height - (entries.length * 15 + 22))),
      width, actions: entries };
    this.renderSoon();
  }

  private inventory(slot: number): ItemView | null {
    return this.state.world?.player.inventory.find(s => s.index === slot)?.item ?? null;
  }
  private sameItem(selection: ItemSelection): boolean {
    const item = this.inventory(selection.slot);
    return item?.id === selection.id && item.instanceId === selection.instanceId;
  }
  private validSlot(slot: number, action: (item: ItemView) => void): () => void {
    const item = this.inventory(slot);
    const identity = item && { id: item.id, instanceId: item.instanceId, slot, name: item.name };
    return () => {
      if (!identity || !this.sameItem(identity)) { this.show("That inventory slot has changed. Choose the item again.", "error", "ui.selection.stale"); return; }
      action(this.inventory(slot)!);
    };
  }
  private examine(item: ItemView): void {
    const text = item.sourceId === null ? null : this.assets.catalogue.items[item.sourceId]?.examine;
    this.show(`${text || item.name}${item.quantity !== 1 ? ` (${item.quantity.toLocaleString("en-US")})` : ""}`);
  }
  private useSelection(target: ItemTarget): void {
    const item = this.local.selectedItem;
    if (!item || !this.sameItem(item)) { this.local.selectedItem = null; this.show("The selected item is no longer in that slot.", "error", "ui.selection.stale"); return; }
    this.send({ kind: "use_item", inventory_slot: item.slot, target });
  }
  private inventoryActions(slot: number): UiAction[] {
    const item = this.inventory(slot), world = this.state.world;
    if (!item || !world) return [];
    const label = `<col=ff9040>${escapeText(item.name)}</col>`;
    if (this.local.selectedItem) return [
      { label: `Use ${escapeText(this.local.selectedItem.name)} -> ${label}`, run: this.validSlot(slot, () => this.useSelection({ kind: "inventory", slot })) },
      { label: `Examine ${label}`, run: () => this.examine(item) },
    ];
    if (world.bank) {
      const bank = world.bank;
      const deposit = (quantity: number) => this.validSlot(slot, () => this.send({ kind: "bank_deposit", banker: bank.banker, inventory_slot: slot, quantity }))();
      const options: UiAction[] = [1, 5, 10].map(q => ({ label: `Deposit-${q} ${label}`, run: () => deposit(q) }));
      options.push({ label: `Deposit-X ${label}`, run: () => this.prompt("Enter amount:", deposit) },
        { label: `Deposit-All ${label}`, run: this.validSlot(slot, item => deposit(item.quantity)) },
        { label: `Examine ${label}`, run: () => this.examine(item) });
      const selected = this.local.bankAmount;
      return [{ label: `Deposit-${selected === "all" ? "All" : selected} ${label}`,
        run: this.validSlot(slot, item => deposit(selected === "all" ? item.quantity : selected)) },
      ...options.filter(o => !o.label.startsWith(`Deposit-${selected} `))];
    }
    if (world.shop) {
      const shop = world.shop;
      const sell = (quantity: number) => this.validSlot(slot, () => this.send({ kind: "shop_sell", shop: shop.id, inventory_slot: slot, quantity }))();
      const value = () => {
        const row = this.state.world?.shop?.rows.find(row => row.item.id === item.id);
        this.show(row?.sellPrice == null ? "The server has not supplied a sell price for this item." : `${item.name}: this shop will buy for ${row.sellPrice} coins.`);
      };
      return [{ label: this.local.shopValue ? `Value ${label}` : `Sell-${this.local.shopAmount} ${label}`,
        run: this.local.shopValue ? value : () => sell(this.local.shopAmount) },
      ...[1, 5, 10, 50].map(q => ({ label: `Sell-${q} ${label}`, run: () => sell(q) })),
      { label: `Sell-X ${label}`, run: () => this.prompt("Enter amount:", sell) },
      { label: `Examine ${label}`, run: () => this.examine(item) }];
    }
    const actions: UiAction[] = [];
    for (const option of item.actions) {
      const lower = option.toLowerCase();
      if (lower === "drop" || lower === "use" || lower === "examine") continue;
      actions.push({ label: `${escapeText(option)} ${label}`, run: this.validSlot(slot, () => {
        if (["wield", "wear", "equip"].includes(lower)) this.send({ kind: "equip", inventory_slot: slot });
        else if (lower === "eat") this.send({ kind: "eat", inventory_slot: slot });
        else this.required(`${option} ${item.name}`, "item_action");
      }) });
    }
    actions.push({ label: `Use ${label}`, run: this.validSlot(slot, item => {
      this.local.selectedItem = { slot, id: item.id, instanceId: item.instanceId, name: item.name };
      this.local.selectedSpell = null; this.renderSoon();
    }) });
    if (item.actions.some(a => a.toLowerCase() === "drop")) actions.push({
      label: `Drop ${label}`, run: this.validSlot(slot, item => this.send({ kind: "drop", inventory_slot: slot, quantity: item.quantity })),
    });
    actions.push({ label: `Examine ${label}`, run: () => this.examine(item) });
    return actions;
  }

  private equipmentActions(slot: string): UiAction[] {
    const item = this.state.world?.player.equipment.find(e => e.slot === slot)?.item;
    if (!item) return [];
    return [{ label: `Remove <col=ff9040>${escapeText(item.name)}</col>`, run: () => {
      const current = this.state.world?.player.equipment.find(e => e.slot === slot)?.item;
      if (current?.id !== item.id || current.instanceId !== item.instanceId) this.show("That equipment slot has changed.", "error", "ui.equipment.stale");
      else this.send({ kind: "unequip", slot });
    } }, { label: `Examine <col=ff9040>${escapeText(item.name)}</col>`, run: () => this.examine(item) }];
  }

  private bankActions(slot: number): UiAction[] {
    const bank = this.state.world?.bank, item = bank?.slots.find(s => s.index === slot)?.item;
    if (!bank || !item) return [];
    const label = `<col=ff9040>${escapeText(item.name)}</col>`;
    const withdraw = (quantity: number) => {
      const current = this.state.world?.bank;
      const found = current?.slots.find(s => s.index === slot)?.item;
      if (current?.banker !== bank.banker || found?.id !== item.id) {
        this.show("That bank slot has changed. Choose the item again.", "error", "ui.bank.stale"); return;
      }
      this.send({ kind: "bank_withdraw", banker: bank.banker, bank_slot: slot, quantity, noted: this.local.bankNotes });
    };
    const currentAmount = this.local.bankAmount;
    return [{ label: `Withdraw-${currentAmount === "all" ? "All" : currentAmount} ${label}`, run: () => withdraw(currentAmount === "all" ? item.quantity : currentAmount) },
      ...[1, 5, 10].filter(q => q !== currentAmount).map(q => ({ label: `Withdraw-${q} ${label}`, run: () => withdraw(q) })),
      { label: `Withdraw-X ${label}`, run: () => this.prompt("Enter amount:", withdraw) },
      { label: `Withdraw-All ${label}`, run: () => withdraw(this.state.world?.bank?.slots.find(s => s.index === slot)?.item?.quantity ?? 0) },
      { label: `Withdraw-All-but-1 ${label}`, run: () => withdraw(Math.max(0, (this.state.world?.bank?.slots.find(s => s.index === slot)?.item?.quantity ?? 0) - 1)),
        ...(item.quantity <= 1 ? { disabled: "There is only one item in this stack." } : {}) },
      { label: `Placeholder ${label}`, run: () => this.required("Bank placeholders", "bank_placeholder_intent") },
      { label: `Examine ${label}`, run: () => this.examine(item) }];
  }

  private shopActions(index: number): UiAction[] {
    const shop = this.state.world?.shop, row = shop?.rows.find(row => row.index === index);
    if (!shop || !row) return [];
    // A held menu/amount prompt must not acquire the identity of a reused row.
    const displayedItem = row.item.id;
    const label = `<col=ff9040>${escapeText(row.item.name)}</col>`;
    const currentRow = () => {
      const current = this.state.world?.shop?.rows.find(r => r.index === index);
      if (this.state.world?.shop?.id !== shop.id || current?.item.id !== displayedItem) {
        this.show("The shop stock has changed. Choose the current item again.", "error", "ui.shop.stale");
        return null;
      }
      return current;
    };
    const value = () => {
      const latest = currentRow();
      if (!latest) return;
      this.show(latest.buyPrice === null ? "The server has not supplied a current buy price."
        : `${latest.item.name}: currently costs ${latest.buyPrice} coins.`);
    };
    const buy = (quantity: number) => {
      if (!currentRow()) return;
      this.send({ kind: "shop_buy", shop: shop.id, item_index: index, quantity, expected_item: displayedItem });
    };
    const disabled = row.stock <= 0 ? { disabled: "This item is out of stock." } : {};
    const actions: UiAction[] = [{ label: this.local.shopValue ? `Value ${label}` : `Buy-${this.local.shopAmount} ${label}`,
      run: this.local.shopValue ? value : () => buy(this.local.shopAmount), ...(this.local.shopValue ? {} : disabled) },
    ...[1, 5, 10, 50].map(q => ({ label: `Buy-${q} ${label}`, run: () => buy(q), ...disabled })),
    { label: `Buy-X ${label}`, run: () => this.prompt("Enter amount:", buy), ...disabled },
    { label: `Value ${label}`, run: value }, { label: `Examine ${label}`, run: () => this.examine(row.item) }];
    return actions.filter((action, at) => actions.findIndex(other => other.label === action.label) === at);
  }

  private prompt(label: string, confirm: (quantity: number) => void): void {
    this.local.amount = { label, value: "", confirm }; this.menu = null; this.renderSoon();
    requestAnimationFrame(() => { if (!this.disposed) this.surface.focus("amount"); });
  }
  private confirmAmount(): void {
    const prompt = this.local.amount;
    if (!prompt || prompt.pending) return;
    const quantity = readQuantity(prompt.value);
    if (quantity === null) { this.show("Enter a positive whole number.", "error", "ui.quantity.invalid"); return; }
    const pending = this.pending.size;
    this.amountRevision = this.state.world?.revision ?? null;
    prompt.confirm(quantity);
    if (this.pending.size > pending) prompt.pending = true;
    else this.local.amount = null;
    this.renderSoon();
  }
  private async depositAll(): Promise<void> {
    const bank = this.state.world?.bank;
    if (!bank) return;
    const slots = this.state.world!.player.inventory.filter(s => s.item).map(s => ({ index: s.index, id: s.item!.id }));
    for (const slot of slots) {
      if (this.state.world?.bank?.banker !== bank.banker) { this.show("The bank has closed.", "error", "ui.bank.closed"); return; }
      const item = this.inventory(slot.index);
      if (!item || item.id !== slot.id) { this.show("The inventory changed during deposit.", "error", "ui.inventory.changed"); return; }
      if (!await this.request(`deposit-all-${slot.index}`, () => this.services.send({
        kind: "bank_deposit", banker: bank.banker, inventory_slot: slot.index, quantity: item.quantity,
      }))) return;
    }
  }

  capturesPointer(x: number, y: number): boolean {
    if (this.disposed) return false;
    if (!this.state.world || this.state.phase !== "world" || this.notice || this.local.amount ||
        (this.state.error && this.state.error !== this.dismissedError)) return true;
    if (this.menu && contains({ ...this.menu, height: this.menu.actions.length * 15 + 22 }, x, y)) return true;
    return Object.values(frameRegions(this.canvas.width, this.canvas.height)).some(rect => contains(rect, x, y)) ||
      this.controls.some(control => contains(control, x, y));
  }

  worldPointer(pointer: WorldPointer): boolean {
    const world = this.state.world;
    if (!world || this.capturesPointer(pointer.x, pointer.y)) return true;
    const pick = pointer.pick;
    if (!pick) return false;
    const entity = pick.kind === "entity" ? world.entities.find(e => e.id === pick.id) : null;
    const target: WorldTarget | null = entity ? entity.kind === "temporary_object"
      ? { kind: "temporary_object", object: entity.id } : { kind: "spawn", spawn: entity.id } : null;
    let actions: UiAction[] = [];
    if (this.local.selectedItem && target) {
      const useTarget: ItemTarget = target.kind === "temporary_object"
        ? target : { kind: "world", spawn: target.spawn };
      actions.push({ label: `Use ${escapeText(this.local.selectedItem.name)} -> <col=ffff00>${escapeText(entity!.name)}</col>`,
        run: () => this.useSelection(useTarget), ...(entity!.available ? {} : { disabled: "That target is unavailable." }) });
    } else if (this.local.selectedSpell && entity) {
      const spell = this.local.selectedSpell;
      actions.push({ label: `Cast ${spell === "spell.wind_strike" ? "Wind Strike" : escapeText(spell)} -> <col=ffff00>${escapeText(entity.name)}</col>`,
        run: () => this.send({ kind: "cast", spell, target: entity.id }), ...(entity.available ? {} : { disabled: "That target is unavailable." }) });
    } else if (entity && target) {
      actions = entity.actions.map(action => ({ label: `${escapeText(action.name)} <col=ffff00>${escapeText(entity.name)}</col>`,
        run: () => {
          const current = this.state.world?.entities.find(e => e.id === entity.id);
          if (!current?.available || !current.actions.find(a => a.name === action.name)?.allowed) {
            this.show(current?.actions.find(a => a.name === action.name)?.reason ?? "That target is no longer available.", "error", "ui.target.changed"); return;
          }
          this.send({ kind: "interact_with", target, action: action.name });
        }, ...(!action.allowed || !entity.available ? { disabled: action.reason ?? "That action is unavailable." } : {}) }));
      if (entity.sourceId !== null) {
        const text = this.assets.catalogue.npcs[entity.sourceId]?.examine;
        if (text) actions.push({ label: `Examine <col=ffff00>${escapeText(entity.name)}</col>`, run: () => this.show(text) });
      }
    }
    for (const ground of world.groundItems.filter(g => g.tile.x === pick.tile.x && g.tile.y === pick.tile.y && g.tile.plane === pick.tile.plane)) {
      actions.push({ label: `${this.local.selectedItem ? "Use item ->" : "Take"} <col=ff9040>${escapeText(ground.item.name)}</col>`,
        run: () => this.local.selectedItem ? this.useSelection({ kind: "ground", ground_item_id: ground.id })
          : this.send({ kind: "take_ground_item", ground_item_id: ground.id }),
        ...(ground.canTake ? {} : { disabled: "You cannot take this item yet." }) });
    }
    const running = world.player.settings.find(setting => setting.setting === "run")?.enabled ?? false;
    actions.push({ label: "Walk here", run: () => this.send({ kind: "walk", destination: pick.tile, running: pointer.control ? !running : running }) });
    if (pointer.kind === "context") this.openMenu(actions, pointer.x, pointer.y);
    else if (pointer.kind === "primary") {
      const first = actions[0];
      if (first?.disabled) this.show(first.disabled, "error");
      else first?.run();
    } else {
      this.point = { x: pointer.x, y: pointer.y };
      this.hover = { id: "world", label: plainText(actions[0]?.label ?? ""), actions, x: pointer.x, y: pointer.y, width: 0, height: 0 };
      this.renderSoon();
    }
    return true;
  }

  private render(): void {
    const controls: Control[] = [], inputs: InputField[] = [];
    this.raster.clear();
    const world = this.state.world;
    const error = this.notice?.scope === "error" ? this.notice : this.state.error;
    this.previewBounds = null;
    const character = this.state.phase === "character" || world?.player.tutorialStage === "stage.tutorial.appearance";
    if (!world && character) {
      this.previewBounds = paintCharacter(this.raster, this.local.appearance, controls,
        body => { this.local.appearance.body_type = body; this.renderSoon(); },
        () => { void this.request("appearance", () => this.services.createCharacter({ ...this.local.appearance })); },
        (label, field) => this.required(label, field));
      if (this.preview && this.previewBounds) {
        const bounds = this.previewBounds;
        if (this.preview.width === bounds.width && this.preview.height === bounds.height)
          this.raster.clip(bounds, () => this.raster.context.drawImage(this.preview!, bounds.x, bounds.y));
        else this.show("The renderer's character preview does not match the native preview dimensions.", "error", "ui.preview.size");
      }
    } else if (!world || (this.state.phase !== "world" && this.state.phase !== "reconnecting")) {
      paintEntry(this.raster, { state: this.state, name: this.name, password: this.password, confirmation: this.confirmation,
        focus: this.focus, hideName: this.hideName, busy: this.pending.has("auth"), error,
        dismissedError: error !== this.notice && this.dismissedError === this.state.error },
      { screen: screen => this.screen(screen), submit: () => { void this.submitAuth(); },
        change: (field, value) => { this[field] = value; this.renderSoon(); },
        retry: () => {
          void this.request("retry", async () => {
            await this.assets.retryFailed();
            this.notice = null; this.dismissedError = this.state.error;
            if (this.state.phase === "error") await this.services.enterWorld();
          });
        },
        dismiss: () => this.cancel(), audio: () => this.audio(), hideName: () => { this.hideName = !this.hideName; this.renderSoon(); },
        unavailable: name => this.unavailable(name),
      }, controls, inputs);
    } else {
      paintGame(this.raster, {
        state: this.state, world, local: this.local, controls, inputs, minimap: this.minimap,
        send: intent => this.send(intent), openTab: tab => this.openTab(tab),
        change: change => { change(); this.renderSoon(); },
        notice: (message, scope = "information", id) => this.show(message, scope, id),
        unavailable: name => this.unavailable(name), required: (name, field) => this.required(name, field),
        prompt: (label, confirm) => this.prompt(label, confirm),
        inventoryActions: slot => this.inventoryActions(slot), equipmentActions: slot => this.equipmentActions(slot),
        bankActions: slot => this.bankActions(slot), shopActions: slot => this.shopActions(slot),
        depositAll: () => { void this.request("deposit-inventory", () => this.depositAll()); },
        logout: () => { void this.request("logout", () => this.services.logout()); },
        volume: (channel, value) => {
          void this.request(`volume-${channel}`, async () => { await this.services.unlockAudio(); this.services.audioVolume(channel, value); });
        },
        confirmAppearance: () => { void this.request("appearance", () => this.services.createCharacter({ ...this.local.appearance })); },
        minimapClick: widget => {
          const destination = this.minimap.destination(widget, world.player.tile, this.point.x, this.point.y);
          if (destination) this.send({ kind: "walk", destination, running: world.player.settings.find(s => s.setting === "run")?.enabled ?? false });
        },
        faceNorth: () => this.cameraRequest ? this.cameraRequest(0) : this.required("Camera rotation", "camera_request_adapter"),
      });
      if (character) {
        const characterControls: Control[] = [];
        this.previewBounds = paintCharacter(this.raster, this.local.appearance, characterControls,
          body => { this.local.appearance.body_type = body; this.renderSoon(); },
          () => { void this.request("appearance", () => this.services.createCharacter({ ...this.local.appearance })); },
          (label, field) => this.required(label, field));
        const hudControls = controls.filter(control => !control.id.startsWith("appearance-"));
        controls.length = 0; controls.push(...hudControls, ...characterControls);
        if (this.preview && this.previewBounds) {
          const bounds = this.previewBounds;
          if (this.preview.width === bounds.width && this.preview.height === bounds.height)
            this.raster.clip(bounds, () => this.raster.context.drawImage(this.preview!, bounds.x, bounds.y));
        }
      }
    }
    if (this.drag?.active) {
      const item = this.inventory(this.drag.slot);
      if (item?.sourceId !== null && item?.sourceId !== undefined) this.raster.item(item.sourceId, item.quantity, this.point.x - 18, this.point.y - 16, 2, false, 128);
    }
    if (this.local.amount && world) {
      controls.length = 0; inputs.length = 0;
      this.paintChatOverlay(this.local.amount.label, "", controls);
      const rect = frameRegions(this.canvas.width, this.canvas.height).chat;
      this.raster.center(escapeText(this.local.amount.value) + "<col=0000ff>*</col>", rect.x + 259, rect.y + 88, 496, 0, null);
      inputs.push({ id: "amount", label: this.local.amount.label, x: rect.x + 130, y: rect.y + 68, width: 260, height: 26,
        type: "text", inputMode: "numeric", autocomplete: "off", maximum: 16, value: this.local.amount.value,
        change: value => { if (this.local.amount) this.local.amount.value = value; this.renderSoon(); }, submit: () => this.confirmAmount() });
      controls.push({ id: "amount-confirm", label: "Confirm amount", x: rect.x + 140, y: rect.y + 105, width: 239, height: 22,
        actions: [{ label: "Confirm amount", run: () => this.confirmAmount() }] });
      this.raster.center("Press Enter to confirm", rect.x + 259, rect.y + 121, 495, 0x0000ff, null);
    }
    const visibleError = this.state.error && this.state.error !== this.dismissedError ? this.state.error : null;
    const overlay = this.notice || (world || character ? visibleError : null);
    if (overlay && (world || character || this.notice?.scope !== "error")) {
      controls.length = 0; inputs.length = 0;
      if (world) this.paintChatOverlay(overlay.errorId ? `${overlay.message}\nError ID: ${overlay.errorId}` : overlay.message, "Click here to continue", controls);
      else {
        const pad = Math.floor((this.canvas.width - 765) / 2), x = pad + 202;
        this.raster.sprite(499, x, 171);
        this.raster.center(this.notice?.scope === "unavailable" ? "Not available in this slice" : "ClubScape", x + 180, 202, 496, 0xffff00);
        this.raster.textBox(escapeText(overlay.message), { x: x + 14, y: 216, width: 332, height: 70 }, { font: 495, lineHeight: 18, xAlign: 1, yAlign: 1 });
        this.raster.sprite(500, x + 107, 303); this.raster.center("Back", x + 180, 328, 496);
        controls.push({ id: "notice-close", label: "Back", x: x + 107, y: 303, width: 146, height: 40,
          actions: [{ label: "Back", run: () => this.cancel() }] });
      }
    }
    if (world && this.state.phase === "reconnecting") {
      controls.length = 0; inputs.length = 0;
      const rect = { x: 0, y: 0, width: 259, height: 45 };
      this.raster.fill(rect, 0); this.raster.border({ x: 0, y: 0, width: 258, height: 44 }, 0xffffff);
      this.raster.text("Connection lost", 6, 16, 496);
      this.raster.text("Please wait - attempting to reestablish", 6, 32, 495);
    }
    if (this.menu) {
      const menu = this.menu, rect = { x: menu.x, y: menu.y, width: menu.width, height: menu.actions.length * 15 + 22 };
      this.raster.fill(rect, 0x5d5447); this.raster.fill({ x: rect.x + 1, y: rect.y + 1, width: rect.width - 2, height: 16 }, 0);
      this.raster.border({ x: rect.x + 1, y: rect.y + 18, width: rect.width - 2, height: rect.height - 19 }, 0);
      this.raster.text("Choose Option", menu.x + 3, menu.y + 14, 496, 0x5d5447);
      controls.length = 0; inputs.length = 0;
      menu.actions.forEach((action, index) => {
        const bounds = { x: menu.x + 2, y: menu.y + 19 + index * 15, width: menu.width - 4, height: 15 };
        const active = contains(bounds, this.point.x, this.point.y) || this.focus === `menu-${index}`;
        this.raster.text(action.label, bounds.x + 1, bounds.y + 12, 496, action.disabled ? 0xaaaaaa : active ? 0xffff00 : 0xffffff);
        controls.push({ ...bounds, id: `menu-${index}`, label: plainText(action.label), ...(action.disabled ? { disabled: action.disabled } : {}),
          actions: [{ label: action.label, run: () => { this.menu = null; action.run(); this.renderSoon(); } }] });
      });
    } else if (this.hover?.tooltip && !overlay && !this.local.amount) {
      const text = this.hover.tooltip, width = Math.min(310, Math.max(...text.split("\n").map(line => this.raster.measure(line, 494))) + 8);
      const lines = sourceLines(text, width - 8, this.assets.catalogue.fonts[494]!);
      const rect = { x: Math.max(0, Math.min(this.point.x + 10, this.canvas.width - width)), y: Math.max(0, Math.min(this.point.y + 20, this.canvas.height - lines.length * 12 - 8)), width, height: lines.length * 12 + 8 };
      this.raster.fill(rect, 0xffffa0); this.raster.border(rect, 0);
      lines.forEach((line, i) => this.raster.text(line, rect.x + 4, rect.y + 12 + i * 12, 494, 0, null));
    }
    const focused = controls.find(c => c.id === this.focus);
    if (focused && !this.menu) this.raster.border({ x: focused.x + 1, y: focused.y + 1, width: focused.width - 2, height: focused.height - 2 }, 0xffff00);
    this.controls = controls;
    this.surface.sync(controls, inputs);
  }

  private paintChatOverlay(message: string, continuation: string, controls: Control[]): void {
    const chat = frameRegions(this.canvas.width, this.canvas.height).chat;
    this.raster.sprite(1017, chat.x, chat.y);
    const lines = sourceLines(escapeText(message), 472, this.assets.catalogue.fonts[495]!);
    const pageSize = 5;
    const page = Math.min(this.local.dialoguePage, Math.max(0, Math.ceil(lines.length / pageSize) - 1));
    this.raster.textBox(lines.slice(page * pageSize, (page + 1) * pageSize).join("<br>"),
      { x: chat.x + 20, y: chat.y + 10, width: 472, height: 88 }, { font: 495, color: 0, shadow: null, xAlign: 1, yAlign: 1, lineHeight: 16 });
    if (continuation) {
      const more = (page + 1) * pageSize < lines.length;
      this.raster.center(more ? "Click here to continue" : continuation, chat.x + 259, chat.y + 121, 495, 0x0000ff, null);
      controls.push({ id: "notice-close", label: more ? "Continue message" : "Continue", x: chat.x + 8, y: chat.y + 100, width: 506, height: 29,
        actions: [{ label: "Continue", run: () => {
          if (more) this.local.dialoguePage++; else { this.local.dialoguePage = 0; this.cancel(); }
          this.renderSoon();
        } }] });
    }
  }
}
