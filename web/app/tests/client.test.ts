import assert from "node:assert/strict";
import { test } from "node:test";
import { BrowserApp, bridgeState } from "../client.ts";
import type { BridgeState, WasmClient, ClientHooks } from "../client.ts";
import { RpcTransport } from "../transport.ts";
import type { Fetch } from "../transport.ts";
import type { PublicWorld } from "../public-state.ts";
import { AppError } from "../errors.ts";
import type { GameplayUiIntent } from "../../shared/contracts.ts";

// These doubles isolate composition/order/privacy, not protocol or gameplay correctness.
class FixtureBridge implements WasmClient {
  stateValue: BridgeState = {
    version: 1, phase: "account_ready", authenticated: true, accountName: "fixture",
    characterInitialized: true, gameplayAvailable: true, unavailableReason: null,
    capabilities: ["game.v1"], gameplayUiWireSupported: false,
    serverBuild: "fixture-not-game", contentRevision: "fixture", contentManifestPath: "/content/manifest.json",
    world: null, events: [], nextSequence: "1", uncertainInput: false,
    worldJoined: false, quote: null, quoteError: null,
  };
  intents: unknown[] = [];
  operations: string[] = [];
  inputs: Array<{ operation: string; input: string }> = [];
  lastOperation = "";
  freed = false;
  pending = false;
  selected: Array<{ input: string; itemId: string }> = [];
  prepare(_request: string, operation: string, input: string): Uint8Array {
    this.inputs.push({ operation, input });
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
  submit_selected(request: string, input: string, itemId: string): Uint8Array {
    this.selected.push({ input, itemId });
    return this.submit(request, JSON.stringify({ ...JSON.parse(input), expected_item: itemId }));
  }
  receive(): string {
    this.pending = false;
    if (this.lastOperation === "join") {
      this.stateValue.phase = "in_world";
      this.stateValue.worldJoined = true;
    }
    if (this.lastOperation === "create_character") this.stateValue.characterInitialized = true;
    if (this.lastOperation === "intent" && (this.intents.at(-1) as { kind?: string } | undefined)?.kind === "confirm_appearance"
      && this.stateValue.world) this.stateValue.world.player.appearanceConfirmed = true;
    if (this.lastOperation === "leave") this.stateValue.worldJoined = false;
    if (this.lastOperation === "logout") {
      this.stateValue.authenticated = false; this.stateValue.accountName = null; this.stateValue.world = null;
      this.stateValue.worldJoined = false;
    }
    return this.state();
  }
  request_id(): string { return "00000000-0000-4000-8000-000000000001"; }
  request_is_shop_buy(): boolean { return (this.intents.at(-1) as { kind?: string } | undefined)?.kind === "shop_buy"; }
  receive_for(): string { return this.receive(); }
  state(): string { return JSON.stringify(this.stateValue); }
  authorization(): string | undefined { return this.stateValue.authenticated ? "not-a-real-token" : undefined; }
  transport_lost(): void { this.stateValue.phase = "reconnecting"; this.pending = false; }
  retry_uncertain_input(): Uint8Array | undefined { return undefined; }
  retry_lifecycle(): Uint8Array | undefined { return undefined; }
  set_catalog(): string {
    this.stateValue.world = {
      revision: "9007199254740993", tick: "9007199254740993",
      player: { region: "region.fixture", instance: null, tile: { x: 1, y: 1, plane: 0 }, skills: [],
        presence: { kind: "connected", connected: true, acceptsInput: true, presentInWorld: true }, appearanceConfirmed: false },
      recoveryContext: null,
    } as unknown as PublicWorld;
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

test("actual UI appearance submission keeps source creation empty and confirms only after joining", async () => {
  const bridge = new FixtureBridge();
  bridge.stateValue.characterInitialized = false;
  const app = new BrowserApp(bridge, new RpcTransport((async () =>
    new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooks());
  const appearance = { body_type: 0 };
  const created = app.createCharacter(appearance);
  appearance.body_type = 99;
  await created;
  assert.equal(bridge.inputs.find((request) => request.operation === "create_character")?.input, "{}");
  assert(bridge.operations.indexOf("create_character") < bridge.operations.indexOf("join"));
  assert.deepEqual(bridge.intents, [{ kind: "confirm_appearance", appearance: { body_type: 0 } }]);
  assert.equal((app.state().world as PublicWorld).player.appearanceConfirmed, true);
  assert.equal(app.state().phase, "world");
  await assert.rejects(app.createCharacter({ body_type: 0 }), /already confirmed/);
  assert.equal(bridge.operations.filter((operation) => operation === "create_character").length, 1);
  await app.dispose();
});

test("published gameplay UI requests preserve their selection but send no legacy substitute or optimistic state", async () => {
  const bridge = new FixtureBridge();
  let requests = 0;
  const app = new BrowserApp(bridge, new RpcTransport((async () => {
    requests++;
    return new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } });
  }) as Fetch), hooks());
  await app.enterWorld();
  assert.equal(app.gameplayUi().available, false);
  assert.equal(app.gameplayUi().reason, "not_advertised");
  assert.equal(app.state().world?.ui, undefined);
  assert.match(app.state().error?.message ?? "", /game.ui.v1/);
  const baseline = requests;
  let seen: unknown;
  bridge.submit = (_id, input) => {
    seen = JSON.parse(input);
    throw new AppError("game.ui.v1 is not advertised; no wire request was sent.", { kind: "unsupported_capability" });
  };
  const intent: Extract<GameplayUiIntent, { kind: "item_action" }> = { kind: "item_action", inventory_slot: 1, expected_item: "item.original",
    expected_instance: "item_instance.original", action: "action.original" };
  const pending = app.send(intent);
  intent.expected_item = "item.replacement";
  intent.expected_instance = null;
  await assert.rejects(pending, /no wire request/);
  assert.deepEqual(seen, { kind: "item_action", inventory_slot: 1, expected_item: "item.original",
    expected_instance: "item_instance.original", action: "action.original" });
  assert.equal(requests, baseline);
  assert.equal(app.state().phase, "world");
  assert.equal(bridge.stateValue.nextSequence, "1");
  assert.equal(Object.hasOwn(app.state().world!, "ui"), false);
  await app.dispose();
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

test("shop purchase selection identity reaches WASM unchanged and never becomes an index-only write", async () => {
  const bridge = new FixtureBridge();
  let requests = 0;
  const app = new BrowserApp(bridge, new RpcTransport((async () => {
    requests++;
    return new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } });
  }) as Fetch), hooks());
  await app.enterWorld();
  const baseline = requests;
  await assert.rejects(app.send({ kind: "shop_buy", shop: "shop.fixture", item_index: 0, quantity: 1 }), /expected_item/);
  const selected = { kind: "shop_buy" as const, shop: "shop.fixture", item_index: 0, quantity: 1, expected_item: "item.fixture.tool" };
  const pending = app.send(selected);
  selected.expected_item = "item.fixture.other";
  await pending;
  assert.deepEqual(bridge.intents, [{ kind: "shop_buy", shop: "shop.fixture", item_index: 0, quantity: 1, expected_item: "item.fixture.tool" }]);
  assert.equal(bridge.selected.length, 0, "The current shell sends the canonical field directly.");
  assert.equal(requests, baseline + 1);
  await app.dispose();
});

test("rejected buys and quotes refresh real view data, preserve the error ID and never retarget queued input", async () => {
  for (const rejectedOperation of ["intent", "quote"]) {
    const bridge = new FixtureBridge();
    const originalReceive = bridge.receive.bind(bridge);
    const errorId = "00000000-0000-4000-8000-000000000060";
    const rejection = JSON.stringify({
      kind: "server", message: "The authoritative source query requirements are not satisfied.",
      code: 3, errorId, recoverable: true,
    });
    bridge.receive = () => {
      if (bridge.lastOperation === rejectedOperation) {
        bridge.pending = false;
        throw rejection;
      }
      if (bridge.lastOperation === "poll" && bridge.stateValue.world) {
        bridge.stateValue.world.shop = {
          id: "shop.fixture", name: "Fixture shop", interfaceId: null, currency: "item.coins",
          rows: [{ index: 0, itemId: "item.replacement", stock: 2, buyPrice: 7, sellPrice: 1,
            item: { id: "item.replacement", name: "Replacement", quantity: 2, sourceId: 23,
              iconAsset: null, instanceId: null, charges: null, actions: [] } }],
        };
      }
      return originalReceive();
    };
    const app = new BrowserApp(bridge, new RpcTransport((async () =>
      new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooks());
    await app.enterWorld();
    if (rejectedOperation === "intent") {
      const buy = { kind: "shop_buy" as const, shop: "shop.fixture", item_index: 0, quantity: 1, expected_item: "item.original" };
      const results = await Promise.allSettled([app.send(buy), app.send(buy)]);
      assert(results.every((result) => result.status === "rejected"));
      assert.equal(bridge.intents.length, 1);
      assert.deepEqual(bridge.intents[0], buy);
    } else {
      await assert.rejects(app.quote({
        kind: "shop_buy", shop: "shop.fixture", itemIndex: 0, quantity: 1, expected_item: "item.original",
      }), /choose the current item again/);
      assert.equal(bridge.intents.length, 0);
    }
    assert.equal(bridge.operations.at(-1), "poll");
    assert.equal(app.state().world?.shop?.rows[0]?.item.id, "item.replacement");
    assert.equal(app.state().error?.errorId, errorId);
    assert.match(app.state().error?.message ?? "", /refreshed/);
    assert.equal(bridge.stateValue.nextSequence, "1");
    await app.dispose();
  }
});

test("lost logout acknowledgements reconcile via real account logout without rejoining the body", async () => {
  const bridge = new FixtureBridge();
  let fail = false;
  const app = new BrowserApp(bridge, new RpcTransport((async () => {
    if (fail) throw new Error("fixture transport lost");
    return new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } });
  }) as Fetch), hooks());
  await app.enterWorld();
  const joins = bridge.operations.filter((operation) => operation === "join").length;
  fail = true;
  await assert.rejects(app.send({ kind: "request_logout" }), /interrupted/);
  fail = false;
  await app.reconnect();
  assert.equal(bridge.operations.filter((operation) => operation === "join").length, joins);
  assert.equal(bridge.operations.at(-1), "logout");
  assert.equal(app.state().phase, "title");
  assert.equal(bridge.authorization(), undefined);
  await app.dispose();
});

