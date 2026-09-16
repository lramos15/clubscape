import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { resolve } from "node:path";
import { sourceByte, sourceLines, textAdvance } from "../raster.ts";
import { frameRegions, nativeTree, tabWidget, TABS } from "../layout.ts";
import { readQuantity, isInterfaceUnlocked } from "../index.ts";
import { skillTooltip } from "../world-view.ts";
import { fixtureWorld, immutable } from "./component-fixture.ts";
import type { UiCatalogue } from "../assets.ts";
import { UiAssets, decodeUiCatalogue } from "../assets.ts";
import type { ClientAssets } from "../../shared/contracts.ts";
import { abilityVisible, filterOptionEnabled, spellGrid } from "../filters.ts";
import { recoveryFeeText, projectRecovery, recoveryControls, recoveryTemplate } from "../recovery.ts";
import type { RecoveryDisplay, RecoveryUiCommand } from "../recovery.ts";
import { entryErrorLines } from "../entry.ts";
import { projectProduction } from "../production.ts";
import { projectDeathPreview } from "../death-preview.ts";
import { projectQuestReward } from "../rewards.ts";
import type { ItemView } from "../../shared/contracts.ts";

const root = resolve(import.meta.dirname, "../../..");
const catalogue: UiCatalogue = decodeUiCatalogue(JSON.parse(readFileSync(resolve(root, "assets/compiled/ui/manifest.json"), "utf8")));
const pack = JSON.parse(readFileSync(resolve(root, "research/reference-pack/v1/manifest.json"), "utf8"));
const oracles = JSON.parse(readFileSync(resolve(root, "research/reference-pack/v1/dynamic-text-oracles.json"), "utf8"));

test("exact owner approval and source-font metrics are bound", () => {
  const approval = JSON.parse(readFileSync(resolve(root, "milestones/approvals/m1-reference-pack-v1.3.0.json"), "utf8"));
  assert.equal(approval.decision, "approved");
  assert.equal(catalogue.sourcePackSha256, approval.reference_pack_sha256);
  for (const id of [494, 495, 496, 497]) {
    const metrics = JSON.parse(readFileSync(resolve(root, `assets/source/osrs/cache2695/fonts/${id}.json`), "utf8"));
    assert.equal(catalogue.fonts[id]!.ascent, metrics.ascent);
    assert.deepEqual(catalogue.fonts[id]!.advances, metrics.advances);
  }
});

test("lossless source-widget pool rejects corrupt references", () => {
  const widget = catalogue.templates["native-inventory"]![0]!;
  const encoded = { ...catalogue, templateEncoding: "native-widget-pool-v1", widgetPool: [widget], templates: { first: [0], second: [0] } };
  const decoded = decodeUiCatalogue(encoded);
  assert.deepEqual(decoded.templates.first, [widget]);
  assert.deepEqual(decoded.templates.second, [widget]);
  assert.throws(() => decodeUiCatalogue({ ...encoded, templates: { invalid: [1] } }), /Invalid source widget reference/);
});

test("native CP1252 mapping, escapes, line breaks and indivisible tokens", () => {
  assert.equal(sourceByte("€"), 128); assert.equal(sourceByte("’"), 146); assert.equal(sourceByte("\u00a0"), 32);
  assert.equal(sourceByte("\u0080"), 63); assert.equal(sourceByte("🐧"), 63);
  assert.equal(textAdvance("<col=ff0000>A</col><lt>", catalogue.fonts[495]!), textAdvance("A<lt>", catalogue.fonts[495]!));
  assert.deepEqual(sourceLines("one<br>two", 300, catalogue.fonts[495]!), ["one", "two"]);
  assert.deepEqual(sourceLines("unbreakable", 1, catalogue.fonts[495]!), ["unbreakable"]);
  assert.deepEqual(sourceLines("one two three", textAdvance("one two", catalogue.fonts[495]!), catalogue.fonts[495]!), ["one two", "three"]);
});

