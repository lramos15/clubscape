import { invariant } from "./errors.ts";

export function canonicalJson(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  return `{${Object.entries(value).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
    .map(([key, value]) => `${JSON.stringify(key)}:${canonicalJson(value)}`).join(",")}}`;
}

export async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new Uint8Array(bytes));
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function isHash(value: unknown): value is string {
  return typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
}

export function publicPath(value: unknown, prefix?: string): asserts value is string {
  invariant(typeof value === "string" && value.length <= 256 && value.startsWith("/") && !value.startsWith("//")
    && value.slice(1).split("/").every((part) => /^[A-Za-z0-9_-][A-Za-z0-9_.-]*$/.test(part))
    && !value.startsWith("/v1/") && value !== "/healthz"
    && (prefix === undefined || value.startsWith(prefix)),
  "An asset reference is not a canonical same-origin public path.");
}

export function assetId(value: unknown): asserts value is string {
  invariant(typeof value === "string" && /^[a-z][a-z0-9_.-]{0,191}$/.test(value), "Invalid stable content asset ID.");
}
