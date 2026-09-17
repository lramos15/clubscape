import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { decodeUiCatalogue } from "../assets.ts";
import { projectRecovery, recoveryContextItems, recoveryControls, recoveryFeeText, recoveryTemplate } from "../recovery.ts";
import type { RecoveryDisplay } from "../recovery.ts";
import { bindBankRevision, checkUiIntent, gameplayUiProblem, isGameplayUiIntent, requiresBankRevision } from "../gameplay-ui.ts";
import { MAX_RECOVERY_SELECTION_ENTRIES, recoverySelectionProblem, sameRecoveryContext, sameRecoverySelection } from "../recovery-context.ts";
import { recoveryContextFixture } from "./recovery-fixture.ts";
import { immutable } from "./component-fixture.ts";
import type { GameplayUiIntent, RecoveryContextSelection, RecoveryContextView } from "../../shared/contracts.ts";

const catalogue = decodeUiCatalogue(JSON.parse(readFileSync(
  new URL("../../../assets/compiled/ui/manifest.json", import.meta.url), "utf8",
)));

function originalRecoveryFixture(): { view: RecoveryDisplay; expected: string } {
  const source = catalogue.templates["native-retrieval-scroll-669"];
  assert(source);
  const info = source.find(widget => widget.id === (669 << 16 | 11) && widget.index === -1);
  assert(info);
  const items = source.filter(widget => widget.id === (669 << 16 | 3) && widget.index >= 0 && widget.item >= 0)
    .map(widget => {
      const definition = catalogue.items[widget.item];
      assert(definition);
      return {
        id: `entry.${widget.index}`, slot: widget.index, allowed: true, reason: null,
        item: {
          id: `item.source.${widget.item}`, sourceId: widget.item, name: definition.name,
          quantity: widget.item_quantity, actions: [], iconAsset: null, instanceId: null, charges: null,
        },
      };
    });
  assert.equal(items.length, 80);
  return {
    view: {
      storage: "death_office", items, selectedId: "entry.7", coffer: "12345",
      unitFee: 42, capacity: null, bankAll: false, discardAll: false, scroll: 60,
    },
    expected: info.text,
  };
}

test("original recovery readback counts the selected item type across duplicate slots", () => {
  const { view, expected } = originalRecoveryFixture();
  assert.equal(view.items.find(row => row.id === view.selectedId)?.item.quantity, 7);
  assert.match(expected, /^35 x Bronze arrow:/);
  assert.equal(recoveryFeeText(view), expected);
});

test("current entry quotes preserve the original selected-type recovery caption", () => {
  const { view, expected } = originalRecoveryFixture();
  const current: RecoveryDisplay = {
    ...view, unitFee: null,
    items: view.items.map(row => ({
      ...row, unitFee: "42", fullStackFee: String(42 * row.item.quantity),
      selectedTypeCaption: row.item.sourceId === 882
        ? { kind: "source", sourceId: 882, quantity: "35", unitFee: "42", totalFee: "1470" }
        : { kind: "unavailable", reason: "This controlled source regression selects only the bronze arrows." },
    })),
  };
  assert.equal(recoveryFeeText(current), expected);
  assert.equal(current.items[7]!.fullStackFee, "294");
  const projected = projectRecovery(recoveryTemplate(catalogue, current), { ...current, capacity: 120, entryCount: 80 });
  const title = projected.find(widget => widget.id === (669 << 16 | 1) && widget.index === 1)!;
  assert.equal(new Set(current.items.map(row => row.item.sourceId)).size, 16);
  assert.equal(title.text, "Death's Office Item Retrieval <col=ffb83f>(80/120)</col>");
});

test("native display captions remain separate from per-entry and combined executable fees", () => {
  const { context } = recoveryContextFixture();
  const display: RecoveryDisplay = {
    storage: "death_office", items: recoveryContextItems(context), selectedId: "entry.two", coffer: "12345",
    unitFee: null, capacity: context.counts.capacity, entryCount: context.counts.entries,
    bankAll: false, discardAll: false, scroll: 0, fullSelectionFee: context.takeAll.plan!.totalFee,
  };
  immutable(display);
  assert.equal(display.items[1]!.slot, 1);
  assert.equal(display.items[1]!.item.quantity, 7);
  assert.equal(display.items[1]!.fullStackFee, "308");
  assert.equal(display.fullSelectionFee, "602");
  assert.equal(recoveryFeeText(display),
    "14 x Bronze arrow:<br>Fee: <col=ffffff>44 coins</col> each (<col=ffffff>616</col>)<br>Death's Coffer: <col=ffffff>12,345 coins</col>");
});

