import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { bindBankRevision, checkUiIntent, gameplayUiProblem, requiresBankRevision } from "../gameplay-ui.ts";
import { decodeUiCatalogue } from "../assets.ts";
import { newcomerMapMarker, projectDocument } from "../documents.ts";
import { minimapSurfaceProblem } from "../minimap-surface.ts";
import type { UiMinimapSurface } from "../minimap-surface.ts";
import { fixtureUi, fixtureWorld, immutable } from "./component-fixture.ts";
import type { GameplayUiIntent, GameplayUiView } from "../../shared/contracts.ts";

const catalogue = decodeUiCatalogue(JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../assets/compiled/ui/manifest.json"), "utf8")));
function world() {
  const value = fixtureWorld();
  return { ...value, ui: fixtureUi(value) };
}
function bankWorld() {
  const value = world(), item = value.player.inventory[0]!.item!;
  value.ui.bank = {
    revision: "9007199254740993", capacity: 400, selectedTab: 0, insertMode: false, placeholders: false, amount: 1, noted: false,
    entries: [{ id: "bank-entry", slot: 0, tab: 0, item: item.id, value: item, placeholder: false }],
    tabs: [{ tab: 0, firstEntry: "bank-entry", entries: 1 }], depositEquipment: { allowed: true, code: null, reason: null },
    unavailableContainers: [],
  };
  return value;
}
const document = (): NonNullable<GameplayUiView["document"]> => ({
  id: "document.opaque", interface: "interface.read_book", title: "Actual book title",
  pages: ["First supplied page", "Second supplied page"], page: 0, nativeMap: false, mapAsset: null,
});

test("UI4 requires activeTab and document fields while explicit closed/null state remains valid", () => {
  const value = world();
  value.ui.activeTab = null;
  assert.equal(gameplayUiProblem(value), null);
  Reflect.deleteProperty(value.ui, "document");
  assert.match(gameplayUiProblem(value)!.message, /ui.document/);
  value.ui.document = null;
  Reflect.deleteProperty(value.ui, "activeTab");
  assert.match(gameplayUiProblem(value)!.message, /ui.activeTab/);
});

test("document requests retain identity, page bounds and native-map empty-page semantics", () => {
  const value = world(); value.ui.document = document();
  assert.equal(checkUiIntent(value, { kind: "ui_document_page", document_id: "document.opaque", page: 1 }), null);
  assert.equal(checkUiIntent(value, { kind: "ui_document_page", document_id: "stale", page: 1 })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(value, { kind: "ui_document_page", document_id: "document.opaque", page: 2 })?.code, "ui.document.page.invalid");
  assert.equal(checkUiIntent(value, { kind: "ui_dismiss", presentation_id: "document.opaque" }), null);
  value.ui.document = { ...document(), interface: "interface.newcomer_map", nativeMap: true, pages: [] };
  assert.equal(gameplayUiProblem(value), null);
  assert.equal(checkUiIntent(value, { kind: "ui_document_page", document_id: "document.opaque", page: 0 })?.code, "ui.document.page.invalid");
  value.ui.document.nativeMap = false;
  assert.equal(gameplayUiProblem(value)?.code, "ui.projection.invalid");
});

test("every published bank operation binds the decimal bank revision without substituting a world tick", () => {
  const value = bankWorld(), revision = value.ui.bank!.revision;
  const requests: GameplayUiIntent[] = [
    { kind: "bank_placeholder", entry_id: "bank-entry" }, { kind: "bank_select_tab", tab: 0 },
    { kind: "bank_create_tab", entry_id: "bank-entry" }, { kind: "bank_move", entry_id: "bank-entry", before_entry_id: null, tab: 0 },
    { kind: "bank_collapse_tab", tab: 0 }, { kind: "bank_set_insert", enabled: true },
    { kind: "bank_set_placeholders", enabled: true }, { kind: "bank_release_placeholder", entry_id: "bank-entry" },
    { kind: "bank_deposit_equipment" }, { kind: "bank_withdraw_entry", entry_id: "bank-entry", quantity: 1, noted: false },
    { kind: "bank_set_options", amount: 5, noted: true },
  ];
  for (const intent of requests) {
    assert.equal(requiresBankRevision(intent), true);
    assert.equal(bindBankRevision(intent, revision).expected_bank_revision, revision);
    assert.equal(intent.expected_bank_revision, undefined);
  }
  const request = bindBankRevision(requests[0]!, revision);
  value.revision = "18446744073709551615"; value.tick = "18446744073709551614";
  assert.equal(checkUiIntent(value, request), null);
  assert.equal(checkUiIntent(value, requests[0]!)?.code, "ui.bank.revision.required");
  value.ui.bank!.revision = "9007199254740994";
  assert.equal(bindBankRevision(request, value.ui.bank!.revision), request);
  assert.equal(checkUiIntent(value, request)?.code, "ui.bank.revision.stale");
  const chat: GameplayUiIntent = { kind: "public_chat", channel: "public", text: "hello" };
  assert.equal(bindBankRevision(chat, revision), chat);
});

test("placeholder requests never mutate ownership or turn placeholder metadata into a spendable item", () => {
  const value = bankWorld(), request = bindBankRevision({ kind: "bank_placeholder", entry_id: "bank-entry" }, value.ui.bank!.revision);
  const original = structuredClone(value);
  assert.equal(checkUiIntent(immutable(value), request), null);
  assert.deepEqual(value, original);
  const placeholder = bankWorld();
  placeholder.ui.bank!.entries[0]!.placeholder = true; placeholder.ui.bank!.entries[0]!.value = null;
  assert.equal(checkUiIntent(placeholder, request)?.code, "ui.identity.stale");
});

test("book text preserves source colours, native line slots and reachable overflow without editing authoritative pages", () => {
  const value = document();
  value.pages = ["<col=7f0000>" + "Source colour ".repeat(120) + "</col>", "Final page"];
  const original = structuredClone(value);
  const first = projectDocument(catalogue, immutable(value), 0, false, null);
  assert.ok(first.parts > 1); assert.equal(first.previous, false); assert.equal(first.next, true);
  const second = projectDocument(catalogue, value, 1, false, null);
  assert.equal(second.previous, true);
  assert.ok(second.widgets.some(widget => (widget.id >>> 16) === 392 && widget.text.startsWith("<col=7f0000>")));
  assert.deepEqual(value, original);
});

test("newcomer-map source2043 geometry preserves underground normalization and genuine outside-map hiding", () => {
  assert.deepEqual(newcomerMapMarker({ x: 2912, y: 3136, plane: 0 }), { x: 75, y: 235 });
  assert.deepEqual(newcomerMapMarker({ x: 3296, y: 3520, plane: 0 }), { x: 402, y: 97 });
  assert.deepEqual(newcomerMapMarker({ x: 2912, y: 9536, plane: 0 }), { x: 75, y: 235 });
  assert.equal(newcomerMapMarker({ x: 2911, y: 3136, plane: 0 }), null);
  const value = { ...document(), interface: "interface.newcomer_map", nativeMap: true, pages: [] };
  const hidden = projectDocument(catalogue, value, 0, false, null);
  const shown = projectDocument(catalogue, value, 0, true, { x: 3222, y: 3218, plane: 0 });
  assert.ok(hidden.widgets.some(widget => widget.text === "Show Tutors"));
  assert.ok(shown.widgets.some(widget => widget.text === "Hide Tutors"));
  assert.ok(shown.widgets.some(widget => widget.model === 3062));
});

function surface(): UiMinimapSurface {
  const data = new Uint8ClampedArray(512 * 512 * 4);
  for (let index = 3; index < data.length; index += 4) data[index] = 255;
  return {
    width: 512, height: 512, scale: 4, marginX: 48, marginY: 48, baseX: 3168, baseY: 3168, plane: 0,
    revision: 1, complete: false, notes: ["Explicit component input"],
    stats: { terrainTiles: 0, wallMarks: 0, diagonalMarks: 0, mapScenes: 0, unresolved: 1 },
    icons: [], pixels: { width: 512, height: 512, data, colorSpace: "srgb" }, mask: new Uint8Array(512 * 512),
  };
}

test("renderer surfaces retain incomplete qualifications and reject malformed original pixel/mask formats", () => {
  const value = surface();
  assert.equal(minimapSurfaceProblem(value), null);
  assert.match(minimapSurfaceProblem({ ...value, scale: 2 })!, /scale4/);
  assert.match(minimapSurfaceProblem({ ...value, revision: Number.MAX_SAFE_INTEGER + 1 })!, /revision/);
  assert.match(minimapSurfaceProblem({ ...value, mask: new Uint8Array(1) })!, /mask/);
  value.mask[0] = 2;
  assert.match(minimapSurfaceProblem(value)!, /only 0 or 1/);
  value.mask[0] = 0; value.pixels.data[3] = 0;
  assert.match(minimapSurfaceProblem(value)!, /opaque/);
});