test("696 pinned text records match metrics; two explicitly unrecorded texts remain unknown", () => {
  assert.equal(oracles.records.length, 698);
  let unrecorded = 0;
  for (const oracle of oracles.records) {
    if (oracle.metrics_by_font === null) {
      assert.equal(oracle.kind, "source_text_unrecorded");
      assert.equal(oracle.desktop_text, null);
      unrecorded++;
      continue;
    }
    for (const id of [494, 495, 496, 497]) {
      const metrics = oracle.metrics_by_font[String(id)];
      const lines = oracle.desktop_text.split("\n");
      assert.equal(lines.length, metrics.logical_line_advances_px.length, oracle.id);
      for (let i = 0; i < lines.length; i++) {
        const advances = [...lines[i]].map(character => catalogue.fonts[id]!.advances[sourceByte(character)]);
        assert.deepEqual(advances, metrics.codepoint_advances_px[i], `${oracle.id} / ${id}`);
      }
    }
  }
  assert.equal(unrecorded, 2);
});

test("71-state/29-family/11-signature coverage is retained without declaring unknown controls hidden", () => {
  assert.equal(catalogue.tutorialStates.length, 71);
  assert.equal(catalogue.hudSignatures.length, 11);
  assert.equal(pack.evidence_factorization.families.length, 29);
  assert.deepEqual(catalogue.tutorialStates.map(s => s.state_id), pack.evidence_factorization.state_bindings.map((s: { state_id: string }) => s.state_id));
  for (const state of catalogue.tutorialStates) assert.ok(catalogue.hudSignatures.some(s => s.id === state.hud_signature_id));
});

test("native anchors remain source-aligned across the approved resize range", () => {
  for (const [width, height] of [[1920, 1080], [1024, 768], [2560, 1440]] as const) {
    const regions = frameRegions(width, height), tree = nativeTree(catalogue.templates["native-inventory"]!, width, height);
    assert.deepEqual(regions.minimap, { x: width - 211, y: 0, width: 211, height: 207 });
    assert.deepEqual(regions.chat, { x: 0, y: height - 165, width: 519, height: 165 });
    assert.deepEqual(regions.sidebar, { x: width - 241, y: height - 335, width: 241, height: 335 });
    const inventory = tree.find(w => w.id >> 16 === 149 && w.index === 0)!;
    assert.equal(inventory.x, width - 200); assert.equal(inventory.y, height - 290);
    for (let i = 0; i < 14; i++) assert.ok(tree.some(w => w.id === tabWidget(i)));
  }
});

test("genuine source lock uses authoritative unlocks, never inferred negative evidence", () => {
  const world = fixtureWorld();
  world.player.tutorialStage = "stage.tutorial.inventory_open";
  world.player.unlockedInterfaces = ["interface.inventory"];
  immutable(world);
  assert.equal(isInterfaceUnlocked(world, "interface.inventory"), true);
  assert.equal(isInterfaceUnlocked(world, "interface.combat"), false);
  assert.equal(TABS.length, 14);
});

test("quantity parser rejects zero, negative, fractional, exponent, overflow and blank", () => {
  for (const input of ["", " ", "0", "-1", "1.5", "1e4", "Infinity", "9007199254740992", "1foo"]) assert.equal(readQuantity(input), null);
  assert.equal(readQuantity(" 12345 "), 12345);
});

test("skill XP formatting never loses u64/string precision or creates XP", () => {
  const skill = immutable({ id: "skill.mining", name: "Mining", baseLevel: 99, currentLevel: 99,
    xpTenths: "90071992547409931", iconAsset: null });
  assert.match(skillTooltip(skill, undefined), /9,007,199,254,740,993\.1/);
  assert.equal(skill.xpTenths, "90071992547409931");
});

test("content-derived weapon IDs and experience choices remain data-only source bindings", () => {
  const content = JSON.parse(gunzipSync(readFileSync(resolve(root, "content/m1/game-content.json.gz"))).toString());
  assert.deepEqual(catalogue.presentation!.weapons["1277"], content.items["item.sword.bronze"].equipment.weapon.styles);
  assert.deepEqual(catalogue.presentation!.experiences.map(c => c.id).sort(), Object.keys(content.mechanics.experiences).sort());
});

test("failed asset requests require explicit retry and release references on dispose", async () => {
  let attempts = 0;
  const errors: string[] = [];
  const client: ClientAssets = {
    baseUrl: "/assets/", url: id => "/assets/" + id,
    json: async () => catalogue,
    image: async id => {
      if (id === "component-test/lazy" && ++attempts === 1) throw new Error("Exact asset failure");
      return { naturalWidth: 1, naturalHeight: 1 } as HTMLImageElement;
    },
  };
  const assets = await UiAssets.load(client, error => errors.push(error.message));
  assert.equal(assets.image("component-test/lazy"), null);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(errors, ["Exact asset failure"]);
  assert.equal(assets.image("component-test/lazy"), null);
  assert.equal(attempts, 1);
  await assets.retryFailed();
  assert.ok(assets.image("component-test/lazy"));
  assert.equal(attempts, 2);
  assets.dispose();
  assert.equal(assets.images.size, 0);
});

