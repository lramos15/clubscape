import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { createHash, randomUUID } from "node:crypto";
import { collectBuild, publicAssetPath, publicFile } from "../deliver.ts";

test("delivery is explicit, hash-pinned and rejects private extensions/traversal/symlinks", async () => {
  const root = fileURLToPath(new URL("../../../.local/web-delivery-checks/", import.meta.url));
  const directory = join(root, randomUUID());
  await mkdir(directory, { recursive: true });
  try {
    await writeFile(join(directory, "index.html"), "<p>infrastructure fixture, not game UI</p>");
    await writeFile(join(directory, "module.wasm"), new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
    const files = await collectBuild(directory);
    assert.deepEqual(files.map((file) => file.url).sort(), ["/", "/module.wasm"]);
    for (const file of files) {
      assert.equal(file.sha256, createHash("sha256").update(await readFile(join(directory, file.path))).digest("hex"));
    }
    await writeFile(join(directory, "private.rs"), "not public");
    await assert.rejects(collectBuild(directory), /nonpublic file extension/);
    await rm(join(directory, "private.rs"));
    await symlink("index.html", join(directory, "alias.html"));
    await assert.rejects(publicFile(directory, "alias.html", "/alias.html"), /symlink/);
    await assert.rejects(publicFile(directory, "../outside.html", "/outside.html"), /canonical same-origin/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("all packaging entrypoints use the same byte-preserving public gzip carrier", () => {
  assert.equal(publicAssetPath("/assets/compiled/render/blocks/12336.bin.gz"),
    "assets/compiled/render/blocks/12336.bin.gz.bin");
  assert.equal(publicAssetPath("/assets/ui/minimaps/dot-1.png"), "assets/ui/minimaps/dot-1.png");
  assert.throws(() => publicAssetPath("/assets/../private"));
  assert.throws(() => publicAssetPath("https://outside.example/asset.bin.gz"));
});
