import { AppError } from "./errors.ts";
import type { Fetch } from "./transport.ts";

/** Local page transport policy also covers component factories that use global fetch internally. */
export function sameOriginFetch(native: Fetch, origin: string): Fetch {
  return async (input, options) => {
    const path = input instanceof Request ? input.url : input.toString();
    const url = new URL(path, origin);
    if (url.origin !== new URL(origin).origin || !["http:", "https:"].includes(url.protocol)
      || url.username || url.password || url.search || url.hash) {
      throw new AppError("Client resource transport requires a canonical same-origin URL without secrets or redirects.", { kind: "transport" });
    }
    return native(input, {
      ...options, mode: "same-origin", redirect: "error", credentials: "omit", referrerPolicy: "no-referrer",
    });
  };
}

export function installSourceFetch(): () => void {
  const original = globalThis.fetch;
  const guarded = sameOriginFetch(original.bind(globalThis), location.origin);
  globalThis.fetch = guarded;
  return () => { if (globalThis.fetch === guarded) globalThis.fetch = original; };
}
