import assert from "node:assert/strict";
import { test } from "node:test";
import { randomUUID } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { readArtifact } from "../artifact.ts";

test("artifact transport decodes bounded gzip without pretending it validates runtime policy", async () => {
  const directory = fileURLToPath(new URL(`../../../.local/artifact-transport-${randomUUID()}/`, import.meta.url));
  await mkdir(directory, { recursive: true });
  try {
    const bytes = Buffer.from("Transport fixture only; native compiler must still reject non-artifacts.");
    await writeFile(join(directory, "test.csc.gz"), gzipSync(bytes));
    assert.deepEqual(await readArtifact(join(directory, "test.csc.gz")), bytes);
    await writeFile(join(directory, "bad.csc.gz"), "invalid gzip");
    await assert.rejects(readArtifact(join(directory, "bad.csc.gz")), /gzip/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
