import assert from "node:assert/strict";
import { test } from "node:test";
import { AssetLoader } from "../assets.ts";
import { AppError } from "../errors.ts";
import { sha256, publicPath } from "../identity.ts";
import { parseContentManifest } from "../manifest.ts";
import type { ContentManifest } from "../manifest.ts";
import { SOURCE_PACK_SHA256 } from "../../shared/contracts.ts";
import { RpcTransport, boundedBytes } from "../transport.ts";
import type { Fetch } from "../transport.ts";
import { Settings, PREFERENCE_KEY } from "../settings.ts";
import { checkCapability } from "../capability.ts";

export async function fixtureManifest(bytes = new TextEncoder().encode('{"fixture":"not gameplay"}')): Promise<ContentManifest> {
  return parseContentManifest({
    schemaVersion: 1, sourcePackSha256: SOURCE_PACK_SHA256, contentRevision: "test-fixture-only", artifactSha256: "a".repeat(64),
    catalog: { contentRevision: "test-fixture-only", items: {}, skills: {}, entities: {}, quests: {}, equipmentSlots: [] },
    assets: [{ id: "asset.fixture", url: "/assets/fixture.json", sha256: await sha256(bytes), bytes: bytes.length, contentType: "application/json" }],
    bootstrap: ["asset.fixture"], rendererManifest: null, regions: {},
  });
}

test("same-origin binary transport never uses credentials in URL, redirects, cookies, or logs", async () => {
  const input = new Uint8Array([1, 2, 3]);
  let recorded: RequestInit | undefined;
  const transport = new RpcTransport((async (path, options) => {
    assert.equal(path, "/v1/rpc");
    recorded = options;
    assert.equal(options?.method, "POST");
    assert.equal(options?.mode, "same-origin");
    assert.equal(options?.redirect, "error");
    assert.equal(options?.credentials, "omit");
    assert.equal(options?.referrerPolicy, "no-referrer");
    assert.equal(new Headers(options?.headers).get("authorization"), "Bearer memory-only");
    return new Response(new Uint8Array([4, 5]), { status: 401, headers: { "content-type": "application/x-protobuf" } });
  }) as Fetch);
  assert.deepEqual(await transport.post(input, "memory-only"), new Uint8Array([4, 5]));
  assert.deepEqual(input, new Uint8Array([1, 2, 3]));
  assert.deepEqual(recorded?.body, new Uint8Array([0, 0, 0]));
  transport.dispose();
});

test("RPC malformed MIME, oversized streams, and interruption fail explicitly", async () => {
  const transport = new RpcTransport((async () => new Response("<html>error</html>", { headers: { "content-type": "text/html" } })) as Fetch);
  await assert.rejects(transport.post(new Uint8Array([1]), undefined), /binary Protobuf/);
  await assert.rejects(boundedBytes(new Response(new Uint8Array([1, 2, 3])), 2), /byte budget/);
  const interrupted = new RpcTransport((async () => { throw new Error("Never echo a credential-bearing low-level request error."); }) as Fetch);
  await assert.rejects(interrupted.post(new Uint8Array([1]), undefined), (error: unknown) =>
    error instanceof AppError && error.kind === "transport" && !error.message.includes("credential-bearing"));
  transport.dispose(); interrupted.dispose();
});

test("asset readiness requires verified bytes and a completed real decoder call", async () => {
  let requests = 0;
  const manifest = await fixtureManifest();
  const loader = new AssetLoader(manifest, "b".repeat(64), { fetch: (async () => {
    requests++;
    return new Response('{"fixture":"not gameplay"}', { headers: { "content-type": "application/json" } });
  }) as Fetch });
  assert.equal(loader.observe().length, 0);
  assert.equal(loader.url("asset.fixture"), "/assets/fixture.json");
  assert.equal(loader.observe().length, 0, "merely requesting a URL is not loading evidence");
  await loader.bytes("asset.fixture");
  assert.deepEqual(loader.counts(["asset.fixture"]), { fetched: 1, decoded: 0, total: 1 });
  assert.deepEqual(await loader.json("asset.fixture"), { fixture: "not gameplay" });
  assert.deepEqual(loader.counts(["asset.fixture"]), { fetched: 1, decoded: 1, total: 1 });
  assert.equal(requests, 1);
  assert(Object.isFrozen(await loader.json("asset.fixture")));
  loader.dispose();
});

