import assert from "node:assert/strict";
import { test } from "node:test";
import { BrowserApp, bridgeState } from "../client.ts";
import type { BridgeState, WasmClient, ClientHooks } from "../client.ts";
import { RpcTransport } from "../transport.ts";
import type { Fetch } from "../transport.ts";
import type { WorldView } from "../../shared/contracts.ts";
import { AppError } from "../errors.ts";

// These doubles isolate composition/order/privacy, not protocol or gameplay correctness.
class FixtureBridge implements WasmClient {
  stateValue: BridgeState = {
    version: 1, phase: "account_ready", authenticated: true, accountName: "fixture",
    characterInitialized: true, gameplayAvailable: true, unavailableReason: null,
    serverBuild: "fixture-not-game", contentRevision: "fixture", contentManifestPath: "/content/manifest.json",
    world: null, events: [], nextSequence: "1", uncertainInput: false,
  };
  intents: unknown[] = [];
  operations: string[] = [];
  lastOperation = "";
  freed = false;
  pending = false;
  prepare(_request: string, operation: string): Uint8Array {
    this.operations.push(operation); this.lastOperation = operation;
    return new Uint8Array([1]);
  }
  submit(_request: string, input: string): Uint8Array {
    assert.equal(this.pending, false);
    this.pending = true;
    this.lastOperation = "intent";
    this.intents.push(JSON.parse(input));
    return new Uint8Array([1]);
  }
  receive(): string {
    this.pending = false;
    if (this.lastOperation === "join") this.stateValue.phase = "in_world";
    if (this.lastOperation === "logout") {
      this.stateValue.authenticated = false; this.stateValue.accountName = null; this.stateValue.world = null;
    }
    return this.state();
  }
  request_id(): string { return "00000000-0000-4000-8000-000000000001"; }
  receive_for(): string { return this.receive(); }
  state(): string { return JSON.stringify(this.stateValue); }
  authorization(): string | undefined { return this.stateValue.authenticated ? "not-a-real-token" : undefined; }
  transport_lost(): void { this.stateValue.phase = "reconnecting"; this.pending = false; }
  retry_uncertain_input(): Uint8Array | undefined { return undefined; }
  set_catalog(): string {
    this.stateValue.world = {
      revision: "9007199254740993", tick: "9007199254740993",
      player: { region: "region.fixture", instance: null, tile: { x: 1, y: 1, plane: 0 }, skills: [] },
    } as unknown as WorldView;
    return this.state();
  }
  free(): void { this.freed = true; }
}
function hooks(): ClientHooks {
  return {
    content: async () => ({ contentRevision: "fixture", items: {}, skills: {}, entities: {}, quests: {}, equipmentSlots: [] }),
    prepareWorld: async () => {},
    events: () => {}, unlockAudio: async () => {}, volume: () => {}, disconnected: () => {},
  };
}

test("composition serializes one authoritative intent at a time and snapshots mutable UI inputs", async () => {
  const bridge = new FixtureBridge();
  let inFlight = 0;
  let maxInFlight = 0;
  const transport = new RpcTransport((async () => {
    inFlight++; maxInFlight = Math.max(inFlight, maxInFlight);
    await new Promise((resolve) => setTimeout(resolve, 1));
    inFlight--;
    return new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } });
  }) as Fetch);
  const app = new BrowserApp(bridge, transport, hooks());
  await app.enterWorld();
  const first = { kind: "walk" as const, destination: { x: 2, y: 1, plane: 0 }, running: false };
  const sent = app.send(first);
  first.destination.x = 999;
  const second = app.send({ kind: "cancel_activity" });
  await Promise.all([sent, second]);
  assert.equal(maxInFlight, 1);
  assert.deepEqual(bridge.intents, [
    { kind: "walk", destination: { x: 2, y: 1, plane: 0 }, running: false }, { kind: "cancel_activity" },
  ]);
  assert(Object.isFrozen(app.state()));
  assert(Object.isFrozen(app.state().world?.player));
  assert.equal(app.state().world?.revision, "9007199254740993");
  assert(!JSON.stringify(app.state()).includes("not-a-real-token"));
  await app.dispose();
  assert.equal(bridge.freed, true);
});

test("connection loss drops unsent actions instead of replaying stale UI intent", async () => {
  const bridge = new FixtureBridge();
  let fail = false;
  const transport = new RpcTransport((async () => {
    if (fail) throw new Error("interrupted fixture transport");
    return new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } });
  }) as Fetch);
  const app = new BrowserApp(bridge, transport, hooks());
  await app.enterWorld();
  fail = true;
  const results = await Promise.allSettled([app.send({ kind: "cancel_activity" }), app.send({ kind: "close_interface" })]);
  assert(results.every((result) => result.status === "rejected"));
  assert.equal(bridge.intents.length, 1, "unsent queued action was cancelled, not retried");
  assert.equal(app.state().phase, "reconnecting");
  assert.match(app.state().error?.message ?? "", /interrupted/);
  await app.dispose();
});

test("failed real logout cannot erase the world/token or pretend to sign out", async () => {
  const bridge = new FixtureBridge();
  const originalReceive = bridge.receive.bind(bridge);
  bridge.receive = () => {
    if (bridge.lastOperation === "leave") {
      throw JSON.stringify({ kind: "server", message: "Source combat logout lock.", errorId: "00000000-0000-4000-8000-000000000060", code: 3, recoverable: true });
    }
    return originalReceive();
  };
  const app = new BrowserApp(bridge, new RpcTransport((async () =>
    new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooks());
  await app.enterWorld();
  await assert.rejects(app.logout(), /combat logout lock/);
  assert.equal(bridge.authorization(), "not-a-real-token");
  assert.equal(app.state().phase, "world");
  assert(!bridge.operations.includes("logout"));
  await app.dispose();
});

test("UI reported errors retain only an explicit safe correlation and invalid numeric U64 is refused", () => {
  const invalid = new FixtureBridge().stateValue;
  invalid.nextSequence = 9007199254740992 as unknown as string;
  assert.throws(() => bridgeState(JSON.stringify(invalid)), AppError);
});

test("a terminal device/component failure cannot be cleared by a later poll", async () => {
  const bridge = new FixtureBridge();
  const app = new BrowserApp(bridge, new RpcTransport((async () =>
    new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooks());
  await app.enterWorld();
  app.report(new AppError("GPU device lost.", { kind: "device", recoverable: false }));
  await new Promise((resolve) => setTimeout(resolve, 650));
  assert.equal(app.state().phase, "error");
  assert(!bridge.operations.includes("poll"));
  await assert.rejects(app.send({ kind: "cancel_activity" }), /Reload/);
  await app.dispose();
});
