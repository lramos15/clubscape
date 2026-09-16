import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { decodeUiCatalogue } from "../../ui/assets.ts";
import { deepFreeze } from "../errors.ts";
import { captureUiBankRevision } from "../gameplay-ui.ts";
import type { GameplayUiIntent, RecoveryContextView, RecoveryManagementView, UiPermission } from "../../shared/contracts.ts";

const manifest = readFileSync(new URL("../../../assets/compiled/ui/manifest.json", import.meta.url));
const catalogue = decodeUiCatalogue(JSON.parse(manifest.toString("utf8")));

const allowed: UiPermission = { allowed: true, code: null, reason: null };
const unavailable: UiPermission = {
  allowed: false, code: "unavailable", reason: "This source Office has no Bank-All control.",
};

// Shared-contract fixtures only; these never become client-set balances or game records.
function context(): RecoveryContextView {
  const identity = {
    kind: "death_office", interface: "interface.fixture.office", instance: "instance.fixture.office",
  } as const;
  return {
    version: 1, identity,
    counts: { entries: 2, nativeItemTypes: 1, capacity: 120, capacityUnit: "item_types_or_instances", stored: 1, offered: 1 },
    slots: ["one", "two"].map((name, index) => ({
      slot: index, death: `death.fixture.${name}`, currentStorage: "death_office",
      entry: {
        id: `recovery_item.fixture.${name}`,
        item: { id: "item.fixture.arrow", name: "Bronze arrow", quantity: 7, sourceId: 882,
          iconAsset: "asset.fixture.arrow", instanceId: null, charges: null, actions: [] },
        unitFee: index === 0 ? "42" : "44", fullStackFee: index === 0 ? "294" : "308",
        inventoryCapacity: 7, bankCapacity: 0, take: allowed, bank: unavailable,
      },
      selectedTypeCaption: { kind: "source", sourceId: 882, quantity: "14",
        unitFee: index === 0 ? "42" : "44", totalFee: index === 0 ? "588" : "616" },
    })),
    takeAll: {
      permission: allowed,
      selection: {
        context: identity,
        records: ["one", "two"].map(name => ({
          death: `death.fixture.${name}`,
          entries: [{ id: `recovery_item.fixture.${name}`, quantity: 7, current_storage: "death_office" }],
        })),
      },
      plan: {
        totalFee: "602", partial: false,
        transfers: [
          { death: "death.fixture.one", id: "recovery_item.fixture.one", quantity: 7, fee: "294" },
          { death: "death.fixture.two", id: "recovery_item.fixture.two", quantity: 7, fee: "308" },
        ],
      },
    },
  };
}

test("retained Office scroll caption is selected-type35 while its native title counts80 occupied slots", () => {
  assert.equal(createHash("sha256").update(manifest).digest("hex"),
    "6601f1de45324ff23e88df577b3e467d74878f09b6f0f1759c52c6870452b329");
  const scroll = catalogue.templates["native-retrieval-scroll-669"];
  assert.ok(scroll);
  assert.equal(scroll.find(widget => widget.id === ((669 << 16) | 11))?.text,
    "35 x Bronze arrow:<br>Fee: <col=ffffff>42 coins</col> each (<col=ffffff>1,470</col>)<br>Death's Coffer: <col=ffffff>12,345 coins</col>");
  assert.equal(scroll.find(widget => widget.id === ((669 << 16) | 1) && widget.index === 1)?.text,
    "Death's Office Item Retrieval <col=ffb83f>(80/120)</col>");
  const short = catalogue.templates["native-retrieval-669-12345-7-42"];
  assert.ok(short);
  assert.equal(short.find(widget => widget.id === ((669 << 16) | 1) && widget.index === 1)?.text,
    "Death's Office Item Retrieval <col=ffb83f>(16/120)</col>");
});

test("whole-context selection is forwarded once and unchanged without inventing a bank precondition", () => {
  const original = deepFreeze(context());
  const request: GameplayUiIntent = { kind: "recovery_take_all", selection: original.takeAll.selection };
  const forwarded = captureUiBankRevision(request, null);
  assert.equal(forwarded, request);
  assert.equal(Object.hasOwn(forwarded, "expected_bank_revision"), false);
  assert.equal(original.slots[0]?.entry.item.quantity, 7);
  assert.equal(original.slots[0]?.selectedTypeCaption.kind, "source");
  assert.equal(original.takeAll.plan?.totalFee, "602");
  assert.equal(original.takeAll.selection.records.length, 2);
  assert.ok(Object.isFrozen(original.takeAll.selection.records[0]?.entries));
  assert.equal(JSON.stringify(forwarded), JSON.stringify(request));
});

test("existing Bank-All revision echoes remain exact and are never replaced with a context count or price", () => {
  const original = context();
  const request: GameplayUiIntent = {
    kind: "recovery_bank_all", expected_bank_revision: "9007199254743333",
    records: original.takeAll.selection.records.map(record => ({
      death: record.death, items: record.entries.map(entry => entry.id),
    })),
  };
  assert.deepEqual(captureUiBankRevision(request, null), request);
  assert.notEqual(request.expected_bank_revision, original.takeAll.plan?.totalFee);
});

test("legacy absence and a present empty Office context remain distinguishable", () => {
  const legacy: RecoveryManagementView = {
    bankRevision: "9007199254743333", panels: [], bankAll: unavailable, bankAllRecords: [],
  };
  assert.equal(legacy.context, undefined);
  const empty = context();
  empty.counts = { ...empty.counts, entries: 0, nativeItemTypes: 0, stored: 0, offered: 0 };
  empty.slots = [];
  empty.takeAll = {
    permission: { allowed: false, code: "not_owned", reason: "There are no recovery items to take." },
    selection: { context: empty.identity, records: [] }, plan: null,
  };
  const current: RecoveryManagementView = { ...legacy, context: empty };
  assert.equal(current.context?.identity.kind, "death_office");
  assert.equal(current.context?.counts.capacity, 120);
  assert.deepEqual(current.context?.slots, []);
  assert.equal(current.context?.takeAll.permission.allowed, false);
  assert.equal(Object.hasOwn(current.context?.identity ?? {}, "death"), false);
});
