import { ACTOR_OBSERVER_CAPABILITY, GAMEPLAY_UI_CAPABILITY, UI_AMOUNTS_CAPABILITY, UI_RECOVERY_CAPABILITY } from "../shared/contracts.ts";
import type { ActorActionView, GameIntent, GameplayUiIntent, GameplayUiView, RecoveryManagementView, UiAmount, UiPermission, WorldView } from "../shared/contracts.ts";
import { AppError, invariant } from "./errors.ts";

export interface GameplayUiSupport {
  capability: typeof GAMEPLAY_UI_CAPABILITY;
  available: boolean;
  reason: "not_advertised" | "wire_unavailable" | "view_missing" | null;
  message: string | null;
  amounts: boolean;
  recovery: boolean;
  complete: boolean;
}
const requiredFields = [
  "activeTab", "activeInterface", "production", "reward", "confirmation", "document", "interfaces", "combatStyle", "combatStyles",
  "prayers", "spells", "equipment", "inventoryActions", "bank", "keptOnDeath", "recovery", "appearance", "publicChat",
] as const satisfies readonly (keyof GameplayUiView)[];

export function decimal(value: unknown, signed = false): void {
  invariant(typeof value === "string" && (signed ? /^-?\d+$/ : /^\d+$/).test(value)
    && BigInt(value.startsWith("-") ? value.slice(1) : value) <= 18446744073709551615n,
  "Authoritative gameplay UI numeric values must remain exact decimal strings.", "protocol");
}

const bankKinds: ReadonlySet<string> = new Set([
  "bank_select_tab", "bank_create_tab", "bank_move", "bank_collapse_tab", "bank_set_insert",
  "bank_set_placeholders", "bank_release_placeholder", "bank_placeholder", "bank_deposit_equipment",
  "bank_withdraw_entry", "bank_set_options",
  "bank_set_amount", "recovery_bank_all",
] satisfies GameplayUiIntent["kind"][]);

export function isBankUiRequest(intent: GameIntent): intent is GameplayUiIntent {
  return bankKinds.has(intent.kind);
}

/** Capture the displayed bank revision before request queuing; never retarget an explicit retry. */
export function captureUiBankRevision(intent: GameIntent, world: WorldView | null): GameIntent {
  if (!isBankUiRequest(intent)) return intent;
  const revision = intent.expected_bank_revision ?? (intent.kind === "recovery_bank_all"
    ? world?.ui?.recovery?.management?.bankRevision : world?.ui?.bank?.revision);
  if (revision === undefined) throw new AppError("The source bank revision is unavailable; no bank control request was sent.", { kind: "state" });
  decimal(revision);
  return { ...intent, expected_bank_revision: revision };
}

export function validateUiAmount(value: UiAmount): void {
  invariant(value && typeof value === "object" && !Array.isArray(value)
    && ((value.kind === "all" && Object.keys(value).length === 1)
      || (value.kind === "quantity" && Object.keys(value).length === 2 && Number.isSafeInteger(value.quantity)
        && value.quantity > 0 && value.quantity <= 0xffffffff)),
  "A source UI amount must be an explicit positive quantity or All, never a numeric sentinel.", "protocol");
}

function permission(value: UiPermission): void {
  invariant(value && typeof value.allowed === "boolean"
    && (value.allowed ? value.code === null && value.reason === null
      : typeof value.code === "string" && value.code.length > 0 && typeof value.reason === "string" && value.reason.length > 0),
  "The authoritative source permission is incomplete.", "protocol");
}