test("source filter predicates retain unknown requirements and use exact Classic grid arithmetic", () => {
  const wind = catalogue.abilities[String(218 * 65536 + 11)]!;
  assert.equal(abilityVisible("magic", 32, wind, { requirements: null }), true);
  assert.equal(abilityVisible("magic", 32, wind, { requirements: false }), false);
  assert.equal(abilityVisible("magic", 16, wind, { resources: false }), false);
  assert.equal(abilityVisible("magic", 1, wind), false);
  assert.equal(filterOptionEnabled("prayer", 1, 0), false);
  assert.equal(filterOptionEnabled("prayer", 1, 1), true);
  assert.deepEqual(spellGrid(4, 184, 240, true), { size: 40, columns: 3, rows: 2, gapX: 28, gapY: 28, width: 176, height: 108 });
  assert.deepEqual(spellGrid(69, 184, 240, true), { size: 24, columns: 7, rows: 10, gapX: 2, gapY: 0, width: 180, height: 240 });
});

test("web-only error pages preserve long identifiers without overlap or native glyph clipping", () => {
  const message = "asset/" + "a".repeat(400) + "<missing>";
  const lines = entryErrorLines(message, 332, catalogue.fonts[495]!);
  assert.ok(lines.length > 3);
  assert.ok(lines.every(line => textAdvance(line, catalogue.fonts[495]!) <= 332));
  assert.equal(lines.join("").replaceAll("<lt>", "<").replaceAll("<gt>", ">"), message);
});

test("recovery display uses explicit fee/coffer inputs and native controls without granting items", () => {
  const item = fixtureWorld().player.inventory[0]!.item!;
  const view: RecoveryDisplay = {
    storage: "death_office", items: [{ id: "recovery-source-1", slot: 0, item: { ...item, quantity: 7 }, allowed: true, reason: null }],
    selectedId: "recovery-source-1", coffer: "12345", unitFee: 42, capacity: 120, bankAll: false, discardAll: false, scroll: 0,
  };
  immutable(view);
  assert.match(recoveryFeeText(view), /294/);
  assert.match(recoveryFeeText(view), /12,345/);
  const missing = { ...view, unitFee: null, coffer: null };
  assert.match(recoveryFeeText(missing), /Fee: unavailable/);
  assert.doesNotMatch(recoveryFeeText(missing), /12,345|294|>0</);
  const widgets = projectRecovery(recoveryTemplate(catalogue, view), view);
  const calls: RecoveryUiCommand[] = [];
  const controls = recoveryControls(widgets, 1920, 1080, view, command => calls.push(command));
  controls.find(control => control.label === "Retrieve 5")!.actions[0]!.run();
  assert.deepEqual(calls, [{ kind: "retrieve", id: "recovery-source-1", amount: 5 }]);
  assert.equal(view.items[0]!.item.quantity, 7);
  const locked = { ...view, items: [{ ...view.items[0]!, allowed: false, reason: "Authoritative source rejection" }] };
  const lockedControls = recoveryControls(projectRecovery(recoveryTemplate(catalogue, locked), locked),
    1920, 1080, locked, command => calls.push(command));
  assert.equal(lockedControls.find(control => control.id === "recovery-item-recovery-source-1")!.disabled, undefined,
    "Local fee inspection does not retrieve an item.");
  assert.equal(lockedControls.find(control => control.id === "recovery-item-recovery-source-1")!.tooltip, "Authoritative source rejection");
  assert.equal(lockedControls.find(control => control.label === "Retrieve 5")!.disabled, "Authoritative source rejection");
  const current = { ...view, unitFee: null, items: [{ ...view.items[0]!,
    unitFee: "9007199254740993", fullStackFee: "9007199254741001", inventoryCapacity: 2, bankCapacity: 4 }] };
  assert.match(recoveryFeeText(current), /9,007,199,254,740,993/);
  assert.match(recoveryFeeText(current), /9,007,199,254,741,001/);
  assert.doesNotMatch(recoveryFeeText(current), /63,050,394,783,186,951/,
    "The supplied full-entry fee is not reconstructed by multiplying the unit quote.");
});

