import type { AppServices, AppState, AudioEvent, GameIntent, WorldView } from "../shared/contracts.ts";
import { AppError, appError, deepFreeze, invariant } from "./errors.ts";
import type { DisplayCatalog } from "./manifest.ts";
import type { AudioChannel } from "./settings.ts";
import { RpcTransport } from "./transport.ts";
import { presenceOf } from "./public-state.ts";
import type { PublicWorld, QuoteRequest, QuoteView, ShopPurchaseIntent } from "./public-state.ts";
import { captureUiBankRevision, gameplayUiSupport, validateActorObservers } from "./gameplay-ui.ts";
import type { GameplayUiSupport } from "./gameplay-ui.ts";
import type { SourceAudioSession } from "./audio.ts";
import type { PlayerAudioCompositionStatus } from "./player-audio-composition.ts";
import { validateWorldAuthority } from "./authority.ts";

export interface WasmClient {
  prepare(requestId: string, operation: string, input: string): Uint8Array;
  submit(requestId: string, intent: string): Uint8Array;
  submit_selected(requestId: string, intent: string, itemId: string): Uint8Array;
  retry_uncertain_input(): Uint8Array | undefined;
  retry_lifecycle(): Uint8Array | undefined;
  receive(bytes: Uint8Array): string;
  receive_for(requestId: string, bytes: Uint8Array): string;
  request_id(bytes: Uint8Array): string;
  request_is_shop_buy(bytes: Uint8Array): boolean;
  state(): string;
  set_catalog(input: string): string;
  transport_lost(): void;
  authorization(): string | undefined;
  free(): void;
}

export interface BridgeState {
  version: 1;
  phase: "disconnected" | "connecting" | "signing_in" | "account_ready" | "joining_world" | "in_world" | "reconnecting";
  authenticated: boolean;
  accountName: string | null;
  characterInitialized: boolean;
  gameplayAvailable: boolean;
  capabilities: string[];
  gameplayUiWireSupported: boolean;
  unavailableReason: string | null;
  serverBuild: string | null;
  contentRevision: string | null;
  contentManifestPath: string | null;
  world: PublicWorld | null;
  events: AudioEvent[];
  nextSequence: string | null;
  uncertainInput: boolean;
  worldJoined: boolean;
  quote: QuoteView | null;
  quoteError: string | null;
}

export interface ClientHooks {
  content(revision: string, path: string): Promise<DisplayCatalog>;
  prepareWorld(world: WorldView): Promise<void>;
  prepareAudio?(world: WorldView): Promise<void>;
  events(world: WorldView | null, events: readonly AudioEvent[]): void;
  unlockAudio(): Promise<void>;
  audioEnabled?(): boolean;
  audioControls?(): ReturnType<SourceAudioSession["controls"]> | null;
  audioPreferenceStatus?(): Readonly<PlayerAudioCompositionStatus> | null;
  volume(channel: AudioChannel, value: number): void;
  disconnected(): void;
  componentFailure(error: AppError): void;
}

export function bridgeState(json: string): Readonly<BridgeState> {
  const state = JSON.parse(json) as BridgeState;
  invariant(state.version === 1 && typeof state.authenticated === "boolean", "Invalid WASM state envelope.", "protocol");
  invariant(Array.isArray(state.capabilities) && state.capabilities.every((value) => typeof value === "string")
    && typeof state.gameplayUiWireSupported === "boolean", "Invalid WASM capability negotiation state.", "protocol");
  const support = gameplayUiSupport(state.capabilities, state.gameplayUiWireSupported, state.world);
  validateActorObservers(state.capabilities, state.world);
  validateWorldAuthority(state.capabilities, state.world);
  if (state.world !== null && support.reason === "view_missing") {
    throw new AppError(support.message!, { kind: "unsupported_protocol" });
  }
  invariant(state.nextSequence === null || (typeof state.nextSequence === "string" && /^[1-9][0-9]*$/.test(state.nextSequence)), "Invalid WASM sequence.", "protocol");
  if (state.world) {
    invariant(typeof state.world.revision === "string" && /^\d+$/.test(state.world.revision)
      && typeof state.world.tick === "string" && /^\d+$/.test(state.world.tick)
      && state.world.player.skills.every((skill) => typeof skill.xpTenths === "string" && /^\d+$/.test(skill.xpTenths)), "WASM U64 values must remain decimal strings.", "protocol");
    for (const view of state.world.recoveryContext?.views ?? []) {
      invariant(view.items.every((entry) => typeof entry.fullEntryFee === "string" && /^\d+$/.test(entry.fullEntryFee)),
        "Recovery fees must remain exact U64 decimal strings.", "protocol");
    }
  }
  if (state.quote?.kind === "recovery") {
    invariant(typeof state.quote.fullSelectionFee === "string" && /^\d+$/.test(state.quote.fullSelectionFee),
      "Recovery quote fees must remain exact U64 decimal strings.", "protocol");
  }
  return deepFreeze(state);
}

