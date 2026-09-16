import type {
  AppServices, AppState, ClientAssets, CreateUi, GameIntent, ItemTarget, ItemView,
  RenderCamera, ScenePick, UiHandle, WorldTarget, WorldView, GameplayUiIntent, GameplayUiView, AudioHandle,
} from "../shared/contracts.ts";
import { UiAssets, contains } from "./assets.ts";
import type { NativeWidget, Rect } from "./assets.ts";
import { InputSurface } from "./input.ts";
import type { Control, InputField, UiAction } from "./input.ts";
import { SourceRaster, escapeText, plainText, sourceLines } from "./raster.ts";
import { entryErrorLines, paintEntry, paintReconnect } from "./entry.ts";
import { frameRegions, TABS } from "./layout.ts";
import { MinimapPainter } from "./minimap.ts";
import { minimapSurfaceProblem, UiMinimapError } from "./minimap-surface.ts";
import type { UiMinimapSurface, UiMinimapStatus } from "./minimap-surface.ts";
export type { UiMinimapSurface, UiMinimapStatus } from "./minimap-surface.ts";
import { paintCharacter, paintGame } from "./world-view.ts";
import { TitleFlames } from "./flames.ts";
import type { AbilityKind, AbilityVisualTruth } from "./filters.ts";
import { bindBankRevision, checkUiIntent, gameplayUi, gameplayUiProblem, isGameplayUiIntent, permissionReason } from "./gameplay-ui.ts";
import { paintConfirmation } from "./presentations.ts";
import type { ProductionAmount } from "./production.ts";
import { rewardDetails } from "./rewards.ts";
import { audioSliderPercent, observedAudio } from "./audio-controls.ts";
import type { UiAudioChannel, UiAudioView } from "./audio-controls.ts";
import type { SourceMusicState } from "../audio/native-scene.ts";
import { applyNativeMusicControl, musicRequest, musicStateProblem, musicScrollPosition } from "./music-controls.ts";
import type { MusicUiAction } from "./music-controls.ts";
import { defaultSettingsPage } from "./settings.ts";
import type { SettingsPageState, ClientInputSettings } from "./settings.ts";
import type { SourceAudioPreferenceBinding, SourceMusicSkipResult } from "../audio/preferences.ts";
import { UiAudioPreferencePersistence } from "./audio-preference-storage.ts";
import type { UiAudioPreferenceSaveStatus } from "./audio-preference-storage.ts";
export { UiAudioPreferencePersistence } from "./audio-preference-storage.ts";
export type { UiAudioPreferenceStorage, UiAudioPreferenceSaveResult, UiAudioPreferenceSaveStatus } from "./audio-preference-storage.ts";

export interface UiNotice {
  message: string; errorId: string | null; recoverable: boolean; scope: "error" | "unavailable" | "information";
  retry?: () => void;
}
export interface AmountPrompt { label: string; value: string; confirm: (quantity: number) => void; pending?: boolean }
export interface ItemSelection { slot: number; id: string; instanceId: string | null; name: string }
export interface UiPreviewRequest {
  purpose: "appearance" | "equipment";
  bounds: Rect; modelBounds: Rect; sourceWidget: number; modelZoom: number; modelRotation: readonly number[];
  appearance: Readonly<Record<string, number>>;
  equipment: Readonly<WorldView["player"]["equipment"]> | null;
  base: GameplayUiView["appearance"]["base"];
}
export interface LocalUiState {
  tab: number; selectedItem: ItemSelection | null; selectedSpell: string | null;
  bankSearch: string; bankSearchOpen: boolean; bankAmount: number | "all"; bankNotes: boolean;
  shopAmount: number; shopValue: boolean; scroll: number; journal: string | null;
  modal: string | null; dialoguePage: number; amount: AmountPrompt | null;
  appearance: Record<string, number>; quickPrayer: boolean; magicFilter: boolean;
  filterPanel: AbilityKind | null; prayerFilters: number; magicFilters: number;
  recoverySelected: string | null;
  chatDraft: string;
  productionAmount: ProductionAmount;
  settingsPage: "controls" | "audio";
  musicDropdown: boolean;
  settings: SettingsPageState;
  clientInput: ClientInputSettings;
  documentPart: number;
  documentTutors: boolean;
}
export interface WorldPointer {
  kind: "move" | "primary" | "context";
  x: number; y: number; pick: ScenePick | null; control?: boolean;
}
export interface GameViewContext {
  state: Readonly<AppState>; world: WorldView; local: LocalUiState;
  controls: Control[]; inputs: InputField[]; minimap: MinimapPainter;
  send: (intent: GameIntent) => void;
  sendUi: (intent: GameplayUiIntent) => void;
  openTab: (index: number) => void;
  change: (change: () => void) => void;
  notice: (message: string, scope?: UiNotice["scope"], id?: string) => void;
  unavailable: (name: string) => void;
  required: (name: string, field: string) => void;
  prompt: (label: string, confirm: (amount: number) => void) => void;
  inventoryActions: (slot: number) => UiAction[];
  equipmentActions: (slot: string) => UiAction[];
  bankActions: (slot: number) => UiAction[];
  bankEntryActions: (entryId: string) => UiAction[];
  shopActions: (index: number) => UiAction[];
  depositAll: () => void;
  logout: () => void;
  confirmAppearance: () => void;
  minimapClick: (widget: NativeWidget) => void;
  faceNorth: () => void;
  abilityVisuals: Readonly<Record<string, AbilityVisualTruth>>;
  sendChat: () => void;
  continueReward: (presentationId: string, continuation: GameplayUiIntent) => void;
  hoveredProduction: string | null;
  hoveredControl: string | null;
  presentationCurrent: (kind: "document" | "reward", id: string, page?: number) => boolean;
  capture: (bounds: Rect) => void;
  preview: (bounds: Rect, model: NativeWidget) => void;
  audio: UiAudioView | null;
  audioValue: (channel: UiAudioChannel) => number | null;
  audioPercent: (channel: UiAudioChannel, percent: number) => void;
  audioMute: (channel: UiAudioChannel) => void;
  audioToggle: () => void;
  music: SourceMusicState | null;
  musicAction: (action: MusicUiAction) => void;
  focusInput: (id: string) => void;
}

function emptyLocal(): LocalUiState {
  return { tab: 3, selectedItem: null, selectedSpell: null, bankSearch: "", bankSearchOpen: false,
    bankAmount: 1, bankNotes: false, shopAmount: 1, shopValue: true, scroll: 0,
    journal: null, modal: null, dialoguePage: 0, amount: null, appearance: { body_type: 0 },
    quickPrayer: true, magicFilter: false, filterPanel: null, prayerFilters: 0, magicFilters: 0, recoverySelected: null, chatDraft: "", productionAmount: 1, settingsPage: "controls", musicDropdown: false,
    settings: defaultSettingsPage(), clientInput: { singleMouse: false, shiftDrop: true, escapeCloses: true },
    documentPart: 0, documentTutors: false };
}

function errorDetails(error: unknown): { message: string; errorId: string | null; recoverable: boolean } {
  const record = typeof error === "object" && error !== null ? error as Record<string, unknown> : {};
  return {
    message: typeof record.message === "string" ? record.message : String(error),
    errorId: typeof record.errorId === "string" ? record.errorId : typeof record.error_id === "string" ? record.error_id
      : typeof record.code === "string" ? record.code : null,
    recoverable: record.recoverable !== false,
  };
}

export function isInterfaceUnlocked(world: WorldView, id: string): boolean {
  const ui = gameplayUi(world);
  if (ui) {
    const entry = ui.interfaces.find(row => row.interface === id);
    return entry?.visibility === "enabled" && entry.permission.allowed;
  }
  if (world.player.unlockedInterfaces.includes(id)) return true;
  return !world.player.tutorialStage.startsWith("stage.tutorial.") || world.player.tutorialStage === "stage.tutorial.mainland";
}

