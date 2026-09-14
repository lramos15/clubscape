import { parseArgs } from "node:util";
import { loadConfig, contractHash } from "./config.ts";
import { runHarness } from "./capture.ts";
import { startFixture } from "./fixture-server.ts";

async function main() {
  const { values } = parseArgs({
    options: {
      config: { type: "string" },
      "run-id": { type: "string" },
      "print-contract-hash": { type: "boolean" },
      "tool-fixture": { type: "boolean" },
    },
  });
  if (!values.config) throw new Error("Usage: pnpm capture --config config/fixture.json [--run-id id]");
  const config = await loadConfig(values.config);
  if (values["print-contract-hash"]) {
    console.log(`${config.contract.id} sha256:${contractHash(config.contract)}`);
    return;
  }
  const controller = new AbortController();
  let fixture: Awaited<ReturnType<typeof startFixture>> | undefined;
  const interrupt = () => controller.abort(new Error("Capture interrupted; cleaning owned processes"));
  process.once("SIGINT", interrupt);
  process.once("SIGTERM", interrupt);
  try {
    if (values["tool-fixture"]) {
      fixture = await startFixture(config);
      config.url = `${fixture.origin}/${new URL(config.url).search}`;
      config.allowedOrigins = [fixture.origin];
    }
    const result = await runHarness(config, { runId: values["run-id"], signal: controller.signal });
    console.log(JSON.stringify({
      status: result.report.status, purpose: config.purpose, m1Acceptance: "not-evaluated",
      report: `${result.directory}/report.json`,
      failures: result.failures.map((message) => message.slice(0, 500)),
    }));
    process.exitCode = result.ok ? 0 : 1;
  } finally {
    await fixture?.close();
    process.removeListener("SIGINT", interrupt);
    process.removeListener("SIGTERM", interrupt);
  }
}

main().catch((error) => { console.error(`Capture failed: ${error.message}`); process.exitCode = 1; });
