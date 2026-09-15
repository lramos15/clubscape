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

const root = resolve(import.meta.dirname, "../../..");
const catalogue: UiCatalogue = JSON.parse(readFileSync(resolve(root, "assets/compiled/ui/manifest.json"), "utf8"));
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
