export class AppError extends Error {
  readonly kind: string;
  readonly errorId: string;
  readonly recoverable: boolean;
  readonly code: number | null;
  readonly retryAfterSeconds: number;

  constructor(message: string, options: {
    kind?: string; errorId?: string; recoverable?: boolean; code?: number | null; retryAfterSeconds?: number;
  } = {}) {
    super(message);
    this.name = "AppError";
    this.kind = options.kind ?? "client";
    this.errorId = options.errorId ?? crypto.randomUUID();
    this.recoverable = options.recoverable ?? true;
    this.code = options.code ?? null;
    this.retryAfterSeconds = Math.min(60, Math.max(0, options.retryAfterSeconds ?? 0));
  }
}

export function appError(value: unknown, fallback = "The client could not complete this operation."): AppError {
  if (value instanceof AppError) return value;
  // wasm-bindgen errors are structured JSON strings, never echoed request bodies.
  if (typeof value === "string" && value.length < 8192) {
    try {
      const parsed = JSON.parse(value) as Record<string, unknown>;
      if (typeof parsed.message === "string" && typeof parsed.kind === "string") {
        return new AppError(parsed.message, {
          kind: parsed.kind,
          ...(typeof parsed.errorId === "string" ? { errorId: parsed.errorId } : {}),
          ...(typeof parsed.code === "number" ? { code: parsed.code } : {}),
          ...(typeof parsed.recoverable === "boolean" ? { recoverable: parsed.recoverable } : {}),
          ...(typeof parsed.retryAfterSeconds === "number" ? { retryAfterSeconds: parsed.retryAfterSeconds } : {}),
        });
      }
    } catch { /* Unstructured engine/browser exceptions are not public diagnostics. */ }
  }
  return new AppError(fallback);
}

export function invariant(value: unknown, message: string, kind = "asset"): asserts value {
  if (!value) throw new AppError(message, { kind, recoverable: false });
}

export function deepFreeze<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}
