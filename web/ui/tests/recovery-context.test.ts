import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { decodeUiCatalogue } from "../assets.ts";
import { recoveryFeeText } from "../recovery.ts";
import type { RecoveryDisplay } from "../recovery.ts";

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
    })),
  };
  assert.equal(recoveryFeeText(current), expected);
});