function recoveryManagement(value: RecoveryManagementView): void {
  invariant(value && Array.isArray(value.panels) && Array.isArray(value.bankAllRecords),
    "The recovery management projection is incomplete.", "protocol");
  decimal(value.bankRevision);
  permission(value.bankAll);
  const panels = new Set<string>();
  for (const panel of value.panels) {
    invariant(panel && typeof panel.death === "string" && panel.death.length > 0
      && ["grave", "death_office"].includes(panel.storage) && Array.isArray(panel.entries)
      && !panels.has(`${panel.death}/${panel.storage}`), "Invalid recovery panel identity.", "protocol");
    panels.add(`${panel.death}/${panel.storage}`);
    decimal(panel.fullSelectionFee); permission(panel.takeAll);
    const entries = new Set<string>();
    for (const entry of panel.entries) {
      invariant(entry && typeof entry.id === "string" && entry.id.length > 0 && !entries.has(entry.id)
        && entry.item && typeof entry.item.id === "string" && Number.isSafeInteger(entry.item.quantity) && entry.item.quantity > 0
        && [entry.inventoryCapacity, entry.bankCapacity].every((count) => Number.isSafeInteger(count) && count >= 0 && count <= 0xffffffff),
      "Invalid identity-bound recovery entry/capacity.", "protocol");
      entries.add(entry.id);
      decimal(entry.unitFee); decimal(entry.fullStackFee); permission(entry.take); permission(entry.bank);
    }
  }
  const records = new Set<string>();
  for (const record of value.bankAllRecords) {
    invariant(record && typeof record.death === "string" && record.death.length > 0 && !records.has(record.death)
      && Array.isArray(record.items) && record.items.length > 0 && new Set(record.items).size === record.items.length
      && record.items.every((id) => typeof id === "string" && id.length > 0),
    "Invalid opaque recovery Bank-All record selection.", "protocol");
    records.add(record.death);
  }
}

function validateUiExtensions(view: GameplayUiView, capabilities: readonly string[]): void {
  const amounts = capabilities.includes(UI_AMOUNTS_CAPABILITY), recovery = capabilities.includes(UI_RECOVERY_CAPABILITY);
  invariant(view.production === null || view.production.recipes.every((recipe) => (recipe.all !== undefined) === amounts),
    "UI amount negotiation requires each source recipe's explicit All permission.", "protocol");
  invariant(view.bank === null || (view.bank.amountSelection !== undefined) === amounts,
    "UI amount negotiation requires the source bank's semantic amount selection.", "protocol");
  invariant(view.recovery === null || (view.recovery.management !== undefined) === recovery,
    "UI recovery negotiation requires the complete management projection.", "protocol");
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
  invariant(view.activeTab === null || typeof view.activeTab === "string", "Invalid authoritative active tab.", "protocol");
  if (view.document !== null) {
    invariant(typeof view.document.id === "string" && view.document.id.length > 0
      && typeof view.document.interface === "string" && typeof view.document.title === "string"
      && Array.isArray(view.document.pages) && view.document.pages.every((page) => typeof page === "string")
      && Number.isSafeInteger(view.document.page) && view.document.page >= 0
      && typeof view.document.nativeMap === "boolean"
      && (view.document.mapAsset === null || typeof view.document.mapAsset === "string"),
    "The authoritative document/native-map projection is invalid.", "protocol");
  }
  if (view.reward !== null) {
    invariant(Array.isArray(view.reward.xp) && view.reward.continuation !== null,
      "The authoritative reward continuation is missing.", "protocol");
    for (const award of view.reward.xp) decimal(award.amountTenths);
  }
  if (view.confirmation?.credit !== null && view.confirmation !== null) decimal(view.confirmation.credit);
  if (view.bank !== null) {
    decimal(view.bank.revision);
    if (view.bank.amountSelection !== undefined) validateUiAmount(view.bank.amountSelection);
    invariant(Array.isArray(view.bank.entries) && new Set(view.bank.entries.map((entry) => entry.id)).size === view.bank.entries.length
      && view.bank.entries.every((entry) => entry.placeholder === (entry.value === null)
        && (entry.value === null || entry.value.id === entry.item)),
    "Authoritative bank entry identities/placeholders are invalid.", "protocol");
  }
  if (view.keptOnDeath !== null) {
    invariant(view.keptOnDeath.scope === "normal_unsafe_non_pvp", "Unsupported authoritative death-preview scope.", "protocol");
    decimal(view.keptOnDeath.fullGraveFee); decimal(view.keptOnDeath.fullOfficeFee); decimal(view.keptOnDeath.valueRevision);
  }
  if (view.recovery !== null) {
    decimal(view.recovery.cofferBalance);
    if (view.recovery.management !== undefined) recoveryManagement(view.recovery.management);
  }
  if (view.production !== null) {
    invariant(Object.hasOwn(view.production, "target") && view.production.target !== undefined,
      "The authoritative production target field is missing; inventory-only production must carry explicit null.", "protocol");
    const target = view.production.target;
    invariant(target === null || (typeof target === "object" && !Array.isArray(target)
      && ((target.kind === "spawn" && typeof target.spawn === "string" && target.spawn.length > 0)
        || (target.kind === "temporary_object" && typeof target.object === "string" && target.object.length > 0))),
    "The authoritative production target is neither null nor a typed world target.", "protocol");
    invariant(Array.isArray(view.production.recipes), "Missing authoritative production choices.", "protocol");
    for (const recipe of view.production.recipes) if (recipe.all !== undefined) permission(recipe.all);
  }
}