test("missing or unavailable native captions never turn entry quotes into source type totals", () => {
  const { view } = originalRecoveryFixture();
  const items = view.items.map(row => ({ ...row, unitFee: "42", fullStackFee: "294" }));
  const missing: RecoveryDisplay = { ...view, items, unitFee: null };
  assert.match(recoveryFeeText(missing), /Source fee display unavailable/);
  assert.doesNotMatch(recoveryFeeText(missing), /294|1,470/);
  const unavailable: RecoveryDisplay = {
    ...missing, scroll: 0, items: items.map(row => ({
      ...row, selectedTypeCaption: { kind: "unavailable", reason: "The original item identity is unavailable." },
    })),
  };
  assert.match(recoveryFeeText(unavailable), /Source fee display unavailable/);
  const controls = recoveryControls(projectRecovery(recoveryTemplate(catalogue, unavailable), unavailable),
    1920, 1080, unavailable, () => assert.fail("Projection must not dispatch."));
  assert.equal(controls.find(control => control.id === "recovery-item-entry.7")!.tooltip,
    "The original item identity is unavailable.");
});

test("empty Office is a present context without a legacy death panel or invented items", () => {
  const { world, context } = recoveryContextFixture(true);
  assert.equal(world.recovery, null);
  assert.equal(gameplayUiProblem(world), null);
  assert.deepEqual(recoveryContextItems(context), []);
  const display: RecoveryDisplay = {
    storage: "death_office", items: [], selectedId: null, coffer: "0", unitFee: null,
    capacity: context.counts.capacity, entryCount: context.counts.entries, scroll: 0,
    bankAll: false, discardAll: false, takeAll: false, takeAllReason: context.takeAll.permission.reason!,
  };
  const widgets = projectRecovery(recoveryTemplate(catalogue, display), display);
  assert.equal(widgets.filter(widget => widget.id === (669 << 16 | 3) && widget.index >= 0).length, 0);
  assert.match(widgets.find(widget => widget.id === (669 << 16 | 1) && widget.index === 1)!.text, /\(0\/120\)/);
  const controls = recoveryControls(widgets, 1920, 1080, display, () => assert.fail("No items may be fabricated."));
  assert.equal(controls.find(control => control.label === "Take-All")!.disabled, context.takeAll.permission.reason);
  assert.equal(checkUiIntent(world, { kind: "recovery_take_all", selection: context.takeAll.selection })?.code, "InvalidTarget");
});

test("whole-context Take-All echoes one exact immutable observation without a bank precondition", () => {
  const { world, context } = recoveryContextFixture();
  const before = JSON.stringify(world);
  const request: GameplayUiIntent = { kind: "recovery_take_all", selection: structuredClone(context.takeAll.selection) };
  immutable(world); immutable(request);
  assert.equal(gameplayUiProblem(world), null);
  assert(isGameplayUiIntent(request));
  assert.equal(requiresBankRevision(request), false);
  assert.equal(bindBankRevision(request, "1"), request);
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(JSON.stringify(world), before);
  const equivalent: RecoveryContextSelection = {
    records: structuredClone(context.takeAll.selection.records),
    context: { instance: null, interface: "interface.death_retrieval", kind: "death_office" },
  };
  assert(sameRecoverySelection(request.selection, equivalent));
  assert.equal(checkUiIntent(world, { ...request, selection: equivalent }), null);
  assert(!sameRecoveryContext(equivalent.context, { kind: "death_office", interface: "interface.death_retrieval", instance: "another-office" }));
});

test("held Take-All selections cannot be rebound after quantity, physical storage, order or context changes", () => {
  const { world, context } = recoveryContextFixture();
  const changes: Array<(selection: RecoveryContextSelection) => void> = [
    selection => { selection.records[0]!.entries[0]!.quantity = 6; },
    selection => { selection.records[1]!.entries[0]!.current_storage = "death_office"; },
    selection => { selection.records.reverse(); },
    selection => { selection.records.pop(); },
    selection => { selection.records[0]!.death = "another-death"; },
    selection => { selection.context = { kind: "death_office", interface: "interface.death_retrieval", instance: "another-office" }; },
    selection => { selection.context.interface = "another-interface"; },
  ];
  for (const change of changes) {
    const selected = structuredClone(context.takeAll.selection);
    change(selected);
    assert.equal(checkUiIntent(world, { kind: "recovery_take_all", selection: selected })?.code, "ui.identity.stale");
  }
  delete world.ui.recovery!.management!.context;
  assert.equal(checkUiIntent(world, { kind: "recovery_take_all", selection: context.takeAll.selection })?.code, "ui.identity.stale");
});