test("source production model replacement preserves widget identity and native choice coordinates", () => {
  const yes = { allowed: true, code: null, reason: null };
  for (const count of [1, 2, 3, 6, 10, 18]) {
    const source = catalogue.templates[`native-production-choice-${count}`]!;
    const outputs = source.filter(widget => widget.id >> 16 === 270 && widget.type === 6 && widget.item >= 0);
    const view = {
      id: "component-menu", interface: "interface.cooking", target: { kind: "spawn" as const, spawn: "component-facility" },
      recipes: outputs.map((widget, index) => ({
        recipe: `opaque-recipe-${index}`, name: catalogue.items[widget.item]!.name,
        outputs: [{ id: `component-item-${widget.item}`, sourceId: widget.item, name: catalogue.items[widget.item]!.name,
          quantity: 1, instanceId: null, charges: null, iconAsset: null, actions: [] }], single: yes, makeX: yes,
      })),
    };
    immutable(view);
    const projection = projectProduction(catalogue, view, 1, 0, null);
    assert.deepEqual(projection.problems, []);
    const icons = projection.widgets.filter(widget => widget.id >> 16 === 270 && widget.type === 6);
    assert.equal(icons.length, count);
    assert.equal(new Set(icons.map(widget => widget.id)).size, count);
    for (const icon of icons) {
      const parent = projection.widgets.find(widget => widget.id === icon.id && widget.index === -1)!;
      assert.equal(icon.x, parent.x + Math.trunc((parent.width - icon.width) / 2));
      assert.equal(icon.y, parent.y + Math.trunc((parent.height - icon.height) / 2));
      assert.equal(icon.item, outputs.find(widget => widget.id === icon.id)!.item);
    }
  }
});

test("bank placeholder artwork uses the actual original placeholder definition, never an alpha-tinted normal item", () => {
  assert.equal(catalogue.items[1265]!.placeholderId, 14760);
  assert.equal(catalogue.items[1351]!.placeholderId, 14705);
  for (const id of [1265, 1351, 995]) {
    const placeholder = catalogue.items[catalogue.items[id]!.placeholderId]!;
    assert.ok(placeholder.placeholderTemplateId >= 0);
    assert.ok(placeholder.icons.length > 0);
  }
});

test("death preview projects only supplied rows, opaque quantities and full monetary text", () => {
  const supplied = { ...fixtureWorld().player.inventory[0]!.item!, quantity: 37 };
  const view = immutable({ scope: "normal_unsafe_non_pvp" as const, kept: [], lost: [supplied],
    fullGraveFee: "9007199254740993", fullOfficeFee: "18446744073709551615", valueRevision: "9007199254740995" });
  const projection = projectDeathPreview(catalogue, view, 0);
  assert.equal(projection.items.size, 1);
  const row = [...projection.items.values()][0]!;
  assert.equal(row.item.quantity, 37);
  assert.equal(row.kept, false);
  assert.equal(projection.widgets.filter(widget => widget.id >> 16 === 4 && widget.item >= 0).length, 1);
});

test("quest reward lines use the real source 9..15 slots and never retain native fixture awards", () => {
  const source = catalogue.templates["native-reward"]!;
  const output: ItemView = { ...fixtureWorld().player.inventory[2]!.item!, quantity: 7 };
  const reward = immutable({
    id: "reward-id", kind: "quest" as const, interface: "interface.quest_reward", title: "Authoritative title",
    lines: ["Actual authoritative line"], items: [output], xp: [{ skill: "skill.cooking", amountTenths: "9007199254740993" }],
    questPoints: 1, quest: null, skill: null, level: null,
    continuation: { kind: "ui_dismiss" as const, presentation_id: "reward-id" },
  });
  const widgets = projectQuestReward(source, catalogue, reward, fixtureWorld().player.skills, 19, 0);
  const text = (child: number) => widgets.find(widget => widget.id === 153 * 65536 + child)!.text;
  assert.equal(text(4), "Authoritative title");
  assert.equal(text(6), "Total Quest Points: 19");
  assert.equal(text(9), "Actual authoritative line");
  assert.equal(text(10), "7 x Shrimps");
  assert.ok(widgets.some(widget => widget.text.includes("900,719,925,474,099.3")));
  assert.ok(!widgets.some(widget => /^Line [1-7]$/.test(widget.text)));
});
