import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { APPROVAL_PATH, PACK_PATH, verifyAuthority, verifyFrozenContext } from "../reference.ts";

const root = new URL("../../../", import.meta.url);

test("external exact approval governs despite preserved historical context status", async () => {
  const [approval, pack] = await Promise.all([readFile(new URL(APPROVAL_PATH, root)), readFile(new URL(PACK_PATH, root))]);
  const manifest = verifyAuthority(approval, pack);
  const context = await verifyFrozenContext(manifest, (path) => readFile(new URL(path, root)));
  assert.deepEqual(context.map((record) => [record.path, record.sha256]), [
    ["spec/art-style.md", "79a1527765c34612e035d67de0718165c98b4b4c54c056ac7c051ceb3e95c259"],
    ["spec/interface-parity.md", "77cfea36b5fd3c17f15e388c65a33d99bf0e462e898fb407aea93b1ec5ae41be"],
  ]);
  assert.throws(() => verifyAuthority(Buffer.concat([approval, Buffer.from("\n")]), pack), /approval bytes changed/);
  assert.throws(() => verifyAuthority(approval, Buffer.concat([pack, Buffer.from("\n")])), /reference-pack bytes changed/);
});

test("a status-prose edit to frozen inputs fails without modifying source files in the test", async () => {
  const manifest = verifyAuthority(await readFile(new URL(APPROVAL_PATH, root)), await readFile(new URL(PACK_PATH, root)));
  await assert.rejects(verifyFrozenContext(manifest, async (path) => {
    const bytes = await readFile(new URL(path, root));
    return path === "spec/art-style.md" ? Buffer.concat([bytes, Buffer.from("\nUpdated approval status.\n")]) : bytes;
  }), /Frozen pre-approval context changed: spec\/art-style.md/);
});
