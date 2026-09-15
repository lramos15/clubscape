import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";

export const APPROVAL_PATH = "milestones/approvals/m1-reference-pack-v1.3.0.json";
export const APPROVAL_SHA256 = "e910cb02b6984a94be542af1c56e29dc67bfe2e1cd14b72deda8b4b369feeb70";
export const PACK_PATH = "research/reference-pack/v1/manifest.json";
const VALIDATOR_PATH = "tools/reference-pack/validate.py";
const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const digest = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");
const contextPaths = ["spec/art-style.md", "spec/interface-parity.md"] as const;

interface BoundDocument { path: string; sha256: string; size_bytes: number }
interface SourceManifest { bound_existing_documents: BoundDocument[] }
export interface ReferenceIntegrity {
  sourcePackSha256: string;
  approvalSha256: string;
  frozenContext: BoundDocument[];
  validator: { path: string; sha256: string; strictCompleteness: true };
  checked: { hashes: number; cases: number; tutorialStates: number; nativeImages: number; flacs: number };
}

export function verifyAuthority(approvalBytes: Uint8Array, packBytes: Uint8Array): SourceManifest {
  if (digest(approvalBytes) !== APPROVAL_SHA256) throw new Error("Exact external owner approval bytes changed.");
  const approval = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(approvalBytes)) as {
    authority: string; decision: string; reference_pack: string; reference_pack_sha256: string;
  };
  if (approval.authority !== "owner" || approval.decision !== "approved" || approval.reference_pack !== PACK_PATH
    || approval.reference_pack_sha256 !== SOURCE_PACK_SHA256) throw new Error("Exact source-pack implementation authority is absent.");
  if (digest(packBytes) !== SOURCE_PACK_SHA256) throw new Error("Exact approved reference-pack bytes changed.");
  return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(packBytes)) as SourceManifest;
}

export async function verifyFrozenContext(manifest: SourceManifest, read: (path: string) => Promise<Uint8Array>): Promise<BoundDocument[]> {
  const result: BoundDocument[] = [];
  for (const path of contextPaths) {
    const records = manifest.bound_existing_documents.filter((record) => record.path === path);
    if (records.length !== 1) throw new Error(`Frozen source context is missing or duplicated: ${path}`);
    const record = records[0]!;
    const bytes = await read(path);
    if (bytes.length !== record.size_bytes || digest(bytes) !== record.sha256) {
      throw new Error(`Frozen pre-approval context changed: ${path}. Update current guidance, never these source bytes.`);
    }
    result.push({ ...record });
  }
  return result;
}

export async function verifyReferenceIntegrity(): Promise<ReferenceIntegrity> {
  const [approval, pack] = await Promise.all([
    readFile(resolve(root, APPROVAL_PATH)), readFile(resolve(root, PACK_PATH)),
  ]);
  const manifest = verifyAuthority(approval, pack);
  const frozenContext = await verifyFrozenContext(manifest, async (path) => {
    const file = resolve(root, path);
    if (!file.startsWith(root + sep)) throw new Error("Frozen context escaped the worktree.");
    return readFile(file);
  });
  // Invoke the existing complete validator unchanged. --report is deliberately
  // absent: browser builds must not rewrite the frozen source evidence tree.
  const raw = execFileSync("python3", ["-B", VALIDATOR_PATH, "--require-complete"], {
    cwd: root, encoding: "utf8", maxBuffer: 2 * 1024 * 1024, timeout: 180_000,
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
  });
  const report = JSON.parse(raw) as {
    result: string; manifest: { sha256: string }; complete_reference_pack: boolean;
    hash_bound_files_checked: number; required_case_ids_checked: number; tutorial_states_mapped: number;
    original_images_decoded: number; native_hud_images_decoded: number; audio: { files_decoded: number };
  };
  if (report.result !== "passed_source_input_integrity_and_case_index"
    || report.manifest.sha256 !== SOURCE_PACK_SHA256 || !report.complete_reference_pack) {
    throw new Error("The unchanged strict reference validator did not establish complete source integrity.");
  }
  const integrity: ReferenceIntegrity = {
    sourcePackSha256: SOURCE_PACK_SHA256, approvalSha256: APPROVAL_SHA256, frozenContext,
    validator: { path: VALIDATOR_PATH, sha256: digest(await readFile(resolve(root, VALIDATOR_PATH))), strictCompleteness: true },
    checked: {
      hashes: report.hash_bound_files_checked, cases: report.required_case_ids_checked,
      tutorialStates: report.tutorial_states_mapped,
      nativeImages: report.original_images_decoded + report.native_hud_images_decoded,
      flacs: report.audio.files_decoded,
    },
  };
  await mkdir(resolve(root, ".local/evidence"), { recursive: true });
  await writeFile(resolve(root, ".local/evidence/browser-reference-integrity.json"), JSON.stringify({
    kind: "strict-source-integrity-with-external-owner-authority", recordedAt: new Date().toISOString(),
    ...integrity, frozenStatusIsHistorical: true, candidateAccepted: false,
  }, null, 2) + "\n");
  return integrity;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(JSON.stringify(await verifyReferenceIntegrity()));
}
