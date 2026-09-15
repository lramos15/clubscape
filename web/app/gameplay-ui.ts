import { GAMEPLAY_UI_CAPABILITY } from "../shared/contracts.ts";
import type { GameplayUiView, WorldView } from "../shared/contracts.ts";
import { AppError, invariant } from "./errors.ts";

export interface GameplayUiSupport {
  capability: typeof GAMEPLAY_UI_CAPABILITY;
  available: boolean;
  reason: "not_advertised" | "wire_unavailable" | "view_missing" | null;
  message: string | null;
}
const requiredFields = [
  "activeInterface", "production", "reward", "confirmation", "interfaces", "combatStyle", "combatStyles",
  "prayers", "spells", "equipment", "inventoryActions", "bank", "keptOnDeath", "recovery", "appearance", "publicChat",
] as const satisfies readonly (keyof GameplayUiView)[];

function decimal(value: unknown, signed = false): void {
  invariant(typeof value === "string" && (signed ? /^-?\d+$/ : /^\d+$/).test(value)
    && BigInt(value.startsWith("-") ? value.slice(1) : value) <= 18446744073709551615n,
  "Authoritative gameplay UI numeric values must remain exact decimal strings.", "protocol");
}

/** Validates the published shared view boundary, without deriving permissions, prices or defaults. */
export function validateGameplayUi(view: GameplayUiView): void {
  invariant(view && typeof view === "object" && view.version === 1
    && requiredFields.every((key) => Object.hasOwn(view, key) && view[key] !== undefined),
  "game.ui.v1 requires the complete version-1 WorldView.ui projection, not an empty default.", "protocol");
  for (const field of ["interfaces", "combatStyles", "prayers", "spells", "inventoryActions"] as const) {
    invariant(Array.isArray(view[field]), `The authoritative UI field ${field} is missing.`, "protocol");
  }
  invariant(view.equipment && view.appearance && view.publicChat && view.publicChat.channel === "public"
    && Array.isArray(view.publicChat.messages), "The authoritative equipment/appearance/public-chat UI projection is missing.", "protocol");
  decimal(view.equipment.weightGrams, true);
  if (view.reward !== null) {
    invariant(Array.isArray(view.reward.xp) && view.reward.continuation !== null,
      "The authoritative reward continuation is missing.", "protocol");
    for (const award of view.reward.xp) decimal(award.amountTenths);
  }
  if (view.confirmation?.credit !== null && view.confirmation !== null) decimal(view.confirmation.credit);
  if (view.bank !== null) {
    decimal(view.bank.revision);
    invariant(Array.isArray(view.bank.entries) && new Set(view.bank.entries.map((entry) => entry.id)).size === view.bank.entries.length
      && view.bank.entries.every((entry) => entry.placeholder === (entry.value === null)
        && (entry.value === null || entry.value.id === entry.item)),
    "Authoritative bank entry identities/placeholders are invalid.", "protocol");
  }
  if (view.keptOnDeath !== null) {
    invariant(view.keptOnDeath.scope === "normal_unsafe_non_pvp", "Unsupported authoritative death-preview scope.", "protocol");
    decimal(view.keptOnDeath.fullGraveFee); decimal(view.keptOnDeath.fullOfficeFee); decimal(view.keptOnDeath.valueRevision);
  }
  if (view.recovery !== null) decimal(view.recovery.cofferBalance);
  if (view.production !== null) {
    // The nullable-target correction has not been relayed. Never manufacture a
    // target for inventory-only production to make this published shape pass.
    invariant(view.production.target !== null && view.production.target !== undefined,
      "Production target nullability requires the exact shared contract correction; no dummy target was supplied.", "unsupported_protocol");
  }
}

export function gameplayUiSupport(
  capabilities: readonly string[], wireSupported: boolean, world: WorldView | null,
): Readonly<GameplayUiSupport> {
  const advertised = capabilities.includes(GAMEPLAY_UI_CAPABILITY);
  if (world?.ui !== undefined) {
    if (!advertised || !wireSupported) {
      throw new AppError("An authoritative UI view was returned without negotiated game.ui.v1 and a supported generated wire decoder.", {
        kind: "protocol", recoverable: false,
      });
    }
    validateGameplayUi(world.ui);
  }
  const reason = !advertised ? "not_advertised" : !wireSupported ? "wire_unavailable" : world?.ui === undefined ? "view_missing" : null;
  const message = reason === "not_advertised"
    ? "This server does not advertise game.ui.v1. Full authoritative production/reward/bank/death/chat controls are unsupported; no empty UI projection was substituted."
    : reason === "wire_unavailable"
      ? "The server advertises game.ui.v1, but this client has no generated gameplay UI wire support yet. Published types alone do not implement the API."
      : reason === "view_missing"
        ? "The negotiated game.ui.v1 server did not return WorldView.ui version 1."
        : null;
  return Object.freeze({ capability: GAMEPLAY_UI_CAPABILITY, available: reason === null, reason, message });
}
