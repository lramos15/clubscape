import type { AppServices, AppState, AudioEvent, GameIntent, WorldView } from "../shared/contracts.ts";
import { AppError, appError, deepFreeze, invariant } from "./errors.ts";
import type { DisplayCatalog } from "./manifest.ts";
import type { AudioChannel } from "./settings.ts";
import { RpcTransport } from "./transport.ts";

export interface WasmClient {
  prepare(requestId: string, operation: string, input: string): Uint8Array;
  submit(requestId: string, intent: string): Uint8Array;
  retry_uncertain_input(): Uint8Array | undefined;
  receive(bytes: Uint8Array): string;
  receive_for(requestId: string, bytes: Uint8Array): string;
  request_id(bytes: Uint8Array): string;
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
  unavailableReason: string | null;
  serverBuild: string | null;
  contentRevision: string | null;
  contentManifestPath: string | null;
  world: WorldView | null;
  events: AudioEvent[];
  nextSequence: string | null;
  uncertainInput: boolean;
}

export interface ClientHooks {
  content(revision: string, path: string): Promise<DisplayCatalog>;
  prepareWorld(world: WorldView): Promise<void>;
  events(world: WorldView | null, events: readonly AudioEvent[]): void;
  unlockAudio(): Promise<void>;
  volume(channel: AudioChannel, value: number): void;
  disconnected(): void;
}

