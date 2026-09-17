export class AudioFailure extends Error {
  readonly code: string;
  readonly recoverable: boolean;
  readonly detail: Readonly<Record<string, string | number | boolean>>;

  constructor(
    code: string,
    message: string,
    recoverable = true,
    detail: Readonly<Record<string, string | number | boolean>> = {},
  ) {
    super(message);
    this.name = "AudioFailure";
    this.code = code;
    this.recoverable = recoverable;
    this.detail = Object.freeze({ ...detail });
  }
}

export function failure(error: unknown, code: string, message: string): AudioFailure {
  return error instanceof AudioFailure
    ? error
    : new AudioFailure(code, `${message}: ${error instanceof Error ? error.message : String(error)}`);
}

export function requireAudio(condition: unknown, code: string, message: string): asserts condition {
  if (!condition) throw new AudioFailure(code, message);
}

export function integer(value: unknown, min: number, max: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= min && value <= max;
}

export function unit(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 1;
}