export class BrowserApp implements AppServices {
  #bridge: WasmClient;
  #transport: RpcTransport;
  #hooks: ClientHooks;
  #state: Readonly<AppState> = deepFreeze({
    phase: "capability_check", loading: null, accountName: null, error: null, world: null, soundEnabled: false,
  });
  #listeners = new Set<(state: Readonly<AppState>) => void>();
  #tail: Promise<unknown> = Promise.resolve();
  #queued = 0;
  #generation = 0;
  #disposed = false;
  #terminal = false;
  #poll: ReturnType<typeof setTimeout> | undefined;
  #reconnect: ReturnType<typeof setTimeout> | undefined;
  #reconnectAttempts = 0;
  #worldPrepared: string | null = null;
  #logoutRequested = false;
  #pendingExits = 0;
  #uiWarning: string | null = null;
  #uiSupport: Readonly<GameplayUiSupport> = gameplayUiSupport([], false, null);

  constructor(bridge: WasmClient, transport: RpcTransport, hooks: ClientHooks) {
    this.#bridge = bridge;
    this.#transport = transport;
    this.#hooks = hooks;
  }

  state(): Readonly<AppState> { return this.#state; }
  gameplayUi(): Readonly<GameplayUiSupport> { return this.#uiSupport; }
  audioControls(): ReturnType<SourceAudioSession["controls"]> | null { return this.#hooks.audioControls?.() ?? null; }
  audioPreferenceStatus(): Readonly<PlayerAudioCompositionStatus> | null { return this.#hooks.audioPreferenceStatus?.() ?? null; }
  subscribe(listener: (state: Readonly<AppState>) => void): () => void {
    this.#listeners.add(listener);
    try { listener(this.#state); }
    catch (error) {
      this.#listeners.delete(listener);
      throw error;
    }
    return () => this.#listeners.delete(listener);
  }

  #publish(patch: Partial<AppState>): void {
    if (this.#disposed || (this.#terminal && patch.phase !== "error")) return;
    this.#state = deepFreeze({ ...this.#state, ...patch });
    for (const listener of this.#listeners) {
      try { listener(this.#state); }
      catch {
        this.#listeners.delete(listener);
        const error = new AppError("A source presentation subscriber could not apply the authoritative state.", {
          kind: "component", recoverable: false,
        });
        // Report off the request stack so an acknowledged write never becomes a retry.
        queueMicrotask(() => { if (!this.#disposed) this.#hooks.componentFailure(error); });
      }
    }
  }

  loading(completed: number, total: number, label: string): void {
    invariant(Number.isSafeInteger(completed) && Number.isSafeInteger(total) && completed >= 0 && completed <= total,
      "Invalid loading observation.");
    this.#publish({ loading: { completed, total, label } });
  }

  async start(): Promise<void> {
    await this.#serial(async () => {
      this.#publish({ phase: "connecting" });
      await this.#request("hello");
      this.#publish({ phase: "title", loading: null });
    });
  }

  async register(loginName: string, password: string): Promise<void> {
    await this.#serial(async () => {
      this.#publish({ phase: "connecting", error: null });
      await this.#request("register", { loginName, password });
      this.#publish({ phase: "login", loading: null });
    }, false);
  }

  async login(loginName: string, password: string): Promise<void> {
    await this.#serial(async () => {
      this.#publish({ phase: "connecting", error: null });
      await this.#request("hello");
      await this.#request("login", { loginName, password });
      const state = await this.#request("account");
      this.#reconnectAttempts = 0;
      this.#logoutRequested = false;
      if (state.characterInitialized && state.gameplayAvailable) {
        await this.#join();
        return;
      }
      this.#publish({
        phase: "character", accountName: state.accountName, world: null, loading: null,
        error: state.gameplayAvailable ? null : {
          message: state.unavailableReason ?? "The account server has not advertised a playable world.",
          errorId: null, recoverable: true,
        },
      });
    }, false);
  }

  async createCharacter(appearance: Record<string, number>): Promise<void> {
    const selection = structuredClone(appearance);
    await this.#serial(async () => {
      const before = bridgeState(this.#bridge.state());
      this.#publish({ phase: "connecting", error: null });
      if (!before.characterInitialized) {
        await this.#request("hello");
        await this.#request("create_character");
      }
      if (!before.worldJoined || before.phase !== "in_world") await this.#join();
      if (Object.keys(selection).length !== 0) {
        const current = bridgeState(this.#bridge.state());
        if (current.world?.player.appearanceConfirmed !== false) {
          throw new AppError("The source appearance is already confirmed or its confirmation state is unavailable.", { kind: "state" });
        }
        await this.#acceptWorld(await this.#exchange(this.#bridge.submit(crypto.randomUUID(), JSON.stringify({
          kind: "confirm_appearance", appearance: selection,
        }))));
      } else {
        await this.#acceptWorld(bridgeState(this.#bridge.state()));
      }
    });
  }

  async enterWorld(): Promise<void> {
    await this.#serial(async () => {
      const state = bridgeState(this.#bridge.state());
      if (!state.authenticated) throw new AppError("Sign in before entering the world.", { kind: "state" });
      this.#logoutRequested = false;
      if (!state.characterInitialized) {
        // This is the source-defined empty creation RPC, not a seeded character.
        await this.#request("hello");
        await this.#request("create_character");
      }
      this.#publish({ phase: "connecting", error: null });
      await this.#join();
    });
  }

  async #join(): Promise<void> {
    this.#worldPrepared = null;
    this.#hooks.disconnected();
    await this.#request("hello");
    const state = await this.#request("join");
    this.#publish({ error: null });
    invariant(state.contentRevision && state.contentManifestPath, "The joined world has no source content manifest.", "protocol");
    const catalog = await this.#hooks.content(state.contentRevision, state.contentManifestPath);
    const installed = bridgeState(this.#bridge.set_catalog(JSON.stringify(catalog)));
    await this.#acceptWorld(installed);
    if (this.#disposed || this.#terminal || this.#pendingExits > 0 || this.#logoutRequested) return;
    const retry = this.#bridge.retry_uncertain_input();
    if (retry !== undefined) {
      const shopBuy = this.#bridge.request_is_shop_buy(retry);
      try { await this.#acceptWorld(await this.#exchange(retry)); }
      catch (value) {
        const error = appError(value);
        if (shopBuy) await this.#refreshRejectedShop(error);
        throw error;
      }
    }
    this.#reconnectAttempts = 0;
    this.#publish({ loading: null });
    this.#schedulePoll();
  }

  async logout(): Promise<void> {
    this.#pendingExits++;
    this.#hooks.disconnected();
    try {
      await this.#serial(async () => {
        this.#logoutRequested = true;
        this.#stopPoll();
        const state = bridgeState(this.#bridge.state());
        if (state.worldJoined && state.phase !== "reconnecting") await this.#request("leave");
        await this.#finishLogout();
      });
    } finally { this.#pendingExits--; }
  }

  async #finishLogout(): Promise<void> {
    const retry = this.#bridge.retry_lifecycle();
    const recovered = retry === undefined ? null : await this.#exchange(retry);
    if (recovered === null || recovered.authenticated) await this.#request("logout");
    this.#logoutRequested = false;
    this.#generation++;
    this.#reconnectAttempts = 0;
    this.#stopPoll();
    if (this.#reconnect !== undefined) clearTimeout(this.#reconnect);
    this.#reconnect = undefined;
    this.#worldPrepared = null;
    this.#hooks.disconnected();
    this.#hooks.events(null, []);
    this.#publish({ phase: "title", accountName: null, world: null, error: null, loading: null });
  }

  async send(intent: GameIntent | ShopPurchaseIntent): Promise<void> {
    // Snapshot the input before it can be mutated by a UI selection/drag update.
    let selection: GameIntent;
    try { selection = captureUiBankRevision(intent, this.#state.world); }
    catch (error) {
      const problem = appError(error);
      this.report(problem);
      throw problem;
    }
    const json = JSON.stringify(selection);
    const kind = intent.kind;
    const itemId = kind === "shop_buy" ? intent.expected_item : null;
    if (kind === "request_logout") {
      this.#pendingExits++;
      this.#hooks.disconnected();
    }
    try { await this.#serial(async () => {
      if (this.#state.phase !== "world") throw new AppError("Wait for the world connection before acting.", { kind: "state" });
      if (presenceOf(this.#state.world)?.acceptsInput === false) {
        throw new AppError("The authoritative presence view does not currently accept game input.", { kind: "state" });
      }
      this.#publish({ error: null });
      if (kind === "shop_buy" && typeof itemId !== "string") {
        throw new AppError("No purchase was sent. Supply expected_item from the displayed row's canonical item.id; an index or numeric source ID is not a purchase identity.", { kind: "input" });
      }
      if (kind === "request_logout") this.#logoutRequested = true;
      const id = crypto.randomUUID();
      let result: Readonly<BridgeState>;
      try {
        result = await this.#exchange(this.#bridge.submit(id, json));
      } catch (value) {
        const error = appError(value);
        if (kind === "shop_buy") await this.#refreshRejectedShop(error);
        throw error;
      }
      await this.#acceptWorld(result);
      if (kind === "request_logout") await this.#finishLogout();
    }); } finally {
      if (kind === "request_logout") this.#pendingExits--;
    }
  }

  async quote(request: QuoteRequest): Promise<Readonly<QuoteView>> {
    const selection = JSON.parse(JSON.stringify(request)) as QuoteRequest;
    let quote: Readonly<QuoteView> | null = null;
    await this.#serial(async () => {
      if (this.#state.phase !== "world") throw new AppError("A joined source context is required for a quote.", { kind: "state" });
      let state: Readonly<BridgeState>;
      try { state = await this.#request("quote", selection); }
      catch (value) {
        const error = appError(value);
        if (selection.kind === "shop_buy") await this.#refreshRejectedShop(error);
        throw error;
      }
      await this.#acceptWorld(state);
      if (state.quoteError) {
        this.#generation++;
        throw new AppError(state.quoteError, { kind: "stale_selection" });
      }
      invariant(state.quote, "The source quote response is missing.", "protocol");
      quote = state.quote;
    });
    invariant(quote, "The source quote was not returned.", "protocol");
    return quote;
  }

  async #refreshRejectedShop(error: AppError): Promise<void> {
    if (error.kind !== "server" || error.code !== 3) return;
    // Rule denials currently share the Conflict envelope. Refresh every rejected
    // buy/quote, retain the original error ID, and never retry or retarget a buy.
    this.#generation++;
    await this.#acceptWorld(await this.#request("poll"));
    error.message = `${error.message} The shop view was refreshed; choose the current item again.`;
  }

  async #acceptWorld(state: Readonly<BridgeState>): Promise<void> {
    const generation = this.#generation;
    const cancelled = (): boolean => this.#disposed || this.#terminal || generation !== this.#generation
      || this.#pendingExits > 0 || this.#logoutRequested;
    if (cancelled()) return;
    this.#uiSupport = gameplayUiSupport(state.capabilities, state.gameplayUiWireSupported, state.world);
    const world = state.world;
    if (!world) return;
    const presence = presenceOf(world);
    if (presence?.presentInWorld === false) {
      this.#stopPoll();
      this.#worldPrepared = null;
      this.#hooks.disconnected();
      this.#publish({
        phase: this.#logoutRequested ? "character" : "error", world: null, accountName: state.accountName,
        error: this.#logoutRequested ? null : {
          message: "The authoritative world presence is offline. Enter the world explicitly to reconnect.",
          errorId: null, recoverable: true,
        },
      });
      return;
    }
    const key = JSON.stringify([state.contentRevision, world.player.id, world.player.region, world.player.instance, world.player.tile.plane]);
    if (key !== this.#worldPrepared) {
      this.#publish({ phase: "connecting", world, accountName: state.accountName });
      await this.#hooks.prepareWorld(world);
      if (cancelled()) return;
      this.#worldPrepared = key;
    }
    if (cancelled()) return;
    if (this.#pendingExits === 0 && !this.#logoutRequested) await this.#hooks.prepareAudio?.(world);
    if (cancelled()) return;
    this.#publish({ world, accountName: state.accountName, phase: "world" });
    if (this.#pendingExits === 0 && !this.#logoutRequested) this.#hooks.events(world, state.events);
    const support = this.#uiSupport;
    if (!support.available && support.message !== this.#uiWarning) {
      this.#uiWarning = support.message;
      this.report(new AppError(support.message!, { kind: "unsupported_capability" }));
    } else if (support.available) {
      this.#uiWarning = null;
    }
    const unavailable = (world as WorldView & { unavailableViews?: Array<{ view: string; reason: string }> }).unavailableViews;
    if (unavailable?.length) {
      this.#publish({ error: {
        message: unavailable.map((entry) => `${entry.view}: ${entry.reason}`).join("\n"),
        errorId: null, recoverable: true,
      } });
    }
  }

  async #request(operation: string, input: unknown = {}): Promise<Readonly<BridgeState>> {
    return this.#exchange(this.#bridge.prepare(crypto.randomUUID(), operation, JSON.stringify(input)));
  }

  async #exchange(bytes: Uint8Array): Promise<Readonly<BridgeState>> {
    try {
      const requestId = this.#bridge.request_id(bytes);
      const token = this.#bridge.authorization();
      const response = await this.#transport.post(bytes, token);
      try {
        const state = bridgeState(this.#bridge.receive_for(requestId, response));
        this.#uiSupport = gameplayUiSupport(state.capabilities, state.gameplayUiWireSupported, state.world);
        return state;
      }
      finally { response.fill(0); }
    } finally { bytes.fill(0); }
  }

  async #serial(action: () => Promise<void>, allowQueue = true): Promise<void> {
    if (this.#disposed) throw new AppError("The client is closed.");
    if (this.#terminal) throw new AppError("Reload after resolving the client capability/component failure.", { kind: "state", recoverable: false });
    if (this.#queued >= 32 || (!allowQueue && this.#queued !== 0)) {
      const error = new AppError("Wait for the outstanding request before trying again.", { kind: "state" });
      this.report(error);
      throw error;
    }
    this.#queued++;
    const generation = this.#generation;
    const operation = this.#tail.then(async () => {
      if (this.#disposed || generation !== this.#generation) {
        throw new AppError("This unsent input was cancelled after the connection or source selection changed.", { kind: "cancelled" });
      }
      try { await action(); }
      catch (value) {
        const error = appError(value);
        if (error.kind !== "cancelled") this.#failed(error);
        throw error;
      }
    }).finally(() => { this.#queued--; });
    this.#tail = operation.catch(() => {});
    await operation;
  }

  #failed(error: AppError): void {
    let state: Readonly<BridgeState> | null = null;
    try { state = bridgeState(this.#bridge.state()); } catch { /* Keep the last valid immutable view. */ }
    if (error.kind === "region_unavailable" || error.kind === "camera_unavailable" || error.kind === "instance_unavailable") {
      this.#stopPoll();
      this.#hooks.disconnected();
      this.#publish({ phase: "error", world: null, loading: null });
      this.report(error);
      return;
    }
    const definitiveServerRejection = error.kind === "server" && error.code !== 7;
    const lost = !definitiveServerRejection && (error.kind === "transport" || error.kind === "protocol"
      || state?.phase === "reconnecting" || this.#state.phase === "reconnecting");
    if (!lost) this.#logoutRequested = false;
    if (!state?.authenticated) {
      this.#stopPoll();
      this.#hooks.disconnected();
      this.#publish({ phase: "login", world: null, accountName: null });
      if (lost) {
        this.#generation++;
        this.#bridge.transport_lost();
      }
    } else if (lost) {
      this.#generation++;
      this.#bridge.transport_lost();
      this.#stopPoll();
      this.#hooks.disconnected();
      this.#publish({ phase: state?.authenticated ? "reconnecting" : "login" });
      if (state?.authenticated && error.recoverable) this.#scheduleReconnect(error.retryAfterSeconds * 1000);
    } else if (this.#state.phase === "connecting" || this.#state.phase === "reconnecting") {
      this.#publish({ phase: state.phase === "in_world" && this.#worldPrepared ? "world" : "character" });
    }
    this.report(error);
    if (!lost && this.#state.phase === "world") this.#schedulePoll();
  }

  #schedulePoll(): void {
    this.#stopPoll();
    if (this.#disposed || this.#state.phase !== "world") return;
    this.#poll = setTimeout(() => {
      this.#poll = undefined;
      if (this.#queued > 0) { this.#schedulePoll(); return; }
      void this.#serial(async () => {
        await this.#acceptWorld(await this.#request("poll"));
      }).then(() => this.#schedulePoll(), () => {});
    }, 600);
  }

  #stopPoll(): void {
    if (this.#poll !== undefined) clearTimeout(this.#poll);
    this.#poll = undefined;
  }

  #scheduleReconnect(minimumMs = 0): void {
    if (this.#disposed || this.#reconnect !== undefined || this.#reconnectAttempts >= 5) return;
    const wait = Math.max(minimumMs, Math.min(8000, 500 * 2 ** this.#reconnectAttempts++));
    this.#reconnect = setTimeout(() => {
      this.#reconnect = undefined;
      void this.reconnect().catch(() => {});
    }, wait);
  }

  async reconnect(): Promise<void> {
    if (!bridgeState(this.#bridge.state()).authenticated) return;
    await this.#serial(async () => {
      this.#publish({ phase: "reconnecting" });
      if (this.#logoutRequested) {
        // Logout itself atomically reconciles the owned source presence/lease.
        // Do not rejoin a body whose requested logout acknowledgement was lost.
        await this.#finishLogout();
        return;
      }
      if (presenceOf(bridgeState(this.#bridge.state()).world)?.presentInWorld === false) {
        this.#publish({ phase: "error", world: null });
        return;
      }
      await this.#request("hello");
      const state = await this.#request("account");
      if (state.characterInitialized) {
        this.#worldPrepared = null;
        await this.#join();
      } else {
        this.#reconnectAttempts = 0;
        this.#publish({ phase: "character", world: null, error: null, accountName: state.accountName });
      }
    });
  }

  setScreen(screen: "title" | "register" | "login"): void {
    if (bridgeState(this.#bridge.state()).authenticated) {
      this.report(new AppError("Sign out before starting another account flow.", { kind: "state" }));
      return;
    }
    if (this.#queued !== 0) return;
    if (screen === "title") this.#hooks.events(null, []);
    this.#publish({ phase: screen, loading: null, error: null });
  }

  audioStatus(enabled: boolean): void {
    this.#publish({
      soundEnabled: enabled,
      ...(enabled && this.#state.error?.message.startsWith("[AUDIO_GESTURE_REQUIRED]") ? { error: null } : {}),
    });
  }

  async unlockAudio(): Promise<void> {
    try {
      // Called synchronously from the UI's trusted gesture before any network await.
      await this.#hooks.unlockAudio();
      this.audioStatus(this.#hooks.audioEnabled?.() === true);
    } catch (value) {
      const error = appError(value, "Browser audio could not be unlocked. Use a trusted pointer/keyboard gesture.");
      this.report(error);
      throw error;
    }
  }

  audioVolume(channel: AudioChannel, value: number): void {
    try { this.#hooks.volume(channel, value); }
    catch (error) {
      const problem = appError(error, "The current entry's audio controls are unavailable.");
      this.report(problem);
      throw problem;
    }
  }

  report(error: Error, errorId?: string): void {
    const safe = error instanceof AppError ? error : new AppError(error.message.slice(0, 2048) || "A client component failed.");
    if (!safe.recoverable) {
      this.#terminal = true;
      this.#generation++;
      this.#stopPoll();
      if (this.#reconnect !== undefined) clearTimeout(this.#reconnect);
      this.#reconnect = undefined;
      this.#bridge.transport_lost();
      this.#transport.abort();
      this.#hooks.disconnected();
    }
    this.#publish({
      error: { message: safe.message, errorId: errorId ?? safe.errorId, recoverable: safe.recoverable },
      ...(safe.recoverable ? {} : { phase: "error" }),
    });
  }

  async dispose(): Promise<void> {
    this.#disposed = true;
    this.#hooks.disconnected();
    this.#generation++;
    this.#stopPoll();
    if (this.#reconnect !== undefined) clearTimeout(this.#reconnect);
    this.#transport.dispose();
    this.#listeners.clear();
    await this.#tail;
    this.#bridge.free();
  }
}