export function bridgeState(json: string): Readonly<BridgeState> {
  const state = JSON.parse(json) as BridgeState;
  invariant(state.version === 1 && typeof state.authenticated === "boolean", "Invalid WASM state envelope.", "protocol");
  invariant(state.nextSequence === null || (typeof state.nextSequence === "string" && /^[1-9][0-9]*$/.test(state.nextSequence)), "Invalid WASM sequence.", "protocol");
  if (state.world) {
    invariant(typeof state.world.revision === "string" && /^\d+$/.test(state.world.revision)
      && typeof state.world.tick === "string" && /^\d+$/.test(state.world.tick)
      && state.world.player.skills.every((skill) => typeof skill.xpTenths === "string" && /^\d+$/.test(skill.xpTenths)), "WASM U64 values must remain decimal strings.", "protocol");
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

  constructor(bridge: WasmClient, transport: RpcTransport, hooks: ClientHooks) {
    this.#bridge = bridge;
    this.#transport = transport;
    this.#hooks = hooks;
  }

  state(): Readonly<AppState> { return this.#state; }
  subscribe(listener: (state: Readonly<AppState>) => void): () => void {
    this.#listeners.add(listener);
    listener(this.#state);
    return () => this.#listeners.delete(listener);
  }

  #publish(patch: Partial<AppState>): void {
    if (this.#disposed || (this.#terminal && patch.phase !== "error")) return;
    this.#state = deepFreeze({ ...this.#state, ...patch });
    for (const listener of this.#listeners) {
      // A renderer/UI exception must not turn an acknowledged write into a retry.
      try { listener(this.#state); } catch { /* Composition owns component-error reporting. */ }
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
    await this.#serial(async () => {
      if (Object.keys(appearance).length !== 0) {
        throw new AppError("Character creation takes no appearance options. The source appearance confirmation is a sequenced in-world action.", { kind: "input" });
      }
      const before = bridgeState(this.#bridge.state());
      if (before.characterInitialized) throw new AppError("A character already exists. Enter the world to resume it.", { kind: "state" });
      this.#publish({ phase: "connecting", error: null });
      await this.#request("hello");
      await this.#request("create_character");
      await this.#join();
    });
  }

  async enterWorld(): Promise<void> {
    await this.#serial(async () => {
      const state = bridgeState(this.#bridge.state());
      if (!state.authenticated) throw new AppError("Sign in before entering the world.", { kind: "state" });
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
    await this.#request("hello");
    const state = await this.#request("join");
    this.#publish({ error: null });
    invariant(state.contentRevision && state.contentManifestPath, "The joined world has no source content manifest.", "protocol");
    const catalog = await this.#hooks.content(state.contentRevision, state.contentManifestPath);
    const installed = bridgeState(this.#bridge.set_catalog(JSON.stringify(catalog)));
    await this.#acceptWorld(installed);
    const retry = this.#bridge.retry_uncertain_input();
    if (retry !== undefined) await this.#acceptWorld(await this.#exchange(retry));
    this.#reconnectAttempts = 0;
    this.#publish({ phase: "world", loading: null });
    this.#schedulePoll();
  }

  async logout(): Promise<void> {
    await this.#serial(async () => {
      this.#stopPoll();
      const state = bridgeState(this.#bridge.state());
      if (state.phase === "in_world") await this.#request("leave");
      await this.#request("logout");
      this.#generation++;
      this.#reconnectAttempts = 0;
      if (this.#reconnect !== undefined) clearTimeout(this.#reconnect);
      this.#reconnect = undefined;
      this.#worldPrepared = null;
      this.#hooks.disconnected();
      this.#hooks.events(null, []);
      this.#publish({ phase: "title", accountName: null, world: null, error: null, loading: null });
    });
  }

  async send(intent: GameIntent): Promise<void> {
    // Snapshot the input before it can be mutated by a UI selection/drag update.
    const json = JSON.stringify(intent);
    await this.#serial(async () => {
      if (this.#state.phase !== "world") throw new AppError("Wait for the world connection before acting.", { kind: "state" });
      this.#publish({ error: null });
      const result = await this.#exchange(this.#bridge.submit(crypto.randomUUID(), json));
      await this.#acceptWorld(result);
    });
  }

  async #acceptWorld(state: Readonly<BridgeState>): Promise<void> {
    const world = state.world;
    if (!world) return;
    const key = `${state.contentRevision}:${world.player.region}:${world.player.instance ?? ""}:${world.player.tile.plane}`;
    if (key !== this.#worldPrepared) {
      this.#publish({ phase: "connecting", world, accountName: state.accountName });
      await this.#hooks.prepareWorld(world);
      this.#worldPrepared = key;
    }
    this.#publish({ world, accountName: state.accountName, phase: "world" });
    this.#hooks.events(world, state.events);
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
      try { return bridgeState(this.#bridge.receive_for(requestId, response)); }
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
        throw new AppError("This unsent input was cancelled after the connection changed.", { kind: "cancelled" });
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
    const lost = error.kind === "transport" || error.kind === "protocol" || state?.phase === "reconnecting"
      || this.#state.phase === "reconnecting";
    if (lost) {
      this.#generation++;
      this.#bridge.transport_lost();
      this.#stopPoll();
      this.#hooks.disconnected();
      this.#publish({ phase: state?.authenticated ? "reconnecting" : "login" });
      if (state?.authenticated && error.recoverable) this.#scheduleReconnect(error.retryAfterSeconds * 1000);
    } else if (!state?.authenticated) {
      this.#publish({ phase: "login", world: null, accountName: null });
    } else if (this.#state.phase === "connecting") {
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
    this.#publish({ phase: screen, loading: null, error: null });
  }

  async unlockAudio(): Promise<void> {
    try {
      // Called synchronously from the UI's trusted gesture before any network await.
      await this.#hooks.unlockAudio();
      this.#publish({ soundEnabled: true });
    } catch (value) {
      const error = appError(value, "Browser audio could not be unlocked. Use a trusted pointer/keyboard gesture.");
      this.report(error);
      throw error;
    }
  }

  audioVolume(channel: AudioChannel, value: number): void {
    this.#hooks.volume(channel, value);
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
    this.#generation++;
    this.#stopPoll();
    if (this.#reconnect !== undefined) clearTimeout(this.#reconnect);
    this.#transport.dispose();
    this.#listeners.clear();
    await this.#tail;
    this.#bridge.free();
  }
}
