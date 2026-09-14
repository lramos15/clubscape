import { copyFile, lstat, mkdir, readFile, readdir, realpath, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { canonicalJson, harnessRoot, requireCondition, sha256 } from "./config.ts";

export async function outputPath(relative: string): Promise<string> {
  requireCondition(!path.isAbsolute(relative), "Owner output must be relative to tools/browser-harness");
  const parts = relative.split(/[\\/]/);
  requireCondition(parts[0] === "owner-runs" && parts.length >= 2
    && parts.every((part) => /^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(part)), "Owner output must be a simple path under owner-runs/");
  let parent = path.resolve(harnessRoot);
  for (const part of parts.slice(0, -1)) {
    parent = path.join(parent, part);
    await mkdir(parent, { recursive: true, mode: 0o700 });
    requireCondition(await realpath(parent) === parent, "Owner output parent must not be a symlink");
  }
  return path.join(parent, parts[parts.length - 1]);
}

export async function writeOwnerJson(relative: string, value: unknown): Promise<string> {
  const file = await outputPath(relative);
  await writeFile(file, JSON.stringify(value, null, 2) + "\n", { flag: "wx", mode: 0o600 });
  return file;
}

export async function readJsonWithReference(file: string) {
  const bytes = await readFile(file);
  return { value: JSON.parse(bytes.toString("utf8")) as unknown, reference: `${file} sha256:${sha256(bytes)}` };
}

async function runDirectory(id: string) {
  requireCondition(/^[a-z0-9][a-z0-9_-]{0,100}$/.test(id), "Run ID must be an identifier, not a path");
  const directory = path.resolve(harnessRoot, "runs", id);
  requireCondition(await realpath(directory) === directory, "Run path must not be a symlink");
  const reportFile = path.join(directory, "report.json");
  requireCondition((await lstat(reportFile)).isFile(), "Run report must be an ordinary file");
  const report = JSON.parse(await readFile(reportFile, "utf8"));
  requireCondition(report.schemaVersion === 1 && report.runId === id && report.m1Acceptance === "not-evaluated",
    "Directory is not a matching harness observation run");
  return { directory, report };
}

export async function bundleRuns(ids: string[], destination: string) {
  requireCondition(ids.length > 0 && ids.length <= 30 && new Set(ids).size === ids.length, "Supply 1..30 distinct run IDs");
  const directory = await outputPath(destination);
  await mkdir(directory, { mode: 0o700 });
  const artifacts: Array<{ file: string; sha256: string; bytes: number }> = [];
  let totalBytes = 0;
  const observations: Array<{ runId: string; status: unknown; purpose: unknown; platform: unknown; securityCertification: string }> = [];
  try {
    for (const id of ids) {
      const run = await runDirectory(id);
      const target = path.join(directory, id);
      await mkdir(target, { mode: 0o700 });
      const names = (await readdir(run.directory)).filter((name) => /^[a-z0-9_.-]+\.(json|png)$/.test(name)).sort();
      requireCondition(names.includes("report.json") && names.length <= 100, "Run artifact set is missing or unexpectedly large");
      for (const name of names) {
        const source = path.join(run.directory, name);
        const info = await lstat(source);
        requireCondition(info.isFile(), "Only ordinary run artifacts may be bundled");
        requireCondition(info.size <= 50 * 1024 * 1024, "Individual artifact exceeds the bounded bundle size");
        totalBytes += info.size;
        requireCondition(totalBytes <= 512 * 1024 * 1024, "Bundle exceeds 512 MiB; split the run matrix into smaller bundles");
        const bytes = await readFile(source);
        await copyFile(source, path.join(target, name));
        artifacts.push({ file: `${id}/${name}`, sha256: sha256(bytes), bytes: bytes.length });
      }
      observations.push({
        runId: id, status: run.report.status, purpose: run.report.purpose,
        platform: run.report.browser?.nativePlatform ?? run.report.host?.platform ?? "unknown",
        securityCertification: "not-performed",
      });
    }
    const manifest = {
      schemaVersion: 1, createdAt: new Date().toISOString(), m1Acceptance: "not-evaluated",
      securityCertification: "not-performed", baselineApproved: false,
      reviewRequired: "Review console text, URLs and screenshots before sharing; no browser profiles/cookies/storage are bundled.",
      observations, artifacts, artifactSetSha256: sha256(canonicalJson(artifacts)),
    };
    await writeFile(path.join(directory, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
    return { directory, runs: ids.length, artifacts: artifacts.length };
  } catch (error) {
    await rm(directory, { recursive: true, force: true });
    throw error;
  }
}

export async function removeBundledRuns(manifestFile: string) {
  requireCondition((await lstat(manifestFile)).isFile() && await realpath(manifestFile) === path.resolve(manifestFile),
    "Cleanup manifest must be an ordinary owned file");
  const manifestDirectory = await realpath(path.dirname(manifestFile));
  const ownerRoot = path.resolve(harnessRoot, "owner-runs");
  requireCondition(manifestDirectory.startsWith(`${ownerRoot}${path.sep}`), "Cleanup requires an owned owner-runs bundle");
  const manifest = JSON.parse(await readFile(manifestFile, "utf8"));
  requireCondition(manifest.schemaVersion === 1 && manifest.m1Acceptance === "not-evaluated"
    && Array.isArray(manifest.artifacts) && Array.isArray(manifest.observations), "Invalid observation bundle manifest");
  requireCondition(sha256(canonicalJson(manifest.artifacts)) === manifest.artifactSetSha256, "Bundle artifact set digest mismatch");
  for (const artifact of manifest.artifacts) {
    requireCondition(typeof artifact.file === "string" && /^[a-z0-9_-]+\/[a-z0-9_.-]+\.(json|png)$/.test(artifact.file),
      "Unsafe bundled artifact path");
    const file = path.join(manifestDirectory, artifact.file);
    requireCondition((await lstat(file)).isFile() && await realpath(file) === file, "Bundle artifact must not be a symlink");
    requireCondition(sha256(await readFile(file)) === artifact.sha256, "Bundle integrity check failed; raw runs retained");
  }
  let removed = 0;
  for (const observation of manifest.observations) {
    const { directory, report } = await runDirectory(observation.runId);
    requireCondition(report.cleanup?.runtimeRemoved && (report.cleanup.browserPid == null || report.cleanup.browserExitVerified),
      "Run does not confirm browser/runtime cleanup; inspect owned process before removing evidence");
    for (const name of await readdir(directory)) {
      const archived = manifest.artifacts.find((item: { file: string }) => item.file === `${observation.runId}/${name}`);
      requireCondition(archived && (await lstat(path.join(directory, name))).isFile(), "Run has unarchived files; cleanup refused");
      requireCondition(sha256(await readFile(path.join(directory, name))) === archived.sha256, "Raw run differs from its archive; cleanup refused");
    }
    await rm(directory, { recursive: true });
    removed++;
  }
  return { removed, bundleRetained: manifestDirectory };
}
