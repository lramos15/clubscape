import os from "node:os";
import path from "node:path";
import { readFile, realpath } from "node:fs/promises";
import { parseArgs } from "node:util";
import { chromium } from "playwright-core";
import { contractHash, harnessRoot, loadConfig, parseConfig, repositoryRoot, requireCondition, verifySourcePack } from "./config.ts";
import { collectHostFacts, defaultExecutableCandidates, discoverExecutable, hostFingerprint } from "./platform.ts";
import { validateInventory, prepareOwnerConfiguration, inventorySchema } from "./owner-data.ts";
import { bundleRuns, readJsonWithReference, removeBundledRuns, writeOwnerJson } from "./owner-files.ts";
import { startFixture } from "./fixture-server.ts";
import { runHarness } from "./capture.ts";

const usage = "pnpm run handoff discover|prepare|bundle|cleanup [options]; pnpm owner:probe --inventory file --browser chrome|edge --run-id id";

async function main() {
  requireCondition(await realpath(process.cwd()) === await realpath(harnessRoot), "Run from tools/browser-harness");
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: {
      out: { type: "string" }, chrome: { type: "string" }, edge: { type: "string" },
      inventory: { type: "string" }, browser: { type: "string" },
      profile: { type: "string" }, "run-id": { type: "string" },
      "inspect-seconds": { type: "string" }, probe: { type: "string" },
      case: { type: "string" }, runs: { type: "string" }, manifest: { type: "string" },
    },
  });
  requireCondition(positionals.length === 1, usage);
  const operation = positionals[0];
  if (operation === "discover") {
    requireCondition(values.out, "Discovery requires --out owner-runs/inventory.json");
    const host = await collectHostFacts();
    const cached = process.platform === "linux" ? chromium.executablePath() : undefined;
    const browsers = await Promise.all((["chrome", "edge"] as const).map((product) => {
      const explicit = values[product] ?? process.env[`${product.toUpperCase()}_EXECUTABLE`];
      return discoverExecutable(product, explicit ? [explicit] : defaultExecutableCandidates(product, process.platform, os.homedir(), cached));
    }));
    const inventory = inventorySchema.parse({
      schemaVersion: 1, collectedAt: new Date().toISOString(), scope: "local-native-discovery-not-acceptance",
      host, hostFingerprint: hostFingerprint(host), browsers: { chrome: browsers[0], edge: browsers[1] },
      m1Acceptance: "not-evaluated", securityCertification: "not-performed",
    });
    const output = await writeOwnerJson(values.out, inventory);
    console.log(JSON.stringify({
      inventory: output, platform: host.platform, model: host.model, chip: host.chip,
      browsers: browsers.map((b) => ({ product: b.product, status: b.status, version: b.executable?.version ?? null })),
      nativeMacMeasurements: "not-performed", issues: host.issues,
    }));
    return;
  }
  if (operation === "probe") {
    requireCondition(values.inventory && (values.browser === "chrome" || values.browser === "edge"), "Probe requires --inventory and --browser chrome|edge");
    const inventory = validateInventory((await readJsonWithReference(values.inventory)).value);
    requireCondition(inventory.host.platform === process.platform && inventory.host.architecture === process.arch, "Probe must run on the discovered native platform, not a simulated Mac");
    const executable = inventory.browsers[values.browser].executable;
    requireCondition(executable, `${values.browser} is unavailable; discovery attempts explain what is missing. No automatic browser install.`);
    const base = await loadConfig(path.join(harnessRoot, "config/fixture.json"));
    const config = parseConfig({
      ...base,
      browser: {
        ...base.browser, product: values.browser, expectedVersion: executable.version,
        executablePath: executable.path, executableSha256: executable.sha256,
        platform: inventory.host.platform, architecture: inventory.host.architecture,
        hostFingerprint: inventory.hostFingerprint,
        graphicsProfile: values.profile ?? (process.platform === "darwin" ? "mac-metal-default" : "desktop-default"),
      },
    });
    const seconds = Number(values["inspect-seconds"] ?? "0");
    requireCondition(Number.isInteger(seconds) && seconds >= 0 && seconds <= 300, "Inspection must be 0..300 seconds");
    const controller = new AbortController();
    const interrupt = () => controller.abort(new Error("Owner probe interrupted; cleaning owned processes"));
    process.once("SIGINT", interrupt);
    process.once("SIGTERM", interrupt);
    const fixture = await startFixture(config);
    try {
      config.url = `${fixture.origin}/${new URL(config.url).search}`;
      config.allowedOrigins = [fixture.origin];
      const result = await runHarness(config, { runId: values["run-id"], inspectionMs: seconds * 1000, signal: controller.signal });
      console.log(JSON.stringify({
        status: result.report.status, scope: "tool-fixture-not-product", report: path.join(result.directory, "report.json"),
        securityCertification: "not-performed", failures: result.failures.map((s) => s.slice(0, 500)),
      }));
      process.exitCode = result.ok ? 0 : 1;
    } finally {
      await fixture.close();
      process.removeListener("SIGINT", interrupt);
      process.removeListener("SIGTERM", interrupt);
    }
    return;
  }
  if (operation === "prepare") {
    requireCondition(values.inventory && values.probe && values.case && values.out
      && (values.browser === "chrome" || values.browser === "edge"), "Prepare requires --inventory --probe --case --browser chrome|edge --out owner-runs/file.json");
    const [inventory, probe, productCase, ownerRecord] = await Promise.all([
      readJsonWithReference(values.inventory), readJsonWithReference(values.probe),
      readJsonWithReference(values.case),
      readFile(path.join(repositoryRoot, "milestones/m1-owner-followups.json"), "utf8").then(JSON.parse),
    ]);
    const config = prepareOwnerConfiguration(inventory.value, probe.value, productCase.value, values.browser, ownerRecord, {
      inventory: inventory.reference, probe: probe.reference,
    });
    await verifySourcePack(config);
    const output = await writeOwnerJson(values.out, config);
    console.log(JSON.stringify({
      config: output, benchmarkContractId: config.contract.id, benchmarkContractSha256: contractHash(config.contract),
      sourcePackApproval: "from supplied product case, NOT from the Mac hardware-direction record",
      measurement: "not-run", securityCertification: "not-performed",
    }));
    return;
  }
  if (operation === "bundle") {
    requireCondition(values.runs && values.out, "Bundle requires --runs comma-separated-run-ids --out owner-runs/bundle-name");
    console.log(JSON.stringify(await bundleRuns(values.runs.split(","), values.out)));
    return;
  }
  if (operation === "cleanup") {
    requireCondition(values.manifest, "Cleanup requires --manifest owner-runs/bundle-name/manifest.json");
    console.log(JSON.stringify(await removeBundledRuns(values.manifest)));
    return;
  }
  throw new Error(usage);
}

main().catch((error) => { console.error(`Owner handoff failed: ${error.message}`); process.exitCode = 1; });