export function readQuantity(value: string): number | null {
  if (!/^[1-9]\d*$/.test(value.trim())) return null;
  const amount = Number(value.trim());
  return Number.isSafeInteger(amount) && amount <= 4294967295 ? amount : null;
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

export function setUiMinimap(handle: UiHandle, surface: UiMinimapSurface | null): void {
  const controller = controllers.get(handle);
  if (!controller) throw new UiMinimapError("The UI handle is not live.", "ui.minimap.disposed");
  controller.supplyMinimap(surface);
}

export function getUiMinimapStatus(handle: UiHandle): UiMinimapStatus | null {
  return controllers.get(handle)?.minimapStatus() ?? null;
}

function minimapScope(world: WorldView | null): string | null {
  return world ? JSON.stringify([world.player.id, world.player.instance, world.player.tile.plane]) : null;
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

/** Renderer input only: local approved appearance selection plus actual source geometry/equipment. */
export function getUiPreviewRequest(handle: UiHandle): Readonly<UiPreviewRequest> | null {
  const request = controllers.get(handle)?.previewRequest;
  return request ? structuredClone(request) : null;
}

/** Observe the real audio graph; UI disposal detaches without disposing the shell's audio handle. */
export async function bindUiAudio(handle: UiHandle, audio: AudioHandle): Promise<() => void> {
  const module = await import("../audio/index.ts");
  const controller = controllers.get(handle);
  if (!controller) throw new Error("UI handle has been disposed.");
  return controller.bindAudio(audio, module);
}

export function bindUiAudioPreferencePersistence(handle: UiHandle, persistence: UiAudioPreferencePersistence): () => void {
  const controller = controllers.get(handle);
  if (!controller) throw new Error("UI handle has been disposed.");
  controller.preferencePersistence = persistence;
  controller.renderSoon();
  return () => {
    if (controller.preferencePersistence === persistence) { controller.preferencePersistence = null; controller.renderSoon(); }
  };
}

export function getUiAudioPreferences(handle: UiHandle): SourceAudioPreferenceBinding | null {
  return controllers.get(handle)?.readAudioPreferences() ?? null;
}

export function getUiAudioPreferenceSaveStatus(handle: UiHandle): UiAudioPreferenceSaveStatus | null {
  return controllers.get(handle)?.audioPreferenceSaveStatus() ?? null;
}

export function getUiMusicSkipResult(handle: UiHandle): SourceMusicSkipResult | null {
  return controllers.get(handle)?.readMusicSkipResult() ?? null;
}

export function retryUiAudioPreferenceSave(handle: UiHandle): void {
  const controller = controllers.get(handle);
  if (!controller) throw new Error("UI handle has been disposed.");
  controller.retryAudioPreferenceSave();
}

/** Apply the same published music state to audio and UI, scoped to the actual current player. */
export function setUiMusicState(handle: UiHandle, playerId: string, state: SourceMusicState): void {
  const controller = controllers.get(handle);
  if (!controller) throw new Error("UI handle has been disposed.");
  controller.supplyMusicState(playerId, state);
}

export function getUiMusicState(handle: UiHandle): Readonly<SourceMusicState> | null {
  return controllers.get(handle)?.readMusicState() ?? null;
}

/** Reports applied client music preferences; it does not grant unlocks or emit gameplay audio events. */
export function onUiMusicStateChange(handle: UiHandle, listener: (playerId: string, state: SourceMusicState) => void | Promise<void>): () => void {
  const controller = controllers.get(handle);
  if (!controller) throw new Error("UI handle has been disposed.");
  controller.musicChanged = listener;
  return () => { if (controller.musicChanged === listener) controller.musicChanged = null; };
}

/** Supply only an actual model-only renderer surface at the requested native dimensions. */
export function setUiPreview(handle: UiHandle, image: HTMLCanvasElement | OffscreenCanvas | ImageBitmap | null): void {
  const controller = controllers.get(handle);
  if (controller) { controller.preview = image; controller.renderSoon(); }
}

/** Optional source-display facts from an authoritative adapter, scoped to its exact snapshot. */
export function setUiAbilityVisuals(handle: UiHandle, revision: string, values: Readonly<Record<string, AbilityVisualTruth>>): void {
  const controller = controllers.get(handle);
  if (controller) { controller.abilityVisuals = { revision, values: structuredClone(values) }; controller.renderSoon(); }
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
  previewRequest: UiPreviewRequest | null = null;
  abilityVisuals: { revision: string; values: Readonly<Record<string, AbilityVisualTruth>> } | null = null;
  private readonly surface: InputSurface;
  private readonly unsubscribe: () => void;
  private readonly abort = new AbortController();
  private disposed = false;
  private scheduled = 0;
  private controls: Control[] = [];
  private panelBounds: Rect[] = [];
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
  private bankDrag: { entry: string; item: string; instance: string | null; revision: string; x: number; y: number; start: number; active: boolean } | null = null;
  private suppressNextClick = false;
  private name = "";
  private password = "";
  private confirmation = "";
  private hideName = false;
  private authGeneration = 0;
  private readonly titleFlames: TitleFlames;
  private readonly titleStartedAt = performance.now();
  private entryErrorKind: "capability" | "runtime-error" = "runtime-error";
  private entryErrorPage = 0;
  private chatSubmission: { text: string; messageIds: Set<string>; accepted: boolean } | null = null;
  private audioHandle: AudioHandle | null = null;
  private audioApi: typeof import("../audio/index.ts") | null = null;
  private audioEpoch = 0;
  private audioControlSequence = 0;
  private lastMusicSkip: { player: string; epoch: number; result: SourceMusicSkipResult } | null = null;
  preferencePersistence: UiAudioPreferencePersistence | null = null;
  private audioView: UiAudioView | null = null;
  private stopAudio: (() => void) | null = null;
  private masterVolume: ((percent: number) => void) | null = null;
  private readonly mutedPercentages = new Map<UiAudioChannel, number>();
  private sliderDrag: { id: string; grab: number } | null = null;
  private scrollDrag: { id: string; grab: number } | null = null;
  private audioTrace: import("../audio/index.ts").AudioTrace | null = null;
  private musicState: { playerId: string; value: SourceMusicState } | null = null;
  private sourceMusic: ((value: SourceMusicState) => void) | null = null;
  musicChanged: ((playerId: string, state: SourceMusicState) => void | Promise<void>) | null = null;

  constructor(canvas: HTMLCanvasElement, services: AppServices, assets: UiAssets) {
    this.canvas = canvas; this.services = services; this.assets = assets;
    this.state = services.state();
    if (this.state.world) this.local.appearance = { ...this.state.world.player.appearance };
    this.raster = new SourceRaster(canvas, assets);
    const visualSeed = crypto.getRandomValues(new Uint32Array(1))[0]!;
    this.titleFlames = new TitleFlames(assets.catalogue.flames, visualSeed);
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
      blocked: reason => this.show(reason, "error"),
      activate: (control, shifted) => {
        if (this.local.clientInput.singleMouse && !(shifted && control.shiftAction) &&
            !control.id.startsWith("menu-") && control.actions.length > 1)
          this.openMenu(control.actions, control.x, control.y + control.height);
        else (shifted && control.shiftAction ? control.shiftAction : control.actions[0])?.run();
      },
    });
    const scrollTargets: HTMLElement[] = [canvas, this.surface.root];
    for (const target of scrollTargets) target.addEventListener("wheel", event => {
      const [x, y] = this.xy(event);
      if (!this.capturesPointer(x, y) || this.menu || this.notice || this.local.amount || this.local.musicDropdown) return;
      event.preventDefault();
      if (this.local.settings.choice) this.local.settings.choice.scroll = Math.max(0, this.local.settings.choice.scroll + Math.sign(event.deltaY) * 20);
      else if (this.local.modal === "all-settings") this.local.settings.scroll = Math.max(0, this.local.settings.scroll + Math.sign(event.deltaY) * 45);
      else this.local.scroll = Math.max(0, this.local.scroll + Math.sign(event.deltaY) * (this.local.tab === 13 ? 45 : 36));
      this.renderSoon();
    }, { signal: this.abort.signal, passive: false });
    window.addEventListener("pointerup", event => {
      const inControls = event.target instanceof Node && this.surface.root.contains(event.target);
      if ((this.drag || this.bankDrag || this.sliderDrag || this.scrollDrag) && !inControls && event.target !== canvas) this.pointer(event, null, "up");
    }, { signal: this.abort.signal });
    window.addEventListener("blur", () => { this.drag = null; this.bankDrag = null; this.sliderDrag = null; this.scrollDrag = null; this.menu = null; this.renderSoon(); }, { signal: this.abort.signal });
    window.addEventListener("pointercancel", () => { this.drag = null; this.bankDrag = null; this.sliderDrag = null; this.scrollDrag = null; this.renderSoon(); }, { signal: this.abort.signal });
    assets.changed(() => this.renderSoon());
    this.unsubscribe = services.subscribe(state => this.update(state));
    const bounds = canvas.getBoundingClientRect();
    this.resize(bounds.width || canvas.width, bounds.height || canvas.height);
    this.update(this.state);
    if (this.state.world) {
      const problem = gameplayUiProblem(this.state.world);
      if (problem) this.surface.announce(problem.message);
    }
    requestAnimationFrame(() => {
      if (!this.disposed && (this.state.phase === "login" || this.state.phase === "register")) this.surface.focus("name");
    });
  }

  bindAudio(handle: AudioHandle, api: typeof import("../audio/index.ts")): () => void {
    this.stopAudio?.();
    this.audioEpoch++;
    this.mutedPercentages.clear();
    this.audioTrace = null;
    this.audioHandle = handle; this.audioApi = api;
    const receive = (snapshot: import("../audio/index.ts").AudioSnapshot) => {
      if (this.disposed) return;
      const previousPlayer = this.audioView?.preferences?.playerId ?? null;
      const nextPlayer = snapshot.preferences?.playerId ?? null;
      if (previousPlayer !== nextPlayer || snapshot.disposed) {
        this.audioEpoch++; this.menu = null; this.sliderDrag = null;
        if (this.notice?.retry) this.notice = null;
        if (previousPlayer !== null) this.musicState = null;
      }
      const result = observedAudio(snapshot);
      if (result.problem) {
        this.audioView = null;
        this.show(result.problem, "error", "ui.audio.observation");
      } else if (JSON.stringify(result.value) !== JSON.stringify(this.audioView)) {
        this.audioView = result.value; this.renderSoon();
      }
      if (snapshot.preferences && snapshot.preferences.playerId === this.state.world?.player.id)
        this.musicState = { playerId: snapshot.preferences.playerId, value: snapshot.preferences.musicState };
      const failure = snapshot.traces.findLast(trace => trace.type === "error");
      if (failure && failure !== this.audioTrace) {
        this.audioTrace = failure;
        const message = failure.data.message, code = failure.data.code;
        if (typeof message === "string" && typeof code === "string") {
          if (code === "AUDIO_GESTURE_REQUIRED") this.surface.announce(`${message} Error ID: ${code}`);
          else if (this.notice?.retry && this.notice.errorId !== code) {
            const previous = this.notice, retry = this.notice.retry;
            this.show(`${previous.message}\nError ID: ${previous.errorId ?? "unavailable"}\n${message}`, "error", code);
            if (this.notice) this.notice.retry = retry;
          } else this.show(message, "error", code);
        }
      }
    };
    this.masterVolume = percent => api.setSourceMasterVolume(handle, percent);
    this.sourceMusic = state => { api.setSourceMusicState(handle, state); receive(api.readAudioState(handle)); };
    const stop = api.observeAudioState(handle, receive);
    const cleanup = () => {
      stop();
      if (this.stopAudio === cleanup) {
        this.audioEpoch++;
        this.stopAudio = null; this.audioHandle = null; this.audioApi = null; this.audioView = null; this.masterVolume = null; this.sliderDrag = null; this.audioTrace = null;
        this.musicState = null; this.sourceMusic = null;
        this.renderSoon();
      }
    };
    this.stopAudio = cleanup;
    return cleanup;
  }

  supplyMusicState(playerId: string, value: SourceMusicState): void {
    try {
      if (!this.state.world || this.state.world.player.id !== playerId)
        throw Object.assign(new Error("The supplied music state belongs to a different player snapshot."), { errorId: "ui.music.owner" });
      if (!this.sourceMusic || !this.audioView || this.audioView.disposed)
        throw Object.assign(new Error("Bind the actual audio handle before supplying music state."), { errorId: "ui.music.binding" });
      const problem = musicStateProblem(value, this.audioView.preferences?.playerId === playerId);
      if (problem) throw Object.assign(new Error(problem), { errorId: "ui.music.state" });
      const state = Object.freeze({ ...value, unlockedGroups: Object.freeze([...value.unlockedGroups]),
        playlistGroups: Object.freeze([...value.playlistGroups]) });
      this.sourceMusic(state);
      this.musicState = { playerId, value: state };
      this.renderSoon();
    } catch (error) {
      const details = errorDetails(error);
      this.show(details.message, "error", details.errorId);
      throw error;
    }
  }

  readMusicState(): SourceMusicState | null {
    return this.musicState ? structuredClone(this.musicState.value) : null;
  }

  readAudioPreferences(): SourceAudioPreferenceBinding | null {
    const value = this.audioView?.preferences;
    return value && value.playerId === this.state.world?.player.id ? structuredClone(value) : null;
  }

  audioPreferenceSaveStatus(): UiAudioPreferenceSaveStatus | null {
    const player = this.readAudioPreferences()?.playerId;
    return player && this.preferencePersistence ? this.preferencePersistence.status(player) : null;
  }

  readMusicSkipResult(): SourceMusicSkipResult | null {
    const value = this.lastMusicSkip;
    return value && this.audioControlCurrent(value.player, value.epoch) ? { ...value.result } : null;
  }

  private audioControlCurrent(player: string, epoch: number): boolean {
    return !this.disposed && epoch === this.audioEpoch && this.state.world?.player.id === player &&
      (this.state.phase === "world" || this.state.phase === "character");
  }

  private audioControlFailure(error: unknown, player: string, epoch: number): void {
    const details = errorDetails(error);
    if (this.audioControlCurrent(player, epoch)) {
      this.show(details.message, "error", details.errorId);
      const status = this.preferencePersistence?.status(player);
      if (status?.state === "failed" && status.dirty && this.notice) this.notice.retry = () => {
        if (this.audioControlCurrent(player, epoch)) this.retryAudioPreferenceSave();
        else this.show("That save retry belongs to a previous player session.", "error", "ui.audio.control.stale");
      };
    } else this.services.report(error instanceof Error ? error : new Error(String(error)), details.errorId ?? undefined);
  }

  private permissionForAudioControl(player: string, epoch: number): Promise<{ ok: true } | { ok: false; error: unknown }> {
    return this.services.unlockAudio().then(() => ({ ok: true }), error => {
      this.audioControlFailure(error, player, epoch);
      return { ok: false, error };
    });
  }

  private async saveAudioPreferences(binding: SourceAudioPreferenceBinding, epoch: number): Promise<void> {
    const persistence = this.preferencePersistence;
    if (!persistence) throw Object.assign(new Error("The shell has not bound real player audio preference storage."), { errorId: "ui.audio.storage.unbound" });
    try { await persistence.save(binding.playerId, binding.preferences); }
    catch (error) { this.audioControlFailure(error, binding.playerId, epoch); throw error; }
  }

  retryAudioPreferenceSave(): void {
    const binding = this.readAudioPreferences(), persistence = this.preferencePersistence, epoch = this.audioEpoch;
    if (!binding || !persistence) { this.show("Player audio preference storage is not bound.", "error", "ui.audio.storage.unbound"); return; }
    this.notice = null;
    void this.request(`audio-save-retry:${binding.playerId}:${epoch}`, async () => {
      try { await persistence.retry(binding.playerId); }
      catch (error) { this.audioControlFailure(error, binding.playerId, epoch); throw error; }
    }, false, () => this.audioControlCurrent(binding.playerId, epoch));
  }

  private nativeAudioControl(key: string, expectedEpoch: number,
    change: (api: typeof import("../audio/index.ts"), handle: AudioHandle, binding: SourceAudioPreferenceBinding) => SourceAudioPreferenceBinding): void {
    const binding = this.readAudioPreferences(), api = this.audioApi, handle = this.audioHandle, epoch = this.audioEpoch;
    if (expectedEpoch !== epoch || !binding || !api || !handle) {
      this.show("This audio control has no current player preference binding.", "error", "ui.audio.preference.binding"); return;
    }
    if (!this.preferencePersistence) { this.show("The shell has not bound real player audio preference storage.", "error", "ui.audio.storage.unbound"); return; }
    void this.request(`native-audio:${binding.playerId}:${epoch}:${++this.audioControlSequence}:${key}`, async () => {
      const permission = this.permissionForAudioControl(binding.playerId, epoch);
      const next = change(api, handle, binding);
      this.supplyMusicState(next.playerId, next.musicState);
      const [access] = await Promise.all([permission, this.saveAudioPreferences(next, epoch)]);
      if (!access.ok) throw access.error;
    }, false, () => this.audioControlCurrent(binding.playerId, epoch));
  }

  private changeMusic(action: MusicUiAction, expectedEpoch = this.audioEpoch): void {
    if (expectedEpoch !== this.audioEpoch) { this.show("That audio control belongs to a previous player session.", "error", "ui.audio.control.stale"); return; }
    const current = this.musicState, player = this.state.world?.player.id;
    if (!current || current.playerId !== player) {
      this.show("Supply the current player's SourceMusicState through setUiMusicState before using music controls.",
        "error", "ui.music.binding"); return;
    }
    if (action.kind === "skip") {
      const api = this.audioApi, handle = this.audioHandle, epoch = this.audioEpoch;
      if (!api || !handle) { this.show("The real audio handle is not bound.", "error", "ui.audio.observer_binding"); return; }
      void this.request(`music-skip:${player}:${epoch}`, async () => {
        const result = await api.requestSourceMusicSkip(handle, player);
        if (!this.audioControlCurrent(player, epoch)) return;
        this.lastMusicSkip = { player, epoch, result };
        if (result.status === "requested" || result.status === "pending")
          this.surface.announce(`Source Skip Track ${result.status}; playback permission and device output remain observable.`);
        else this.show(result.status === "disabled_mode" ? "Skip Track is available only in Shuffle Mode."
          : result.status === "muted" ? "Music is muted; no next selection was consumed."
            : "No alternative source-unlocked track is available.", "information", `ui.music.skip.${result.status}`);
      }, false, () => this.audioControlCurrent(player, epoch));
      return;
    }
    if (this.audioView?.preferences) {
      this.nativeAudioControl(JSON.stringify(action), expectedEpoch,
        (api, handle, binding) => applyNativeMusicControl(api, handle, binding, action, this.audioView?.plannedGroup ?? null));
      return;
    }
    const next = musicRequest(current.value, action, this.audioView?.plannedGroup ?? null);
    if (!next.state) { this.show(next.problem, "error", "ui.music.selection"); return; }
    void this.request(`music-preference:${JSON.stringify(next.state)}`, async () => {
      this.supplyMusicState(current.playerId, next.state);
      const playback = action.kind === "mode" || action.kind === "play" ? this.services.unlockAudio() : Promise.resolve();
      await Promise.all([playback, Promise.resolve().then(() => this.musicChanged?.(current.playerId, structuredClone(next.state)))]);
    });
  }

  private xy(event: Pick<MouseEvent, "clientX" | "clientY">): [number, number] {
    const point = this.surface.coordinates(event);
    return [point.x, point.y];
  }

  supplyMinimap(surface: UiMinimapSurface | null): void {
    if (this.disposed) throw new UiMinimapError("The UI handle is not live.", "ui.minimap.disposed");
    if (surface === null) { this.minimap.clear(); this.renderSoon(); return; }
    try {
      const world = this.state.world, scope = minimapScope(world);
      if (!world || scope === null) throw new UiMinimapError("Publish the current world before its renderer minimap.", "ui.minimap.world");
      const problem = minimapSurfaceProblem(surface);
      if (problem) throw new UiMinimapError(problem);
      if (surface.plane !== world.player.tile.plane)
        throw new UiMinimapError("The renderer minimap belongs to a different plane.", "ui.minimap.plane");
      if (this.minimap.supply(surface, scope)) this.renderSoon();
    } catch (error) {
      if (!(error instanceof UiMinimapError)) throw error;
      this.show(error.message, "error", error.errorId);
      this.services.report(error, error.errorId);
      throw error;
    }
  }

  minimapStatus(): UiMinimapStatus | null { return this.disposed ? null : this.minimap.status(); }

  update(state: Readonly<AppState>): void {
    if (this.disposed) return;
    const old = this.state;
    if (old.world?.player.id !== state.world?.player.id ||
        old.phase !== state.phase && !["world", "character"].includes(state.phase)) {
      this.audioEpoch++;
      if (this.notice?.retry) this.notice = null;
    }
    this.state = state;
    this.minimap.retainScope(minimapScope(state.world));
    if (old.world?.player.id !== state.world?.player.id) {
      this.local = emptyLocal(); this.chatSubmission = null; this.drag = null; this.bankDrag = null; this.scrollDrag = null; this.menu = null;
      this.musicState = null;
      if (state.world) this.local.appearance = { ...state.world.player.appearance };
    }
    if (old.world?.dialogue?.id !== state.world?.dialogue?.id) this.local.dialoguePage = 0;
    if (old.world?.bank?.banker !== state.world?.bank?.banker) {
      this.local.bankSearch = ""; this.local.bankSearchOpen = false; this.local.scroll = 0; this.local.amount = null;
    }
    if (old.world?.shop?.id !== state.world?.shop?.id) { this.local.scroll = 0; this.local.amount = null; }
    const projection = state.world ? gameplayUi(state.world) : null;
    const previousProjection = old.world ? gameplayUi(old.world) : null;
    if (projection) this.local.tab = TABS.findIndex(tab => tab.interface === projection.activeTab);
    if (projection?.document?.id !== previousProjection?.document?.id ||
        projection?.document?.page !== previousProjection?.document?.page) this.local.documentPart = 0;
    if (projection?.document?.id !== previousProjection?.document?.id) this.local.documentTutors = false;
    if (projection?.production?.id !== previousProjection?.production?.id ||
        projection?.confirmation?.id !== previousProjection?.confirmation?.id ||
        projection?.reward?.id !== previousProjection?.reward?.id) {
      this.local.scroll = 0; this.local.dialoguePage = 0;
      if (!this.local.amount?.pending) this.local.amount = null;
    }
    if (projection?.production?.id !== previousProjection?.production?.id) this.local.productionAmount = 1;
    if (projection && this.chatSubmission?.accepted) this.reconcileChat();
    if (state.world && (!old.world || gameplayUiProblem(state.world)?.code !== gameplayUiProblem(old.world)?.code)) {
      const problem = gameplayUiProblem(state.world);
      if (problem) this.surface.announce(problem.message);
      else this.surface.announce("Versioned game.ui.v1 projection received.");
    }
    if (state.world?.ui?.version === 1) {
      const problem = gameplayUiProblem(state.world);
      if (problem) this.show(problem.message, "error", problem.code);
    }
    const selected = this.local.selectedItem;
    if (selected && !this.sameItem(selected)) this.local.selectedItem = null;
    if (this.awaitingSelectionRevision !== null && state.world?.revision !== this.awaitingSelectionRevision) {
      this.local.selectedItem = null; this.local.selectedSpell = null; this.awaitingSelectionRevision = null;
    }
    if (this.local.amount?.pending && state.world?.revision !== this.amountRevision) {
      this.local.amount = null; this.amountRevision = null;
    }
    if (state.world && !projection && !isInterfaceUnlocked(state.world, TABS[this.local.tab]?.interface ?? "interface.inventory")) {
      const allowed = TABS.findIndex(tab => isInterfaceUnlocked(state.world!, tab.interface));
      this.local.tab = allowed < 0 ? 3 : allowed;
    }
    if (state.error && state.error !== old.error) {
      this.entryErrorPage = 0;
      this.entryErrorKind = old.phase === "capability_check" || state.phase === "capability_check" ? "capability" : "runtime-error";
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
    this.menu = null; this.drag = null; this.bankDrag = null; this.sliderDrag = null; this.scrollDrag = null; this.renderSoon();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true; this.authGeneration++;
    this.stopAudio?.(); this.mutedPercentages.clear();
    this.password = ""; this.confirmation = ""; this.name = "";
    cancelAnimationFrame(this.scheduled);
    this.unsubscribe(); this.abort.abort(); this.surface.dispose(); this.assets.dispose(); this.minimap.dispose();
    this.local = emptyLocal(); this.pending.clear(); this.raster.dispose();
    this.titleFlames.dispose();
    this.controls = []; this.panelBounds = []; this.hover = null; this.menu = null; this.drag = null; this.bankDrag = null; this.notice = null;
    this.scrollDrag = null;
    this.chatSubmission = null;
    this.preview = null; this.previewBounds = null; this.previewRequest = null; this.cameraRequest = null; this.abilityVisuals = null;
    this.musicChanged = null; this.preferencePersistence = null; this.audioEpoch++;
  }

  renderSoon(): void {
    if (this.disposed || this.scheduled) return;
    this.scheduled = requestAnimationFrame(() => {
      this.scheduled = 0;
      if (!this.disposed) {
        this.render();
        if (this.entryAnimationVisible()) this.renderSoon();
      }
    });
  }

  private entryAnimationVisible(): boolean {
    return this.state.phase !== "world" && this.state.phase !== "character" &&
      !(this.state.phase === "reconnecting" && this.state.world) &&
      !(this.state.phase === "capability_check" && this.state.loading);
  }

  show(message: string, scope: UiNotice["scope"] = "information", errorId: string | null = null): void {
    if (this.notice?.message === message && this.notice.errorId === errorId && this.notice.scope === scope) return;
    this.notice = { message, errorId, recoverable: true, scope };
    this.entryErrorPage = 0;
    this.local.dialoguePage = 0;
    this.menu = null;
    this.surface.announce(message + (errorId ? ` Error ID: ${errorId}` : ""));
    this.renderSoon();
  }

  private unavailable(name: string): void {
    this.show(`${name} is not available in this slice. Its original control position is preserved.`, "unavailable");
  }

  private required(name: string, field: string): void {
    if (field === "human_appearance_controls") {
      this.show(`${name} does not apply to the approved penguin base. Body type A/B and confirmation remain available; no additional cosmetic variant has been approved.`, "unavailable");
      return;
    }
    if (this.state.world) {
      const problem = gameplayUiProblem(this.state.world);
      if (problem) { this.show(`${name}: ${problem.message} Required integration: ${field}.`, "error", problem.code); return; }
    }
    this.show(`${name} cannot be completed: the server interface does not supply ${field}. This is a required integration, not an out-of-scope feature.`,
      "error", `ui.contract.${field}`);
  }

  private async request(key: string, action: () => Promise<void>, selection = false, current?: () => boolean): Promise<boolean> {
    if (this.disposed || this.pending.has(key)) return false;
    if (this.state.phase === "reconnecting") { this.show("Connection lost. Please wait - attempting to reestablish.", "error"); return false; }
    const revision = this.state.world?.revision ?? null;
    this.pending.add(key); this.renderSoon();
    try {
      await action();
      if (this.disposed || current && !current()) return false;
      if (selection) {
        if (this.state.world?.revision !== revision) { this.local.selectedItem = null; this.local.selectedSpell = null; }
        else this.awaitingSelectionRevision = revision;
      }
      return true;
    } catch (error) {
      if (!this.disposed && (!current || current())) {
        const details = errorDetails(error);
        if (this.local.amount) this.local.amount.pending = false;
        this.show(details.message, "error", details.errorId);
      } else if (current) {
        const details = errorDetails(error);
        this.services.report(error instanceof Error ? error : new Error(String(error)), details.errorId ?? undefined);
      }
      return false;
    } finally { this.pending.delete(key); this.renderSoon(); }
  }

  private send(intent: GameIntent): void {
    if (isGameplayUiIntent(intent)) { this.sendUi(intent); return; }
    if (this.state.world?.ui !== undefined) {
      const problem = gameplayUiProblem(this.state.world);
      if (problem) { this.show(problem.message, "error", problem.code); return; }
    }
    const ui = this.state.world ? gameplayUi(this.state.world) : null;
    if (ui) {
      if (intent.kind === "open_interface") {
        const entry = ui.interfaces.find(row => row.interface === intent.interface);
        const reason = permissionReason(entry?.permission, intent.interface);
        if (entry?.visibility !== "enabled" || reason) {
          this.show(reason ?? "This interface is locked.", "error", entry?.permission.code ?? "ui.interface.unavailable"); return;
        }
      }
      const ability = intent.kind === "set_combat_style" ? ui.combatStyles.find(row => row.id === intent.style)
        : intent.kind === "set_prayer" ? ui.prayers.find(row => row.id === intent.prayer)
          : intent.kind === "cast" ? ui.spells.find(row => row.id === intent.spell) : undefined;
      if (intent.kind === "set_combat_style" || intent.kind === "set_prayer" || intent.kind === "cast") {
        const reason = permissionReason(ability?.permission, "Ability");
        if (!ability?.visible || reason) { this.show(reason ?? "This ability is not visible in the current projection.", "error", ability?.permission.code ?? "ui.ability.unavailable"); return; }
      }
    }
    void this.request(JSON.stringify(intent), () => this.services.send(intent),
      intent.kind === "use_item" || intent.kind === "cast");
  }

  private sendUi(intent: GameplayUiIntent, bankRevision?: string): void {
    const world = this.state.world;
    if (!world) { this.show("A supported game.ui.v1 world is required.", "error", "ui.capability.game.ui.v1"); return; }
    if (bankRevision !== undefined) intent = bindBankRevision(intent, bankRevision);
    const problem = checkUiIntent(world, intent);
    if (problem) { this.show(problem.message, "error", problem.code); return; }
    const request = structuredClone(intent);
    void this.request(JSON.stringify(request), () => this.services.send(request));
  }

  private reconcileChat(): void {
    const world = this.state.world, pending = this.chatSubmission;
    const ui = world ? gameplayUi(world) : null;
    if (!ui || !world || !pending?.accepted) return;
    if (ui.publicChat.messages.some(message => message.actor === world.player.id &&
        message.text === pending.text && !pending.messageIds.has(message.id))) {
      if (this.local.chatDraft === pending.text) this.local.chatDraft = "";
      this.chatSubmission = null; this.renderSoon();
    }
  }

  private async sendChat(): Promise<void> {
    const world = this.state.world;
    if (!world || this.pending.has("public-chat")) return;
    if (this.chatSubmission?.accepted) {
      this.show("The chat request was acknowledged. Waiting for its authoritative message before sending again.", "information");
      return;
    }
    const ui = gameplayUi(world);
    const intent: GameplayUiIntent = { kind: "public_chat", channel: "public", text: this.local.chatDraft };
    const problem = checkUiIntent(world, intent);
    if (problem || !ui) { this.show(problem?.message ?? "game.ui.v1 is unavailable.", "error", problem?.code ?? "ui.capability.game.ui.v1"); return; }
    const submission = { text: intent.text, messageIds: new Set(ui.publicChat.messages.map(message => message.id)), accepted: false };
    this.chatSubmission = submission;
    if (await this.request("public-chat", () => this.services.send(intent))) {
      if (this.chatSubmission === submission) submission.accepted = true;
      this.reconcileChat();
    } else if (this.chatSubmission === submission) this.chatSubmission = null;
  }

  private continueReward(presentationId: string, continuation: GameplayUiIntent): void {
    const reward = this.state.world ? gameplayUi(this.state.world)?.reward : null;
    if (!reward || reward.id !== presentationId) {
      this.show("That reward presentation changed. Review the current presentation.", "error", "ui.presentation.stale"); return;
    }
    this.sendUi(structuredClone(continuation));
  }

  private confirmAppearance(): void {
    if (this.state.world?.ui !== undefined) {
      const problem = gameplayUiProblem(this.state.world);
      if (problem) { this.show(problem.message, "error", problem.code); return; }
    }
    const ui = this.state.world ? gameplayUi(this.state.world) : null;
    if (ui) {
      const choices = ui.appearance.choices.body_type;
      const choice = choices?.find(choice => choice.value === this.local.appearance.body_type);
      const reason = permissionReason(choice?.permission, "Body type");
      if (reason || ui.appearance.confirmed) { this.show(reason ?? "Appearance is already confirmed.", "error", choice?.permission.code ?? "ui.appearance.confirmed"); return; }
      this.send({ kind: "confirm_appearance", appearance: { body_type: choice!.value } });
    } else void this.request("appearance", () => this.services.createCharacter({ ...this.local.appearance }));
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
    if (!this.audioHandle || !this.audioView || this.audioView.disposed) {
      this.show("Source sound controls require bindUiAudio and the actual audio observer.", "error", "ui.audio.observer_binding"); return;
    }
    const handle = this.audioHandle;
    if (this.audioView.enabled) { handle.mute(true); return; }
    void this.request("audio-unlock", async () => {
      handle.mute(false);
      await this.services.unlockAudio();
    });
  }

  private audioPercent(channel: UiAudioChannel, percent: number, expectedEpoch = this.audioEpoch): void {
    if (expectedEpoch !== this.audioEpoch) { this.show("That audio control belongs to a previous player session.", "error", "ui.audio.control.stale"); return; }
    if (!this.audioView || this.audioView.disposed || !this.audioHandle) {
      this.show("Actual source audio settings are unavailable.", "error", "ui.audio.observer_binding"); return;
    }
    if (!Number.isInteger(percent) || percent < 0 || percent > 100) {
      this.show("Source slider positions must be integer percentages from 0 to 100.", "error", "ui.audio.slider"); return;
    }
    if (this.audioView.preferences) {
      this.nativeAudioControl(`volume:${channel}:${percent}`, expectedEpoch,
        (api, handle, binding) => api.setSourceAudioPercent(handle, binding.playerId, channel, percent));
      return;
    }
    void this.request(`audio-${channel}-${percent}`, async () => {
      if (channel === "master") this.masterVolume!(percent);
      else this.services.audioVolume(channel, percent / 100);
    });
  }

  private audioMute(channel: UiAudioChannel, expectedEpoch = this.audioEpoch): void {
    if (expectedEpoch !== this.audioEpoch) { this.show("That audio control belongs to a previous player session.", "error", "ui.audio.control.stale"); return; }
    if (!this.audioView || this.audioView.disposed) {
      this.show("Actual source audio settings are unavailable.", "error", "ui.audio.observer_binding"); return;
    }
    if (this.audioView.preferences) {
      this.nativeAudioControl(`mute:${channel}`, expectedEpoch,
        (api, handle, binding) => api.toggleSourceAudioMute(handle, binding.playerId, channel));
      return;
    }
    const current = this.audioView.percentages[channel];
    if (current > 0) {
      this.mutedPercentages.set(channel, current);
      this.audioPercent(channel, 0);
    } else {
      const restore = this.mutedPercentages.get(channel);
      if (restore === undefined) {
        this.show("The remembered native mute value was not supplied. Set the desired source position with the slider; no saved value is invented.",
          "error", "ui.audio.remembered_mute"); return;
      }
      this.audioPercent(channel, restore);
    }
  }

  private cancel(): void {
    if (this.sliderDrag || this.scrollDrag) { this.sliderDrag = null; this.scrollDrag = null; this.renderSoon(); return; }
    if (this.menu) this.menu = null;
    else if (this.notice) this.notice = null;
    else if (this.state.error && this.dismissedError !== this.state.error) this.dismissedError = this.state.error;
    else if (this.local.amount) this.local.amount = null;
    else if (this.local.bankSearchOpen) this.local.bankSearchOpen = false;
    else if (this.local.filterPanel) this.local.filterPanel = null;
    else if (this.local.musicDropdown) this.local.musicDropdown = false;
    else if (this.local.settings.choice) this.local.settings.choice = null;
    else if (this.local.modal === "all-settings") this.local.modal = null;
    else if (this.local.selectedItem || this.local.selectedSpell) { this.local.selectedItem = null; this.local.selectedSpell = null; }
    else if (this.state.world && gameplayUi(this.state.world)?.confirmation) {
      this.sendUi({ kind: "ui_confirm", confirmation_id: gameplayUi(this.state.world)!.confirmation!.id, accept: false });
    } else if (this.state.world && gameplayUi(this.state.world)?.reward) {
      this.sendUi(structuredClone(gameplayUi(this.state.world)!.reward!.continuation));
    } else if (this.state.world && gameplayUi(this.state.world)?.document) {
      this.sendUi({ kind: "ui_dismiss", presentation_id: gameplayUi(this.state.world)!.document!.id });
    } else if (this.state.world && gameplayUi(this.state.world)?.activeInterface) {
      this.send({ kind: "close_interface" });
    }
    else if (this.local.modal || this.local.journal || this.state.world?.bank || this.state.world?.shop || this.state.world?.recovery) {
      this.local.modal = null; this.local.journal = null; this.send({ kind: "close_interface" });
    } else if (this.state.world) this.send({ kind: "cancel_activity" });
    else this.screen("title");
    this.drag = null; this.bankDrag = null; this.renderSoon();
  }

  private key(event: KeyboardEvent): void {
    if (event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault(); event.stopPropagation();
      if (this.local.clientInput.escapeCloses || this.menu || this.notice || this.local.amount || this.local.selectedItem || this.local.selectedSpell || this.local.settings.choice)
        this.cancel();
      return;
    }
    if (this.surface.activeInput()) return;
    const scrolling = this.controls.find(control => control.id === this.focus && control.scrollbar);
    if (scrolling?.scrollbar && ["ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      const value = event.key === "Home" ? 0 : event.key === "End" ? scrolling.scrollbar.maximum
        : scrolling.scrollbar.current() + (event.key === "PageUp" ? -scrolling.scrollbar.page : event.key === "PageDown" ? scrolling.scrollbar.page
          : event.key === "ArrowUp" ? -4 : 4);
      scrolling.scrollbar.change(Math.max(0, Math.min(scrolling.scrollbar.maximum, value))); return;
    }
    const slider = this.controls.find(control => control.id === this.focus && control.slider);
    if (slider && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      const current = slider.slider!.current();
      if (slider.disabled || current === null) return;
      const value = event.key === "Home" ? 0 : event.key === "End" ? 100 : current +
        (event.key === "PageUp" ? 10 : event.key === "PageDown" ? -10 : ["ArrowLeft", "ArrowDown"].includes(event.key) ? -1 : 1);
      slider.slider!.change(Math.max(0, Math.min(100, value))); return;
    }
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
      if (event.key === " " || event.key === "Enter") {
        event.preventDefault();
        if (this.notice?.retry) { this.controls.find(control => control.id === "notice-close")?.actions[0]?.run(); return; }
        const next = this.controls.find(control => control.id === "entry-error-next");
        if (next) next.actions[0]?.run(); else this.cancel();
      }
      return;
    }
    if (this.state.world && gameplayUi(this.state.world)?.confirmation && event.key === " ") {
      event.preventDefault(); this.controls.find(control => control.id === "ui-confirm-accept")?.actions[0]?.run(); return;
    }
    if (this.state.world && gameplayUi(this.state.world)?.reward && (event.key === " " || event.key === "Enter")) {
      const reward = gameplayUi(this.state.world)!.reward!;
      event.preventDefault();
      const level = this.controls.find(control => control.id === "level-up-continue");
      if (level) level.actions[0]?.run(); else this.continueReward(reward.id, reward.continuation);
      return;
    }
    if (this.state.world && gameplayUi(this.state.world)?.document &&
        ["ArrowLeft", "ArrowRight", "PageUp", "PageDown"].includes(event.key)) {
      event.preventDefault();
      const id = event.key === "ArrowLeft" || event.key === "PageUp" ? "document-previous" : "document-next";
      this.controls.find(control => control.id === id)?.actions[0]?.run();
      return;
    }
    const production = this.state.world ? gameplayUi(this.state.world)?.production : null;
    if (production) {
      const shortcut = event.key === " " ? "space" : event.key.toLowerCase();
      const control = this.controls.find(control => control.productionRecipe && control.shortcut === shortcut);
      if (control) {
        event.preventDefault();
        const action = control.actions[0];
        const reason = control.disabled ?? action?.disabled;
        if (reason) this.show(reason, "error"); else action?.run();
        return;
      }
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
      event.preventDefault();
      const chat = gameplayUi(this.state.world)?.publicChat;
      if (chat?.permission.allowed) {
        this.local.chatDraft += event.key; this.renderSoon();
        requestAnimationFrame(() => this.surface.focus("public-chat"));
      } else this.required("Chat messages", "send_chat_intent");
    }
  }

  private openTab(index: number): void {
    const world = this.state.world, tab = TABS[index];
    if (!world || !tab) return;
    if (!isInterfaceUnlocked(world, tab.interface)) {
      const declared = gameplayUi(world)?.interfaces.find(row => row.interface === tab.interface);
      this.show(permissionReason(declared?.permission, tab.name) ?? "This interface is locked.", "error", declared?.permission.code ?? "ui.interface.locked");
      return;
    }
    if (!gameplayUi(world)) this.local.tab = index;
    this.local.scroll = 0; this.menu = null; this.local.filterPanel = null;
    this.local.musicDropdown = false;
    if (this.assets.catalogue.presentation?.interfaces[tab.interface]) this.send({ kind: "open_interface", interface: tab.interface });
    else this.unavailable(tab.name);
    this.renderSoon();
  }

  private pointer(event: PointerEvent, control: Control | null, phase: "down" | "move" | "up"): void {
    const [x, y] = this.xy(event); this.point = { x, y };
    if (this.scrollDrag && phase !== "down") {
      const control = this.controls.find(control => control.id === this.scrollDrag!.id);
      if (phase === "move" && control?.scrollbar && event.buttons & 1)
        control.scrollbar.change(musicScrollPosition(y - control.y, control.height, control.scrollbar.thumb, control.scrollbar.maximum, this.scrollDrag.grab));
      if (phase === "up") this.scrollDrag = null;
      this.renderSoon(); return;
    }
    if (this.sliderDrag && phase !== "down") {
      const slider = this.controls.find(control => control.id === this.sliderDrag!.id);
      if (phase === "move" && slider?.slider && !slider.disabled && event.buttons & 1)
        slider.slider.change(audioSliderPercent(x - slider.x, slider.width, this.sliderDrag.grab));
      if (phase === "up") this.sliderDrag = null;
      this.renderSoon(); return;
    }
    if (phase === "move") {
      if (this.drag && (event.buttons & 1) && performance.now() - this.drag.start >= 100 &&
          Math.max(Math.abs(x - this.drag.x), Math.abs(y - this.drag.y)) >= 5) this.drag.active = true;
      if (this.bankDrag && (event.buttons & 1) && performance.now() - this.bankDrag.start >= 100 &&
          Math.max(Math.abs(x - this.bankDrag.x), Math.abs(y - this.bankDrag.y)) >= 5) this.bankDrag.active = true;
      this.renderSoon(); return;
    }
    if (phase === "down") {
      this.suppressNextClick = false;
      if (this.menu && !control?.id.startsWith("menu-")) { this.menu = null; this.suppressNextClick = true; this.renderSoon(); return; }
      if (this.local.musicDropdown && !control?.id.startsWith("music-filter-") && control?.id !== "music-list-filter") {
        this.local.musicDropdown = false; this.suppressNextClick = true; this.renderSoon(); return;
      }
      if (this.local.settings.choice && !control?.id.startsWith("settings-option-")) {
        this.local.settings.choice = null; this.suppressNextClick = true; this.renderSoon(); return;
      }
      if (event.button !== 0 || this.notice || this.local.amount || control?.disabled) return;
      if (control?.scrollbar) {
        const scroll = control.scrollbar, top = 16 + Math.trunc((control.height - 32 - scroll.thumb) * scroll.value / Math.max(1, scroll.maximum));
        if (y < control.y + 16) scroll.change(Math.max(0, scroll.current() - 4));
        else if (y >= control.y + control.height - 16) scroll.change(Math.min(scroll.maximum, scroll.current() + 4));
        else {
          const grab = y >= control.y + top && y < control.y + top + scroll.thumb ? y - control.y - top : Math.trunc(scroll.thumb / 2);
          this.scrollDrag = { id: control.id, grab };
          scroll.change(musicScrollPosition(y - control.y, control.height, scroll.thumb, scroll.maximum, grab));
          if (event.isTrusted && event.target instanceof Element) event.target.setPointerCapture(event.pointerId);
        }
        return;
      }
      if (control?.slider && control.slider.value !== null) {
        const left = control.x + Math.trunc(control.slider.value * (control.width - 16) / 100);
        const grab = x >= left && x < left + 16 ? x - left : 0;
        this.sliderDrag = { id: control.id, grab };
        control.slider.change(audioSliderPercent(x - control.x, control.width, grab));
        if (event.isTrusted && event.target instanceof Element) event.target.setPointerCapture(event.pointerId);
        return;
      }
      if (control?.bankEntryId && this.state.world) {
        const bank = gameplayUi(this.state.world)?.bank;
        const entry = bank?.entries.find(entry => entry.id === control.bankEntryId);
        if (entry && bank) this.bankDrag = { entry: entry.id, item: entry.item, instance: entry.value?.instanceId ?? null, revision: bank.revision,
          x, y, start: performance.now(), active: false };
      }
      if (control?.draggableSlot !== undefined) {
        const item = this.inventory(control.draggableSlot);
        if (item) this.drag = { slot: control.draggableSlot, x, y, start: performance.now(), active: false,
          identity: { slot: control.draggableSlot, id: item.id, instanceId: item.instanceId, name: item.name } };
      }
    } else if (this.bankDrag) {
      const drag = this.bankDrag; this.bankDrag = null;
      if (drag.active) {
        this.suppressNextClick = true;
        const bank = this.state.world ? gameplayUi(this.state.world)?.bank : null;
        const current = bank?.entries.find(entry => entry.id === drag.entry);
        const target = this.controls.find(control => (control.bankEntryId || control.bankTab !== undefined || control.bankCreate) && contains(control, x, y));
        if (!current || current.item !== drag.item || (current.value?.instanceId ?? null) !== drag.instance)
          this.show("The dragged bank entry changed. Choose it again.", "error", "ui.bank.stale");
        else if (target?.bankCreate) this.sendUi({ kind: "bank_create_tab", entry_id: drag.entry }, drag.revision);
        else if (target && target.bankEntryId !== drag.entry) this.sendUi({
          kind: "bank_move", entry_id: drag.entry, before_entry_id: target.bankEntryId ?? null,
          tab: target.bankTab ?? bank!.entries.find(entry => entry.id === target.bankEntryId)?.tab ?? bank!.selectedTab,
        }, drag.revision);
        setTimeout(() => { this.suppressNextClick = false; }, 0);
      }
      this.renderSoon();
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
    const projection = gameplayUi(world);
    if (this.local.selectedItem) return [
      { label: `Use ${escapeText(this.local.selectedItem.name)} -> ${label}`, run: this.validSlot(slot, () => this.useSelection({ kind: "inventory", slot })) },
      { label: `Examine ${label}`, run: () => this.examine(item) },
    ];
    if (projection?.bank && !world.bank) {
      return [{ label: `Deposit ${label}`, run: () => this.required("Deposit inventory", "contextual_banker_identity") }];
    }
    if (projection ? projection.bank && world.bank : world.bank) {
      const bank = world.bank;
      if (!bank) return [];
      const deposit = (quantity: number) => this.validSlot(slot, () => this.send({ kind: "bank_deposit", banker: bank.banker, inventory_slot: slot, quantity }))();
      const options: UiAction[] = [1, 5, 10].map(q => ({ label: `Deposit-${q} ${label}`, run: () => deposit(q) }));
      options.push({ label: `Deposit-X ${label}`, run: () => this.prompt("Enter amount:", deposit) },
        { label: `Deposit-All ${label}`, run: this.validSlot(slot, item => deposit(item.quantity)) },
        { label: `Examine ${label}`, run: () => this.examine(item) });
      const selected = projection?.bank ? projection.bank.amount : this.local.bankAmount;
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
    if (projection) {
      const row = projection.inventoryActions.find(row => row.slot === slot && row.item === item.id && row.instance === item.instanceId);
      const actions: UiAction[] = row?.actions.map(action => ({
        label: `${escapeText(action.label)} ${label}`,
        run: () => this.sendUi({ kind: "item_action", inventory_slot: slot, expected_item: item.id, expected_instance: item.instanceId, action: action.id }),
        ...(permissionReason(action.permission, action.label) ? { disabled: permissionReason(action.permission, action.label)! } : {}),
      })) ?? [{ label: `Item actions ${label}`, disabled: "The projection did not supply actions for this item identity.", run: () => {} }];
      actions.push({ label: `Use ${label}`, run: this.validSlot(slot, current => {
        this.local.selectedItem = { slot, id: current.id, instanceId: current.instanceId, name: current.name };
        this.local.selectedSpell = null; this.renderSoon();
      }) });
      const offer = projection.recovery?.cofferItems.find(row => row.slot === slot && row.item === item.id && row.instance === item.instanceId);
      if (offer) {
        const reason = permissionReason(projection.recovery?.cofferOffer, "Coffer offer") ??
          (offer.actions.some(action => action.permission.allowed) ? undefined : permissionReason(offer.actions[0]?.permission, "Offer item"));
        actions.push({ label: `Offer to Death's Coffer ${label}`, run: () => this.prompt("Offer how many?", quantity =>
          this.sendUi({ kind: "coffer_offer", inventory_slot: slot, expected_item: item.id, expected_instance: item.instanceId, quantity })),
        ...(reason ? { disabled: reason } : {}) });
      }
      actions.push({ label: `Examine ${label}`, run: () => this.examine(item) });
      return actions;
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

  private bankEntryActions(entryId: string): UiAction[] {
    const world = this.state.world, bank = world ? gameplayUi(world)?.bank : null;
    const entry = bank?.entries.find(row => row.id === entryId);
    if (!bank || !entry) return [];
    const label = `<col=ff9040>${escapeText(entry.value?.name ?? entry.item)}</col>`;
    const current = () => {
      const view = this.state.world ? gameplayUi(this.state.world)?.bank : null;
      const row = view?.entries.find(row => row.id === entryId);
      if (!view || !row || row.item !== entry.item || row.value?.instanceId !== entry.value?.instanceId) {
        this.show("That bank entry identity changed. Choose the current entry again.", "error", "ui.bank.stale"); return null;
      }
      return { view, row };
    };
    const withdraw = (quantity: number) => {
      const value = current();
      if (value) this.sendUi({ kind: "bank_withdraw_entry", entry_id: entryId, quantity, noted: bank.noted }, bank.revision);
    };
    const actions: UiAction[] = entry.placeholder ? [{
      label: `Release placeholder ${label}`, run: () => { if (current()) this.sendUi({ kind: "bank_release_placeholder", entry_id: entryId }, bank.revision); },
    }] : [
      { label: `Withdraw-${bank.amount} ${label}`, run: () => withdraw(bank.amount) },
      ...[1, 5, 10].filter(quantity => quantity !== bank.amount).map(quantity => ({ label: `Withdraw-${quantity} ${label}`, run: () => withdraw(quantity) })),
      { label: `Withdraw-X ${label}`, run: () => this.prompt("Enter amount:", withdraw) },
      { label: `Withdraw-All ${label}`, run: () => { const value = current(); if (value?.row.value) withdraw(value.row.value.quantity); } },
      { label: `Withdraw-All-but-1 ${label}`, run: () => {
        const value = current(); if (value?.row.value) withdraw(value.row.value.quantity - 1);
      }, ...(entry.value && entry.value.quantity > 1 ? {} : { disabled: "There is only one item in this stack." }) },
      { label: `Placeholder ${label}`, run: () => {
        if (current()) this.sendUi({ kind: "bank_placeholder", entry_id: entryId }, bank.revision);
      } },
    ];
    actions.push({ label: `Create tab ${label}`, run: () => { if (current()) this.sendUi({ kind: "bank_create_tab", entry_id: entryId }, bank.revision); } });
    for (const tab of bank.tabs) actions.push({
      label: `Move to tab ${tab.tab} ${label}`,
      run: () => { if (current()) this.sendUi({ kind: "bank_move", entry_id: entryId, before_entry_id: null, tab: tab.tab }, bank.revision); },
    });
    actions.push({ label: `Examine ${label}`, run: () => {
      if (entry.value) this.examine(entry.value);
      else {
        const source = this.assets.catalogue.presentation?.sourceItems[entry.item];
        this.show(source === undefined ? entry.item : this.assets.catalogue.items[source]?.examine || entry.item);
      }
    } });
    return actions;
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
    const pending = this.pending.size, notice = this.notice;
    this.amountRevision = this.state.world?.revision ?? null;
    prompt.pending = true;
    prompt.confirm(quantity);
    if (this.pending.size <= pending) {
      prompt.pending = false;
      if (this.notice === notice) this.local.amount = null;
    }
    this.renderSoon();
  }
  private async depositAll(): Promise<void> {
    if (this.state.world?.ui !== undefined) {
      const problem = gameplayUiProblem(this.state.world);
      if (problem) { this.show(problem.message, "error", problem.code); return; }
    }
    const bank = this.state.world?.bank;
    if (!bank) { this.required("Deposit inventory", "contextual_banker_identity"); return; }
    const slots = this.state.world!.player.inventory.filter(s => s.item).map(s => ({ index: s.index, id: s.item!.id, instance: s.item!.instanceId }));
    for (const slot of slots) {
      if (this.state.world?.bank?.banker !== bank.banker) { this.show("The bank has closed.", "error", "ui.bank.closed"); return; }
      const item = this.inventory(slot.index);
      if (!item || item.id !== slot.id || item.instanceId !== slot.instance) { this.show("The inventory changed during deposit.", "error", "ui.inventory.changed"); return; }
      if (!await this.request(`deposit-all-${slot.index}`, () => this.services.send({
        kind: "bank_deposit", banker: bank.banker, inventory_slot: slot.index, quantity: item.quantity,
      }))) return;
    }
  }

  capturesPointer(x: number, y: number): boolean {
    if (this.disposed) return false;
    if (!this.state.world || this.state.phase !== "world" || this.notice || this.local.amount ||
        this.sliderDrag || this.scrollDrag || this.local.musicDropdown || this.local.settings.choice ||
        (this.state.error && this.state.error !== this.dismissedError)) return true;
    const presentation = gameplayUi(this.state.world);
    if (presentation?.reward || presentation?.confirmation) return true;
    if (this.menu && contains({ ...this.menu, height: this.menu.actions.length * 15 + 22 }, x, y)) return true;
    return Object.values(frameRegions(this.canvas.width, this.canvas.height)).some(rect => contains(rect, x, y)) ||
      this.panelBounds.some(rect => contains(rect, x, y)) ||
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
    if (pointer.kind === "context" || pointer.kind === "primary" && this.local.clientInput.singleMouse &&
        !this.local.selectedItem && !this.local.selectedSpell && actions.length > 1)
      this.openMenu(actions, pointer.x, pointer.y);
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
    this.panelBounds = [];
    this.raster.clear();
    const world = this.state.world;
    const error = this.notice?.scope === "error" ? this.notice : this.state.error;
    this.previewBounds = null; this.previewRequest = null;
    const gameplay = world ? gameplayUi(world) : null;
    const character = gameplay ? gameplay.activeInterface === "interface.appearance" ||
      this.state.phase === "character" && !gameplay.appearance.confirmed
      : this.state.phase === "character" || world?.player.tutorialStage === "stage.tutorial.appearance";
    if (!world && character) {
      this.previewBounds = paintCharacter(this.raster, this.local.appearance, controls,
        body => { this.local.appearance.body_type = body; this.renderSoon(); },
        () => this.confirmAppearance(),
        (label, field) => this.required(label, field), undefined,
        (bounds, model) => this.recordPreview("appearance", bounds, model));
      if (this.preview && this.previewBounds) {
        const bounds = this.previewBounds;
        if (this.preview.width === bounds.width && this.preview.height === bounds.height)
          this.raster.clip(bounds, () => this.raster.context.drawImage(this.preview!, bounds.x, bounds.y));
        else this.show("The renderer's character preview does not match the native preview dimensions.", "error", "ui.preview.size");
      }
    } else if (!world || (this.state.phase !== "world" && this.state.phase !== "reconnecting" && this.state.phase !== "character")) {
      const cycle = Math.floor((performance.now() - this.titleStartedAt) / 20);
      this.titleFlames.advance(cycle);
      paintEntry(this.raster, { state: this.state, name: this.name, password: this.password, confirmation: this.confirmation,
        ...(this.audioView ? { audioEnabled: this.audioView.enabled } : {}),
        focus: this.focus, hideName: this.hideName, busy: this.pending.has("auth"), error,
        errorKind: this.entryErrorKind, cursorVisible: cycle % 40 < 20,
        errorPage: this.entryErrorPage,
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
        nextErrorPage: () => { this.entryErrorPage++; this.renderSoon(); },
        unavailable: name => this.unavailable(name),
      }, controls, inputs, this.titleFlames);
    } else {
      const audioEpoch = this.audioEpoch;
      paintGame(this.raster, {
        state: this.state, world, local: this.local, controls, inputs, minimap: this.minimap,
        abilityVisuals: this.abilityVisuals?.revision === world.revision ? this.abilityVisuals.values : {},
        send: intent => this.send(intent), sendUi: intent => this.sendUi(intent, world.ui?.bank?.revision), openTab: tab => this.openTab(tab),
        change: change => { change(); this.renderSoon(); },
        notice: (message, scope = "information", id) => this.show(message, scope, id),
        unavailable: name => this.unavailable(name), required: (name, field) => this.required(name, field),
        prompt: (label, confirm) => this.prompt(label, confirm),
        inventoryActions: slot => this.inventoryActions(slot), equipmentActions: slot => this.equipmentActions(slot),
        bankActions: slot => this.bankActions(slot), bankEntryActions: entry => this.bankEntryActions(entry), shopActions: slot => this.shopActions(slot),
        depositAll: () => { void this.request("deposit-inventory", () => this.depositAll()); },
        logout: () => { void this.request("logout", () => this.services.logout()); },
        confirmAppearance: () => this.confirmAppearance(),
        minimapClick: widget => {
          if (!this.minimap.enabled || !this.minimap.status()) {
            this.show("The dynamic minimap is not available.", "error", "ui.minimap.unavailable"); return;
          }
          const destination = this.minimap.destination(widget, world.player.tile, this.point.x, this.point.y);
          if (destination) this.send({ kind: "walk", destination, running: world.player.settings.find(s => s.setting === "run")?.enabled ?? false });
        },
        faceNorth: () => this.cameraRequest ? this.cameraRequest(0) : this.required("Camera rotation", "camera_request_adapter"),
        sendChat: () => { void this.sendChat(); },
        continueReward: (id, continuation) => this.continueReward(id, continuation),
        hoveredProduction: this.hover?.productionRecipe ?? null,
        hoveredControl: this.hover?.id ?? null,
        presentationCurrent: (kind, id, page) => {
          const current = this.state.world ? gameplayUi(this.state.world) : null;
          const matches = kind === "document" ? current?.document?.id === id &&
            (page === undefined || current.document.page === page) : current?.reward?.id === id;
          if (!matches) this.show("That presentation changed. Review the current interface.", "error", "ui.presentation.stale");
          return matches;
        },
        audio: this.audioView, audioPercent: (channel, percent) => this.audioPercent(channel, percent, audioEpoch),
        audioValue: channel => this.audioView?.percentages[channel] ?? null,
        audioMute: channel => this.audioMute(channel, audioEpoch),
        audioToggle: () => this.audio(),
        music: this.musicState?.playerId === world.player.id ? this.musicState.value : null,
        musicAction: action => this.changeMusic(action, audioEpoch),
        focusInput: id => requestAnimationFrame(() => { if (!this.disposed) this.surface.focus(id); }),
        capture: bounds => this.panelBounds.push(bounds),
        preview: (bounds, model) => {
          this.recordPreview("equipment", bounds, model);
          if (this.preview) {
            if (this.preview.width === bounds.width && this.preview.height === bounds.height)
              this.raster.context.drawImage(this.preview, bounds.x, bounds.y);
            else this.show("The renderer's equipment preview does not match the native preview dimensions.", "error", "ui.preview.size");
          }
        },
      });
      if (character) {
        const characterControls: Control[] = [];
        this.previewBounds = paintCharacter(this.raster, this.local.appearance, characterControls,
          body => { this.local.appearance.body_type = body; this.renderSoon(); },
          () => this.confirmAppearance(),
          (label, field) => this.required(label, field), gameplayUi(world)?.appearance,
          (bounds, model) => this.recordPreview("appearance", bounds, model));
        const hudControls = controls.filter(control => !control.id.startsWith("appearance-"));
        controls.length = 0; controls.push(...hudControls, ...characterControls);
        if (this.preview && this.previewBounds) {
          const bounds = this.previewBounds;
          if (this.preview.width === bounds.width && this.preview.height === bounds.height)
            this.raster.clip(bounds, () => this.raster.context.drawImage(this.preview!, bounds.x, bounds.y));
          else this.show("The renderer's character preview does not match the native preview dimensions.", "error", "ui.preview.size");
        }
      }
    }
    if (this.drag?.active) {
      const item = this.inventory(this.drag.slot);
      if (item?.sourceId !== null && item?.sourceId !== undefined) this.raster.item(item.sourceId, item.quantity, this.point.x - 18, this.point.y - 16, 2, false, 128);
    }
    if (gameplay?.reward && !((gameplay.reward.kind === "quest" &&
        this.assets.catalogue.presentation?.interfaces[gameplay.reward.interface]?.sourceIds.includes(153)) ||
        (gameplay.reward.kind === "level_up" &&
        this.assets.catalogue.presentation?.interfaces[gameplay.reward.interface]?.sourceIds.includes(233)))) {
      const reward = gameplay.reward;
      controls.length = 0; inputs.length = 0;
      const message = `Required source layout unavailable for ${reward.interface} (${reward.kind}).\n${reward.title}\n` +
        rewardDetails(reward, world!.player.skills).join("\n");
      this.surface.announce(`${message}\nError ID: ui.source.reward.layout`);
      this.paintChatOverlay(message, "Click here to continue", controls);
      const continuation = controls.find(control => control.id === "notice-close");
      const lines = entryErrorLines(message, 472, this.assets.catalogue.fonts[495]!);
      if (continuation && (this.local.dialoguePage + 1) * 5 >= lines.length)
        continuation.actions = [{ label: "Continue presentation", run: () => this.continueReward(reward.id, reward.continuation) }];
    }
    if (gameplay?.confirmation && !this.notice && !this.local.amount) {
      controls.length = 0; inputs.length = 0;
      paintConfirmation(this.raster, gameplay.confirmation, this.local.dialoguePage, controls,
        intent => this.sendUi(intent), () => { this.local.dialoguePage++; this.renderSoon(); });
    }
    if (this.local.amount && world) {
      controls.length = 0; inputs.length = 0;
      this.paintChatOverlay(this.local.amount.label, "", controls);
      const rect = frameRegions(this.canvas.width, this.canvas.height).chat;
      this.raster.center(escapeText(this.local.amount.value) + "<col=0000ff>*</col>", rect.x + 259, rect.y + 88, 496, 0, null);
      inputs.push({ id: "amount", label: this.local.amount.label, x: rect.x + 130, y: rect.y + 68, width: 260, height: 26,
        type: "text", inputMode: "numeric", autocomplete: "off", maximum: 16, value: this.local.amount.value,
        readOnly: Boolean(this.local.amount.pending),
        change: value => { if (this.local.amount) this.local.amount.value = value; this.renderSoon(); }, submit: () => this.confirmAmount() });
      controls.push({ id: "amount-confirm", label: "Confirm amount", x: rect.x + 140, y: rect.y + 105, width: 239, height: 22,
        ...(this.local.amount.pending ? { disabled: "Waiting for an authoritative update." } : {}),
        actions: [{ label: "Confirm amount", run: () => this.confirmAmount() }] });
      this.raster.center(this.local.amount.pending ? "Waiting for server..." : "Press Enter to confirm", rect.x + 259, rect.y + 121, 495, 0x0000ff, null);
    }
    const visibleError = this.state.error && this.state.error !== this.dismissedError ? this.state.error : null;
    const overlay = this.notice || (world || character ? visibleError : null);
    if (overlay && (world || character || this.notice?.scope !== "error")) {
      controls.length = 0; inputs.length = 0;
      if (world) this.paintChatOverlay(overlay.errorId ? `${overlay.message}\nError ID: ${overlay.errorId}` : overlay.message,
        this.notice?.retry ? "Click here to retry saving" : "Click here to continue", controls, this.notice?.retry);
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
      paintReconnect(this.raster);
    }
    if (this.hover && this.hover.id !== "world") this.hover = controls.find(control => control.id === this.hover!.id) ?? null;
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

  private recordPreview(purpose: UiPreviewRequest["purpose"], bounds: Rect, model: NativeWidget): void {
    const world = this.state.world;
    this.previewBounds = { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height };
    this.previewRequest = {
      purpose, bounds: this.previewBounds,
      modelBounds: { x: model.x, y: model.y, width: model.width, height: model.height },
      sourceWidget: model.id, modelZoom: model.modelZoom, modelRotation: [...model.modelRotation],
      appearance: { ...(purpose === "appearance" ? this.local.appearance : world?.player.appearance) },
      equipment: world ? structuredClone(world.player.equipment) : null,
      base: world ? gameplayUi(world)?.appearance.base ?? null : null,
    };
  }

  private paintChatOverlay(message: string, continuation: string, controls: Control[], resume?: () => void): void {
    const chat = frameRegions(this.canvas.width, this.canvas.height).chat;
    this.raster.sprite(1017, chat.x, chat.y);
    const lines = entryErrorLines(message, 472, this.assets.catalogue.fonts[495]!);
    const pageSize = 5;
    const page = Math.min(this.local.dialoguePage, Math.max(0, Math.ceil(lines.length / pageSize) - 1));
    this.raster.textBox(lines.slice(page * pageSize, (page + 1) * pageSize).join("<br>"),
      { x: chat.x + 20, y: chat.y + 10, width: 472, height: 88 }, { font: 495, color: 0, shadow: null, xAlign: 1, yAlign: 1, lineHeight: 16 });
    if (continuation) {
      const more = (page + 1) * pageSize < lines.length;
      this.raster.center(more ? "Click here to continue" : continuation, chat.x + 259, chat.y + 121, 495, 0x0000ff, null);
      controls.push({ id: "notice-close", label: more ? "Continue message" : resume ? "Retry saving audio preferences" : "Continue", x: chat.x + 8, y: chat.y + 100, width: 506, height: 29,
        actions: [{ label: "Continue", run: () => {
          if (more) this.local.dialoguePage++; else { this.local.dialoguePage = 0; if (resume) resume(); else this.cancel(); }
          this.renderSoon();
        } }] });
    }
  }
}