test("asset 404, wrong bytes and invalid JSON never mark decoding complete", async () => {
  const good = await fixtureManifest();
  for (const response of [
    new Response("missing", { status: 404, headers: { "content-type": "application/json" } }),
    new Response('{"wrong":true}', { headers: { "content-type": "application/json" } }),
  ]) {
    const loader = new AssetLoader(good, "b".repeat(64), { fetch: (async () => response) as Fetch });
    await assert.rejects(loader.json("asset.fixture"));
    assert.equal(loader.observe()[0]?.decoded, false);
    loader.dispose();
  }
  const bad = new TextEncoder().encode("not json");
  const invalid = new AssetLoader(await fixtureManifest(bad), "b".repeat(64), {
    fetch: (async () => new Response(bad, { headers: { "content-type": "application/json" } })) as Fetch,
  });
  await assert.rejects(invalid.json("asset.fixture"), /could not be decoded/);
  assert.equal(invalid.observe()[0]?.fetched, true);
  assert.equal(invalid.observe()[0]?.decoded, false);
  invalid.dispose();
});

test("untrusted asset paths and duplicate/foreign source manifest identities are refused", async () => {
  for (const path of ["//elsewhere.example/a", "/assets/../secret", "/assets/%61", "/assets/file?secret=x", "/v1/rpc", "/assets/.private"]) {
    assert.throws(() => publicPath(path));
  }
  const manifest = await fixtureManifest();
  assert.throws(() => parseContentManifest({ ...manifest, sourcePackSha256: "0".repeat(64) }));
  assert.throws(() => parseContentManifest({ ...manifest, assets: [...manifest.assets, manifest.assets[0]] }));
  assert.throws(() => parseContentManifest({
    ...manifest, catalog: { ...manifest.catalog, inventoryActions: { "item.not_in_source_catalog": ["Drop"] } },
  }));
  assert.throws(() => parseContentManifest({ ...manifest, aliases: { "../private": "asset.fixture" } }));
  assert.throws(() => parseContentManifest({ ...manifest, aliases: { "research/source.json": "asset.unknown" } }));
});

test("only explicitly declared source path aliases resolve to verified canonical assets", async () => {
  const manifest = parseContentManifest({ ...await fixtureManifest(), aliases: { "research/audio-source/source-map.json": "asset.fixture" } });
  let requests = 0;
  const assets = new AssetLoader(manifest, "b".repeat(64), { fetch: (async (url) => {
    assert.equal(url, "/assets/fixture.json"); requests++;
    return new Response('{"fixture":"not gameplay"}', { headers: { "content-type": "application/json" } });
  }) as Fetch });
  assert.equal(assets.url("research/audio-source/source-map.json"), "/assets/fixture.json");
  await assets.json("research/audio-source/source-map.json");
  await assets.json("asset.fixture");
  assert.equal(requests, 1);
  assert.equal(assets.observe()[0]?.id, "asset.fixture");
  assert.throws(() => assets.url("research/private.json"));
  assets.dispose();
});

test("preferences persist only a bounded allowlist, never arbitrary input fields", async () => {
  const writes = new Map<string, string>();
  const settings = new Settings({
    getItem: () => JSON.stringify({ schemaVersion: 1, profile: "source-resizable-classic-v1", audio: { music: 0.3, effects: 99 }, password: "not copied", token: "not copied" }),
    setItem: (key, value) => { writes.set(key, value); },
  });
  assert.equal(settings.read().audio.music, 0.3);
  assert.equal(settings.read().audio.effects, 1);
  assert.equal(settings.volume("effects", -7), 0);
  const written = writes.get(PREFERENCE_KEY)!;
  assert(!written.includes("password") && !written.includes("token") && !written.includes("not copied"));
  assert.match(await settings.hash({ source: "test-fixture-only" }), /^[0-9a-f]{64}$/);
});

test("capability failure never selects a software/WebGL fallback", async () => {
  await assert.rejects(checkCapability({ secure: false, gpu: undefined, wasm: WebAssembly }), /HTTPS/);
  await assert.rejects(checkCapability({ secure: true, gpu: undefined, wasm: WebAssembly }), /WebGPU/);
  await assert.rejects(checkCapability({ secure: true, gpu: { requestAdapter: async () => null }, wasm: WebAssembly }), /hardware WebGPU/);
  let requested: GPURequestAdapterOptions | undefined;
  await assert.rejects(checkCapability({
    secure: true, wasm: WebAssembly,
    gpu: { requestAdapter: async (options) => {
      requested = options;
      return { info: { isFallbackAdapter: true } } as GPUAdapter;
    } },
  }), /hardware WebGPU/);
  assert.equal(requested?.forceFallbackAdapter, false);
});