test("partial recovery uses the selected record and Office request context, not physical grave storage", () => {
  const { world, context } = recoveryContextFixture();
  const selected = context.slots[1]!;
  assert.equal(selected.currentStorage, "grave");
  const request: GameplayUiIntent = {
    kind: "recovery_take", death: selected.death, storage: context.identity.kind,
    items: [{ id: selected.entry.id, amount: { kind: "quantity", quantity: 5 } }],
  };
  assert.equal(checkUiIntent(world, request), null);
  assert.equal(checkUiIntent(world, { ...request, storage: "grave" })?.code, "ui.identity.stale");
  assert.equal(checkUiIntent(world, { ...request, death: "death.one" })?.code, "ui.identity.stale");
});

test("recovery observation bounds are global across distinct nonempty records", () => {
  const { world, context } = recoveryContextFixture();
  const selection: RecoveryContextSelection = {
    context: context.identity,
    records: [0, 1].map(index => ({
      death: `bounded-${index}`, entries: Array.from({ length: MAX_RECOVERY_SELECTION_ENTRIES / 2 }, (_, offset) => ({
        id: `bounded-${index}-${offset}`, quantity: 1, current_storage: "death_office",
      })),
    })),
  };
  assert.equal(recoverySelectionProblem(selection), null);
  selection.records[1]!.entries.push({ id: "one-over", quantity: 1, current_storage: "death_office" });
  assert.equal(checkUiIntent(world, { kind: "recovery_take_all", selection })?.code, "ui.request.invalid");
  for (const change of [
    (value: RecoveryContextSelection) => { value.records[1]!.entries = []; },
    (value: RecoveryContextSelection) => { value.records[1]!.entries[0]!.id = value.records[0]!.entries[0]!.id; },
    (value: RecoveryContextSelection) => { value.records[1]!.death = value.records[0]!.death; },
    (value: RecoveryContextSelection) => { value.context = { kind: "grave", interface: "interface.grave", death: "death.one" }; },
  ]) {
    const value = structuredClone(context.takeAll.selection); change(value);
    assert.equal(checkUiIntent(world, { kind: "recovery_take_all", selection: value })?.code, "ui.request.invalid");
  }
});

test("inconsistent native context identities, slots, counts, captions and plans surface projection errors", () => {
  const changes: Array<(context: RecoveryContextView) => void> = [
    context => { context.identity.interface = "another-interface"; },
    context => { context.slots[1]!.slot = 0; },
    context => { context.slots[1]!.entry.id = context.slots[0]!.entry.id; },
    context => { context.counts.entries = 1; },
    context => { context.counts.nativeItemTypes = 3; },
    context => { context.counts.capacityUnit = "entries"; },
    context => { context.counts.stored = 121; },
    context => { context.takeAll.selection.records[0]!.entries[0]!.quantity = 6; },
    context => { context.takeAll.selection.records[1]!.entries = []; },
    context => { context.takeAll.plan = null; },
    context => { context.takeAll.plan!.transfers = []; },
    context => { context.takeAll.plan!.transfers[0]!.death = "another-death"; },
    context => { context.takeAll.plan!.transfers[0]!.quantity = 8; },
    context => { context.slots[0]!.entry.inventoryCapacity = 8; },
    context => { context.slots[0]!.selectedTypeCaption = { kind: "source", sourceId: 995, quantity: "14", unitFee: "42", totalFee: "588" }; },
    context => { Reflect.set(context, "version", 2); },
    context => { Reflect.deleteProperty(context.slots[0]!, "currentStorage"); },
    context => { Reflect.set(context.counts, "capacity", 0); },
  ];
  for (const change of changes) {
    const { world, context } = recoveryContextFixture(); change(context);
    assert.equal(gameplayUiProblem(world)?.code, "ui.projection.invalid", change.toString());
  }
  const { world } = recoveryContextFixture();
  immutable(world.ui);
  assert.equal(gameplayUiProblem(world), null);
  world.player.instance = "a-different-instance";
  assert.equal(gameplayUiProblem(world)?.code, "ui.recovery.context.invalid",
    "A cached immutable projection still checks the current actor instance.");
});
