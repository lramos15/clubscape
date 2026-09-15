import initWasm, { BrowserClient } from "../generated/protocol/clubscape_wasm.js";
import wasmUrl from "../generated/protocol/clubscape_wasm_bg.wasm?url";
import { invariant } from "./errors.ts";
import { boundedBytes } from "./transport.ts";
export { BrowserApp } from "./client.ts";
export { RpcTransport } from "./transport.ts";
export { checkCapability } from "./capability.ts";
export { AssetLoader } from "./assets.ts";
export { parseContentManifest } from "./manifest.ts";
export { loadBuild, verifiedJson } from "./build.ts";
export { SourceAudioSession, audioProblem, playbackEnabled, sourceControlState } from "./audio.ts";
export { AUDIO_INPUTS } from "../audio/index.ts";
export { gameplayUiSupport, validateGameplayUi } from "./gameplay-ui.ts";
export type { GameplayUiView, GameplayUiIntent } from "../shared/contracts.ts";
export { sourceAudioDefaults, sourceSliderToMixer, sourceMixerToAssetGain, SOURCE_MUSIC_MODE_IDS } from "../audio/index.ts";
export type { SourceAudioScene, SourceMusicState } from "../audio/index.ts";
export { sourceAudioPreferenceDefaults, parseSourceAudioPreferences, deserializeSourceAudioPreferences, serializeSourceAudioPreferences } from "../audio/index.ts";
export type { SourceAudioPreferences, SourceAudioPreferenceBinding, SourceMusicPreferences, SourceMusicSkipResult } from "../audio/index.ts";
export { PlayerAudioPreferenceStore, browserPlayerAudioStorage, PLAYER_AUDIO_PREFERENCE_PREFIX } from "./player-audio-store.ts";
export type { PlayerAudioStorage } from "./player-audio-store.ts";
export { PlayerAudioPreferences } from "./player-audio.ts";
export type { PlayerAudioControls } from "./player-audio.ts";
export { PlayerAudioComposition } from "./player-audio-composition.ts";
export type { PublicWorld, PublicRecovery, QuoteRequest, QuoteView, ShopPurchaseIntent } from "./public-state.ts";

/** Programmatic bridge entry; not a UI, fixture world, or account bypass. */
export async function createProtocolClient(): Promise<BrowserClient> {
  const response = await fetch(wasmUrl, {
    mode: "same-origin", credentials: "omit", redirect: "error", referrerPolicy: "no-referrer",
    signal: AbortSignal.timeout(30_000),
  });
  invariant(response.ok && response.headers.get("content-type")?.split(";")[0]?.trim() === "application/wasm",
    "The real protocol WASM module could not be loaded.", "wasm");
  const bytes = await boundedBytes(response, 16 * 1024 * 1024);
  await initWasm({ module_or_path: bytes });
  return new BrowserClient();
}
