import { AppError, invariant } from "./errors.ts";

export type Fetch = typeof globalThis.fetch;
export const RPC_PATH = "/v1/rpc";
export const MAX_RESPONSE_BYTES = 256 * 1024;

export async function boundedBytes(response: Response, maximum: number): Promise<Uint8Array<ArrayBuffer>> {
  const length = response.headers.get("content-length");
  if (length !== null) {
    invariant(/^\d+$/.test(length) && Number(length) <= maximum, "The response exceeds its byte budget.", "transport");
  }
  invariant(response.body, "The server returned an empty response body.", "transport");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      size += next.value.byteLength;
      invariant(size <= maximum, "The response exceeds its byte budget.", "transport");
      chunks.push(next.value);
    }
  } catch (error) {
    await reader.cancel().catch(() => {});
    throw error;
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

export class RpcTransport {
  #fetch: Fetch;
  #active = new Set<AbortController>();
  #closed = false;
  readonly timeoutMs: number;

  constructor(fetcher: Fetch = globalThis.fetch.bind(globalThis), timeoutMs = 12_000) {
    this.#fetch = fetcher;
    this.timeoutMs = timeoutMs;
  }

  async post(bytes: Uint8Array, token: string | undefined): Promise<Uint8Array<ArrayBuffer>> {
    if (this.#closed) throw new AppError("The connection is closed.", { kind: "transport" });
    invariant(bytes.byteLength > 0 && bytes.byteLength <= 16 * 1024, "Invalid RPC request size.", "protocol");
    const controller = new AbortController();
    this.#active.add(controller);
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    const body = new Uint8Array(bytes);
    try {
      const headers = new Headers({ "content-type": "application/x-protobuf", accept: "application/x-protobuf" });
      if (token !== undefined) headers.set("authorization", `Bearer ${token}`);
      const response = await this.#fetch(RPC_PATH, {
        method: "POST", body, headers, signal: controller.signal,
        redirect: "error", mode: "same-origin", credentials: "omit",
        cache: "no-store", referrerPolicy: "no-referrer",
      });
      invariant(!response.redirected, "The account endpoint attempted a redirect.", "transport");
      invariant(response.headers.get("content-type")?.split(";")[0]?.trim() === "application/x-protobuf",
        "The account endpoint did not return binary Protobuf.", "transport");
      // Structured 4xx/5xx errors must reach client-core for correlation/auth reconciliation.
      return await boundedBytes(response, MAX_RESPONSE_BYTES);
    } catch (error) {
      if (error instanceof AppError) throw error;
      throw new AppError(
        controller.signal.aborted ? "The server connection timed out. The operation's outcome is not yet known."
          : "The server connection was interrupted. The operation's outcome is not yet known.",
        { kind: "transport" },
      );
    } finally {
      body.fill(0);
      clearTimeout(timer);
      this.#active.delete(controller);
    }
  }

  abort(): void {
    for (const controller of this.#active) controller.abort();
  }

  dispose(): void {
    this.#closed = true;
    this.abort();
  }
}