export function validateActorObservers(capabilities: readonly string[], world: WorldView | null): void {
  if (world === null || !capabilities.includes(ACTOR_OBSERVER_CAPABILITY)) return;
  const action = (value: ActorActionView): void => {
    invariant(value.version === 1 && typeof value.id === "string" && value.id.length > 0
      && typeof value.activity === "string", "Invalid source action observer identity/version.", "protocol");
    decimal(value.startedAtTick); decimal(value.cycleStartedAtTick); decimal(value.observedAtTick);
    if (value.nextActionTick !== null) decimal(value.nextActionTick);
    for (const field of ["actionId", "recipeId", "styleId", "spellId", "animation"] as const) {
      invariant(value[field] === null || typeof value[field] === "string", "An actor observer omitted its nullable source identity.", "protocol");
    }
    invariant(value.target === null || (value.target !== undefined && typeof value.target === "object"
      && ((value.target.kind === "spawn" && typeof value.target.spawn === "string")
        || (value.target.kind === "temporary_object" && typeof value.target.object === "string"))),
    "Invalid actor observer target.", "protocol");
  };
  for (const actor of [world.player, ...world.entities.filter((entity) => entity.kind === "player")]) {
    invariant(typeof actor.running === "boolean" && Object.hasOwn(actor, "movementTick") && Object.hasOwn(actor, "action"),
      "game.observer.v1 requires actual movement and nullable action observations.", "protocol");
    if (actor.movementTick !== null) decimal(actor.movementTick);
    invariant(actor.action !== undefined, "The source action observer is missing.", "protocol");
    if (actor.action !== null) action(actor.action);
  }
}

export function gameplayUiSupport(
  capabilities: readonly string[], wireSupported: boolean, world: WorldView | null,
): Readonly<GameplayUiSupport> {
  const advertised = capabilities.includes(GAMEPLAY_UI_CAPABILITY);
  const amounts = capabilities.includes(UI_AMOUNTS_CAPABILITY), recovery = capabilities.includes(UI_RECOVERY_CAPABILITY);
  invariant(advertised || (!amounts && !recovery), "UI extensions require the versioned base UI capability.", "protocol");
  if (world?.ui !== undefined) {
    if (!advertised || !wireSupported) {
      throw new AppError("An authoritative UI view was returned without negotiated game.ui.v1 and a supported generated wire decoder.", {
        kind: "protocol", recoverable: false,
      });
    }
    validateGameplayUi(world.ui);
    validateUiExtensions(world.ui, capabilities);
  }
  const reason = !advertised ? "not_advertised" : !wireSupported ? "wire_unavailable" : world?.ui === undefined ? "view_missing" : null;
  const message = reason === "not_advertised"
    ? "This server does not advertise game.ui.v1. Full authoritative production/reward/bank/death/chat controls are unsupported; no empty UI projection was substituted."
    : reason === "wire_unavailable"
      ? "The server advertises game.ui.v1, but this client has no generated gameplay UI wire support yet. Published types alone do not implement the API."
      : reason === "view_missing"
        ? "The negotiated game.ui.v1 server did not return WorldView.ui version 1."
        : null;
  return Object.freeze({ capability: GAMEPLAY_UI_CAPABILITY, available: reason === null, reason, message,
    amounts, recovery, complete: reason === null && amounts && recovery });
}
