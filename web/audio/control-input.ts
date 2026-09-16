import { AudioFailure, failure, requireAudio } from "./errors.ts";

export const SOURCE_CONTROL_CUE = 2266;
export interface SourceControlInputState {
  readonly phase: "pending" | "ready" | "failed" | "disposed";
  readonly errorCode: string | null;
}

/** Input readiness is established before a control is allowed to create a source deadline. */
export class SourceControlInput<T> {
  private phase: SourceControlInputState["phase"] = "pending";
  private value: T | null = null;
  private error: AudioFailure | null = null;
  private pending: Promise<void> | null = null;
  private readonly load: () => Promise<T>;
  private readonly changed: (state: SourceControlInputState) => void;

  constructor(load: () => Promise<T>, changed: (state: SourceControlInputState) => void) {
    this.load = load;
    this.changed = changed;
  }

  get state(): SourceControlInputState {
    return Object.freeze({ phase: this.phase, errorCode: this.error?.code ?? null });
  }

  prepare(): Promise<void> {
    if (this.phase === "disposed") return Promise.reject(new AudioFailure("AUDIO_DISPOSED", "The control input was disposed."));
    if (this.error) return Promise.reject(this.error);
    if (this.phase === "ready") return Promise.resolve();
    if (this.pending) return this.pending;
    this.changed(this.state);
    this.pending = Promise.resolve().then(this.load).then((value) => {
      requireAudio(this.phase !== "disposed", "AUDIO_CANCELLED", "Control input preparation was cancelled.");
      requireAudio(value !== null && value !== undefined, "AUDIO_CONTROL_INPUT", "Control input preparation returned no decoded input.");
      this.value = value;
      this.phase = "ready";
      this.changed(this.state);
    }).catch((error: unknown) => {
      const problem = failure(error, "AUDIO_CONTROL_INPUT", "Cannot prepare the original native control input");
      if (this.phase !== "disposed") {
        this.error = problem;
        this.phase = "failed";
        this.changed(this.state);
      }
      throw problem;
    });
    return this.pending;
  }

  requireReady(): T {
    requireAudio(this.phase !== "disposed", "AUDIO_DISPOSED", "The original control input is disposed.");
    if (this.error) throw this.error;
    requireAudio(this.phase === "ready" && this.value !== null, "AUDIO_CONTROL_INPUT_PENDING",
      "The original native control input is not ready. No control deadline was admitted.");
    return this.value;
  }

  dispose(): void {
    if (this.phase === "disposed") return;
    this.value = null;
    this.phase = "disposed";
    this.changed(this.state);
  }
}