test("an observed source-offline body is not automatically rejoined", async () => {
  const bridge = new FixtureBridge();
  const originalReceive = bridge.receive.bind(bridge);
  bridge.receive = () => {
    const result = originalReceive();
    if (bridge.lastOperation === "intent" && bridge.stateValue.world) {
      bridge.stateValue.world.player.presence = {
        kind: "offline", connected: false, acceptsInput: false, presentInWorld: false,
      };
      return bridge.state();
    }
    return result;
  };
  const app = new BrowserApp(bridge, new RpcTransport((async () =>
    new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooks());
  await app.enterWorld();
  await app.send({ kind: "cancel_activity" });
  assert.equal(app.state().phase, "error");
  assert.equal(app.state().world, null);
  const joins = bridge.operations.filter((operation) => operation === "join").length;
  await app.reconnect();
  assert.equal(bridge.operations.filter((operation) => operation === "join").length, joins);
  await app.dispose();
});

test("read-only quotes keep exact recovery fees and reject stale selections after reconciling the view", async () => {
  const bridge = new FixtureBridge();
  const app = new BrowserApp(bridge, new RpcTransport((async () =>
    new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooks());
  await app.enterWorld();
  bridge.stateValue.quote = {
    kind: "recovery", revision: "9007199254740993", tick: "9007199254740993",
    death: "death.fixture", storage: "grave", selected: ["recovery_item.fixture"], fullSelectionFee: "18446744073709551615",
  };
  const request = { kind: "recovery" as const, death: "death.fixture", storage: "grave" as const, items: ["recovery_item.fixture"] };
  const quote = await app.quote(request);
  assert.equal(quote.kind, "recovery");
  assert(quote.kind === "recovery" && quote.fullSelectionFee === "18446744073709551615");
  assert(Object.isFrozen(quote));
  assert.equal(bridge.intents.length, 0);
  bridge.stateValue.quote = null;
  bridge.stateValue.quoteError = "Source selection changed.";
  await assert.rejects(app.quote(request), /selection changed/);
  assert.equal(app.state().phase, "world");
  await app.dispose();
});

test("uncovered authoritative regions fail explicitly instead of borrowing an arbitrary fixture scene", async () => {
  const bridge = new FixtureBridge();
  const hooksValue = hooks();
  hooksValue.prepareWorld = async () => { throw new AppError("Actual source region is not exported.", { kind: "region_unavailable" }); };
  const app = new BrowserApp(bridge, new RpcTransport((async () =>
    new Response(new Uint8Array([1]), { headers: { "content-type": "application/x-protobuf" } })) as Fetch), hooksValue);
  await assert.rejects(app.enterWorld(), /region is not exported/);
  assert.equal(app.state().phase, "error");
  assert.equal(app.state().world, null);
  assert.equal(bridge.stateValue.worldJoined, true, "A presentation gap does not fabricate a source logout.");
  assert(!bridge.operations.includes("leave"));
  await app.dispose();
});
