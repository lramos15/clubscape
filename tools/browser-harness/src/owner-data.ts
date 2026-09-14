import { z } from "zod";
import { configSchema, contractSchema, digest, parseConfig, requireCondition } from "./config.ts";
import { hostFingerprint, requireOwnerMac } from "./platform.ts";
import type { HarnessConfig } from "./config.ts";

const optionalText = z.string().nullable();
export const hostFactsSchema = z.strictObject({
  platform: z.enum(["linux", "darwin"]), architecture: z.string(), nodeVersion: z.string(),
  osVersion: optionalText, osBuild: optionalText, kernel: z.string(),
  model: optionalText, chip: optionalText, cpuCores: z.number().int().positive(),
  memoryBytes: z.number().int().positive(), arm64Hardware: z.boolean().nullable(),
  appleSilicon: z.boolean().nullable(),
  gpus: z.array(z.strictObject({ model: optionalText, vendor: optionalText, metalSupport: optionalText })),
  issues: z.array(z.string()), sources: z.array(z.string()),
});
const executableSchema = z.strictObject({
  path: z.string().min(1), version: z.string().regex(/^\d+\.\d+\.\d+\.\d+$/),
  versionOutput: z.string().min(1), sha256: digest,
  macBundle: z.strictObject({
    path: z.string(), identifier: z.string(), executableName: z.string(), shortVersion: z.string(),
    architectures: z.array(z.string()), signatureVerified: z.boolean(), signer: z.string(),
    teamIdentifier: optionalText, meaning: z.string(),
  }).optional(),
});
const discoverySchema = z.strictObject({
  product: z.enum(["chrome", "edge"]), status: z.enum(["available", "unavailable"]),
  executable: executableSchema.nullable(),
  attempts: z.array(z.strictObject({ path: z.string(), status: z.string(), detail: z.string().optional() })),
});
export const inventorySchema = z.strictObject({
  schemaVersion: z.literal(1), collectedAt: z.string(),
  scope: z.literal("local-native-discovery-not-acceptance"),
  host: hostFactsSchema, hostFingerprint: digest,
  browsers: z.strictObject({ chrome: discoverySchema, edge: discoverySchema }),
  m1Acceptance: z.literal("not-evaluated"), securityCertification: z.literal("not-performed"),
});
export type Inventory = z.infer<typeof inventorySchema>;

export const productCaseSchema = configSchema.pick({
  version: true, mode: true, url: true, allowedOrigins: true, captureCaseId: true,
}).extend({
  contract: contractSchema.omit({ hardware: true }).extend({ buildArtifactSha256: digest }),
});
export type ProductCase = z.infer<typeof productCaseSchema>;

export function validateInventory(input: unknown): Inventory {
  const inventory = inventorySchema.parse(input);
  requireCondition(hostFingerprint(inventory.host) === inventory.hostFingerprint, "Inventory host fingerprint mismatch");
  for (const product of ["chrome", "edge"] as const) {
    const browser = inventory.browsers[product];
    requireCondition(browser.product === product, "Inventory browser product keys are mismatched");
    requireCondition((browser.status === "available") === (browser.executable !== null), "Inventory availability/executable mismatch");
  }
  return inventory;
}

const probeSchema = z.object({
  purpose: z.literal("tool-fixture"),
  status: z.enum(["valid-measurement", "valid-capture"]),
  hostFingerprint: digest,
  executable: executableSchema,
  configuration: configSchema,
  browser: z.object({
    nativePlatform: z.enum(["darwin", "linux"]),
    gpu: z.object({
      devices: z.array(z.object({
        deviceString: z.string(), driverVendor: z.string(), driverVersion: z.string(),
      })).min(1),
    }),
    sandbox: z.object({
      platform: z.enum(["darwin", "linux"]),
      nativeAttestation: z.string(),
      gpuProcessSandboxed: z.boolean().nullable(),
      securityCertification: z.literal("not-performed"),
    }),
  }),
  initialRenderer: z.object({
    gpu: z.object({
      device: z.object({
        adapter: z.strictObject({ vendor: z.string(), architecture: z.string(), device: z.string(), description: z.string() }),
        fallbackAdapter: z.literal(false),
      }),
    }),
  }),
  cleanup: z.object({ browserExitVerified: z.literal(true), runtimeRemoved: z.literal(true) }),
  m1Acceptance: z.literal("not-evaluated"),
  baselineApproved: z.literal(false),
});

export function ownerSelectionReference(input: unknown): string {
  const record = z.object({
    hardware_direction: z.object({
      authority: z.literal("owner"),
      record: z.string(),
      sandbox_disable_authorized: z.literal(false),
    }),
  }).parse(input);
  requireCondition(/M(?:-| )series\s+mac/i.test(record.hardware_direction.record), "Owner hardware direction does not select an M-series Mac");
  return "milestones/m1-owner-followups.json#/hardware_direction";
}

export function prepareOwnerConfiguration(
  rawInventory: unknown,
  rawProbe: unknown,
  rawCase: unknown,
  product: "chrome" | "edge",
  ownerRecord: unknown,
  references: { inventory: string; probe: string },
): HarnessConfig {
  const inventory = validateInventory(rawInventory);
  requireOwnerMac(inventory.host);
  const probe = probeSchema.parse(rawProbe);
  const productCase = productCaseSchema.parse(rawCase);
  const executable = inventory.browsers[product].executable;
  requireCondition(executable, `No real ${product} executable recorded by discovery`);
  requireCondition(executable.macBundle?.signatureVerified && executable.macBundle.architectures.includes("arm64"),
    "Original signed ARM64 vendor bundle verification is missing");
  requireCondition(probe.browser.nativePlatform === "darwin" && probe.browser.sandbox.platform === "darwin"
    && probe.configuration.browser.product === product && probe.configuration.browser.platform === "darwin"
    && probe.configuration.browser.architecture === "arm64"
    && probe.configuration.browser.graphicsProfile === "mac-metal-default", "Probe is not from the requested real Mac browser");
  requireCondition(probe.hostFingerprint === inventory.hostFingerprint
    && probe.executable.path === executable.path && probe.executable.sha256 === executable.sha256
    && probe.executable.version === executable.version, "Probe differs from the pinned inventory/browser");
  requireCondition(probe.browser.sandbox.gpuProcessSandboxed !== false, "Probe reports an unsandboxed GPU; investigate without bypassing security");
  const selection = ownerSelectionReference(ownerRecord);
  const gpu = probe.browser.gpu.devices[0];
  const hardware = {
    id: `owner-mac-${inventory.hostFingerprint.slice(0, 16)}`,
    representativeIntegratedGraphics: true,
    ownerApprovalRef: selection,
    evidenceRefs: [selection, references.inventory, references.probe],
    gpu: gpu.deviceString || inventory.host.gpus.map((g) => g.model).filter(Boolean).join("; "),
    cpu: inventory.host.chip!,
    memory: `${inventory.host.memoryBytes} bytes; Apple Silicon unified memory architecture`,
    os: `macOS ${inventory.host.osVersion} (${inventory.host.osBuild}); ${inventory.host.model}; arm64`,
    driver: gpu.driverVersion
      ? `${gpu.driverVendor}: ${gpu.driverVersion}`
      : `macOS-managed graphics driver, OS build ${inventory.host.osBuild}; no separate driver version disclosed by CDP`,
    expectedAdapter: probe.initialRenderer.gpu.device.adapter,
  };
  const config = parseConfig({
    ...productCase, purpose: "candidate",
    browser: {
      product, expectedVersion: executable.version, executableEnv: "BROWSER_EXECUTABLE",
      executablePath: executable.path, executableSha256: executable.sha256,
      platform: "darwin", architecture: "arm64", hostFingerprint: inventory.hostFingerprint,
      graphicsProfile: "mac-metal-default",
    },
    contract: { ...productCase.contract, hardware },
  });
  requireCondition(config.contract.sourcePack !== null, "The hardware-direction record does not approve a source pack");
  return config;
}
