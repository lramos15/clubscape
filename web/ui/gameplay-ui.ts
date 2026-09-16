import { GAMEPLAY_UI_CAPABILITY } from "../shared/contracts.ts";
import type { GameIntent, GameplayUiIntent, GameplayUiView, ItemView, UiAmount, UiPermission, WorldView } from "../shared/contracts.ts";

export interface UiContractProblem { message: string; code: string }

type Validator = (value: unknown, path: string) => string | null;
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const scalar = (expected: string, accepts: (value: unknown) => boolean): Validator =>
  (value, path) => accepts(value) ? null : `${path}: expected ${expected}`;
const text = scalar("text", value => typeof value === "string");
const identity = scalar("a nonempty identity", value => typeof value === "string" && value.length > 0);
const bool = scalar("a boolean", value => typeof value === "boolean");
const integer = (minimum: number, maximum: number): Validator => scalar(`an integer in ${minimum}..${maximum}`,
  value => typeof value === "number" && Number.isInteger(value) && value >= minimum && value <= maximum);
const u32 = integer(0, 4294967295), quantity = integer(1, 4294967295);
const decimal = (signed = false): Validator => scalar("lossless decimal integer text",
  value => {
    if (typeof value !== "string" || !(signed ? /^(0|-?[1-9]\d{0,18})$/ : /^(0|[1-9]\d{0,19})$/).test(value)) return false;
    const number = BigInt(value);
    return signed ? number >= -9223372036854775808n && number <= 9223372036854775807n
      : number <= 18446744073709551615n;
  });
const oneOf = (...values: readonly unknown[]): Validator => scalar(values.map(String).join(" | "), value => values.includes(value));
const nullable = (check: Validator): Validator => (value, path) => value === null ? null : check(value, path);
const optional = (check: Validator): Validator => (value, path) => value === undefined ? null : check(value, path);
function object(fields: Record<string, Validator>, exact = false): Validator {
  return (value, path) => {
    if (!isRecord(value)) return `${path}: expected an object`;
    for (const [key, check] of Object.entries(fields)) {
      const problem = check(value[key], `${path}.${key}`);
      if (problem) return problem;
    }
    if (exact) for (const key of Object.keys(value)) if (!Object.hasOwn(fields, key)) return `${path}.${key}: unexpected request field`;
    return null;
  };
}
function array(check: Validator, unique?: string): Validator {
  return (value, path) => {
    if (!Array.isArray(value)) return `${path}: expected an array`;
    const seen = new Set<unknown>();
    for (let index = 0; index < value.length; index++) {
      const problem = check(value[index], `${path}[${index}]`);
      if (problem) return problem;
      if (unique && isRecord(value[index])) {
        const key = value[index][unique];
        if (seen.has(key)) return `${path}[${index}].${unique}: duplicate identity`;
        seen.add(key);
      }
    }
    return null;
  };
}
const dictionary = (check: Validator): Validator => (value, path) => {
  if (!isRecord(value)) return `${path}: expected an object`;
  for (const [key, entry] of Object.entries(value)) {
    const problem = check(entry, `${path}.${key}`);
    if (problem) return problem;
  }
  return null;
};
const permission = object({ allowed: bool, code: nullable(text), reason: nullable(text) });
const item = object({
  id: identity, name: text, quantity, sourceId: nullable(u32), iconAsset: nullable(identity),
  instanceId: nullable(identity), charges: nullable(u32), actions: array(text),
} satisfies Record<keyof ItemView, Validator>);
const ability = object({ id: identity, name: text, selected: bool, visible: bool, permission });
const inventoryActions = object({
  slot: integer(0, 255), item: identity, instance: nullable(identity),
  actions: array(object({ id: identity, label: text, permission }), "id"),
});
const target: Validator = (value, path) => {
  if (!isRecord(value)) return `${path}: expected an authoritative world target`;
  return (value.kind === "spawn" ? object({ kind: oneOf("spawn"), spawn: identity }, true)
    : object({ kind: oneOf("temporary_object"), object: identity }, true))(value, path);
};
const storage = oneOf("grave", "death_office");
const amount: Validator = (value, path) => {
  if (!isRecord(value)) return `${path}: expected a source quantity or All selection`;
  return (value.kind === "all" ? object({ kind: oneOf("all") }, true)
    : object({ kind: oneOf("quantity"), quantity }, true))(value, path);
};
const recoveryRecord = object({ death: identity, items: array(identity) }, true);
const intentFields: Record<GameplayUiIntent["kind"], Record<string, Validator>> = {
  production_select_all: { menu_id: identity, recipe: identity },
  bank_set_amount: { amount, noted: bool },
  recovery_take: { death: identity, storage, items: array(object({ id: identity, amount }, true)) },
  recovery_bank_all: { records: array(recoveryRecord) },
  ui_document_page: { document_id: identity, page: integer(0, 65535) },
  bank_placeholder: { entry_id: identity },
  ui_dismiss: { presentation_id: identity },
  production_select: { menu_id: identity, recipe: identity, quantity, mode: oneOf("single", "make_x") },
  item_action: { inventory_slot: integer(0, 255), expected_item: identity, expected_instance: nullable(identity), action: identity },
  bank_select_tab: { tab: integer(0, 255) },
  bank_create_tab: { entry_id: identity },
  bank_move: { entry_id: identity, before_entry_id: nullable(identity), tab: integer(0, 255) },
  bank_collapse_tab: { tab: integer(0, 255) },
  bank_set_insert: { enabled: bool },
  bank_set_placeholders: { enabled: bool },
  bank_release_placeholder: { entry_id: identity },
  bank_deposit_equipment: {},
  bank_withdraw_entry: { entry_id: identity, quantity, noted: bool },
  bank_set_options: { amount: u32, noted: bool },
  open_death_preview: {},
  request_recovery_discard: { death: identity, storage, items: array(identity) },
  coffer_offer: { inventory_slot: integer(0, 255), expected_item: identity, expected_instance: nullable(identity), quantity },
  ui_confirm: { confirmation_id: identity, accept: bool },
  public_chat: { channel: oneOf("public"), text },
};
const bankRequests = new Set<GameplayUiIntent["kind"]>([
  "bank_placeholder", "bank_select_tab", "bank_create_tab", "bank_move", "bank_collapse_tab", "bank_set_insert",
  "bank_set_placeholders", "bank_release_placeholder", "bank_deposit_equipment", "bank_withdraw_entry", "bank_set_options",
  "bank_set_amount", "recovery_bank_all",
]);
export function requiresBankRevision(intent: GameplayUiIntent): boolean { return bankRequests.has(intent.kind); }
export function bankRevisionForIntent(ui: GameplayUiView, intent: GameplayUiIntent): string | undefined {
  return intent.kind === "recovery_bank_all" ? ui.recovery?.management?.bankRevision : ui.bank?.revision;
}
export function bindBankRevision(intent: GameplayUiIntent, revision: string): GameplayUiIntent {
  return requiresBankRevision(intent) && !Object.hasOwn(intent, "expected_bank_revision")
    ? { ...intent, expected_bank_revision: revision } : intent;
}
export function bankDefaultAmount(bank: NonNullable<GameplayUiView["bank"]>): number | "all" {
  return bank.amountSelection?.kind === "all" ? "all"
    : bank.amountSelection?.quantity ?? bank.amount;
}
export function bankAmountRequest(bank: NonNullable<GameplayUiView["bank"]>, selected: UiAmount,
  noted = bank.noted): GameplayUiIntent {
  return bank.amountSelection === undefined && selected.kind === "quantity"
    ? { kind: "bank_set_options", amount: selected.quantity, noted }
    : { kind: "bank_set_amount", amount: selected, noted };
}
const intentChecks = new Map(Object.entries(intentFields).map(([kind, fields]) => [kind, object({
  kind: oneOf(kind), ...fields, expected_bank_revision: optional(decimal()),
}, true)]));
const intentCheck: Validator = (value, path) => {
  const check = isRecord(value) && typeof value.kind === "string" ? intentChecks.get(value.kind) : undefined;
  return check ? check(value, path) : `${path}: unknown game.ui.v1 request`;
};
const projectionCheck = object({
  version: oneOf(1), activeTab: nullable(identity), activeInterface: nullable(identity),
  production: nullable(object({
    id: identity, interface: identity, target: nullable(target),
    recipes: array(object({ recipe: identity, name: text, outputs: array(item), single: permission, makeX: permission, all: optional(permission) }), "recipe"),
  })),
  reward: nullable(object({
    id: identity, kind: oneOf("quest", "level_up"), interface: identity, title: text, lines: array(text),
    items: array(item), xp: array(object({ skill: identity, amountTenths: decimal() })),
    questPoints: u32, quest: nullable(identity), skill: nullable(identity), level: nullable(integer(0, 65535)), continuation: intentCheck,
  })),
  confirmation: nullable(object({ id: identity, kind: identity, title: text, lines: array(text), items: array(item), credit: nullable(decimal()) })),
  document: nullable(object({
    id: identity, interface: identity, title: text, pages: array(text), page: integer(0, 65535),
    mapAsset: nullable(identity), nativeMap: bool,
  })),
  interfaces: array(object({ interface: identity, visibility: oneOf("hidden", "locked", "enabled"), highlighted: bool, permission }), "interface"),
  combatStyle: nullable(identity), combatStyles: array(ability, "id"), prayers: array(ability, "id"), spells: array(ability, "id"),
  equipment: object({
    bonuses: object({
      attack: dictionary(integer(-2147483648, 2147483647)), defence: dictionary(integer(-2147483648, 2147483647)),
      meleeStrength: integer(-2147483648, 2147483647), rangedStrength: integer(-2147483648, 2147483647),
      magicDamagePercent: integer(-2147483648, 2147483647), prayer: integer(-2147483648, 2147483647),
    }),
    weightGrams: decimal(true), slots: array(identity),
  }),
  inventoryActions: array(inventoryActions, "slot"),
  bank: nullable(object({
    revision: decimal(), capacity: integer(0, 65535), selectedTab: integer(0, 255),
    insertMode: bool, placeholders: bool, amount: u32, amountSelection: optional(amount), noted: bool,
    tabs: array(object({ tab: integer(0, 255), firstEntry: nullable(identity), entries: u32 }), "tab"),
    entries: array(object({ id: identity, slot: integer(0, 65535), tab: integer(0, 255), item: identity, value: nullable(item), placeholder: bool }), "id"),
    depositEquipment: permission, unavailableContainers: array(object({ id: identity, label: text, permission }), "id"),
  })),
  keptOnDeath: nullable(object({
    scope: oneOf("normal_unsafe_non_pvp"), kept: array(item), lost: array(item),
    fullGraveFee: decimal(), fullOfficeFee: decimal(), valueRevision: decimal(),
  })),
  recovery: nullable(object({
    cofferBalance: decimal(), discard: permission, cofferOffer: permission, cofferItems: array(inventoryActions, "slot"),
    management: optional(object({
      bankRevision: decimal(),
      panels: array(object({
        death: identity, storage,
        entries: array(object({
          id: identity, item, unitFee: decimal(), fullStackFee: decimal(),
          inventoryCapacity: u32, bankCapacity: u32, take: permission, bank: permission,
        }), "id"),
        fullSelectionFee: decimal(), takeAll: permission,
      })),
      bankAll: permission, bankAllRecords: array(recoveryRecord, "death"),
    })),
  })),
  appearance: object({
    choices: dictionary(array(object({ value: u32, label: nullable(text), permission }), "value")),
    base: nullable(object({ asset: identity, sourceNpc: u32, adaptation: identity })), confirmed: bool,
  }),
  publicChat: object({
    permission, maximumBytes: integer(0, 65535), channel: oneOf("public"),
    messages: array(object({ id: identity, actor: identity, sender: text, channel: oneOf("public"), text, colour: integer(0, 255), effect: integer(0, 255) }), "id"),
  }),
} satisfies Record<keyof GameplayUiView, Validator>);

const validatedProjections = new WeakMap<GameplayUiView, UiContractProblem | null>();

export function gameplayUiProblem(world: WorldView): UiContractProblem | null {
  const ui = world.ui;
  if (!ui || ui.version !== 1)
    return { message: `This server does not provide ${GAMEPLAY_UI_CAPABILITY}. Required gameplay interfaces are unsupported.`, code: "ui.capability.game.ui.v1" };
  if (Object.isFrozen(ui)) {
    const known = validatedProjections.get(ui);
    if (known !== undefined) return known;
  }
  const problem = validateProjection(ui);
  if (Object.isFrozen(ui)) validatedProjections.set(ui, problem);
  return problem;
}

function validateProjection(ui: GameplayUiView): UiContractProblem | null {
  const invalid = (detail: string): UiContractProblem => ({
    message: `Invalid ${GAMEPLAY_UI_CAPABILITY} projection: ${detail}.`, code: "ui.projection.invalid",
  });
  const problem = projectionCheck(ui, "ui");
  if (problem) return invalid(problem);
  if (ui.document && !ui.document.nativeMap && ui.document.page >= ui.document.pages.length)
    return invalid("ui.document.page: outside the supplied page list");
  if (ui.bank) {
    const slots = new Set<number>();
    for (const entry of ui.bank.entries) {
      if (slots.has(entry.slot)) return invalid(`ui.bank.entries: duplicate source slot ${entry.slot}`);
      slots.add(entry.slot);
      if (entry.placeholder !== (entry.value === null) || entry.value && entry.value.id !== entry.item)
        return invalid(`ui.bank.entries[${entry.id}]: inconsistent placeholder or canonical item identity`);
    }
  }
  const management = ui.recovery?.management;
  if (management) {
    const panels = new Set<string>();
    for (const panel of management.panels) {
      const key = JSON.stringify([panel.death, panel.storage]);
      if (panels.has(key)) return invalid("ui.recovery.management.panels: duplicate death/storage identity");
      panels.add(key);
    }
    for (const record of management.bankAllRecords) {
      if (record.items.length === 0 || new Set(record.items).size !== record.items.length)
        return invalid("ui.recovery.management.bankAllRecords: empty or duplicate item identities");
    }
  }
  return null;
}

export function gameplayUi(world: WorldView): GameplayUiView | null {
  return gameplayUiProblem(world) ? null : world.ui!;
}

export function permissionReason(permission: UiPermission | undefined, label: string): string | undefined {
  if (!permission) return `${label}: authoritative permission is unavailable.`;
  if (permission.allowed) return undefined;
  return `${permission.reason ?? `${label} is not permitted.`}${permission.code ? ` (${permission.code})` : ""}`;
}

export function formatUiInteger(value: string): string {
  if (!/^-?\d+$/.test(value)) throw new Error("Invalid authoritative integer text.");
  return BigInt(value).toLocaleString("en-US");
}

export function formatUiFixed(value: string, decimals: number): string {
  if (!/^-?\d+$/.test(value)) throw new Error("Invalid authoritative fixed-point value.");
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 9) throw new Error("Invalid fixed-point precision.");
  const raw = BigInt(value), negative = raw < 0n, magnitude = negative ? -raw : raw;
  const scale = 10n ** BigInt(decimals), fraction = String(magnitude % scale).padStart(decimals, "0").replace(/0+$/, "");
  return `${negative ? "-" : ""}${(magnitude / scale).toLocaleString("en-US")}${fraction ? "." + fraction : ""}`;
}

export function isGameplayUiIntent(intent: GameIntent): intent is GameplayUiIntent { return intentChecks.has(intent.kind); }

export function inventoryIdentity(world: WorldView, slot: number, item: string, instance: string | null): ItemView | null {
  const value = world.player.inventory.find(row => row.index === slot)?.item;
  return value?.id === item && value.instanceId === instance ? value : null;
}

function denied(permission: UiPermission | undefined, label: string): UiContractProblem | null {
  const message = permissionReason(permission, label);
  return message ? { message, code: permission?.code ?? "ui.permission.unavailable" } : null;
}

/** Validate only published identities/permissions, never implement gameplay outcomes or guards. */
export function checkUiIntent(world: WorldView, intent: GameplayUiIntent): UiContractProblem | null {
  const unsupported = gameplayUiProblem(world);
  if (unsupported) return unsupported;
  const ui = world.ui!;
  const stale = (message: string): UiContractProblem => ({ message, code: "ui.identity.stale" });
  const invalid = intentCheck(intent, "request");
  if (invalid) return { message: invalid, code: "ui.request.invalid" };
  if (requiresBankRevision(intent)) {
    const revision = bankRevisionForIntent(ui, intent);
    if (revision === undefined) return stale("The source bank or recovery context has closed.");
    if (intent.expected_bank_revision === undefined)
      return { message: "The displayed bank revision is required for this request.", code: "ui.bank.revision.required" };
    if (intent.expected_bank_revision !== revision)
      return { message: "The displayed bank changed. Review its current entries and choose again.", code: "ui.bank.revision.stale" };
  }
  switch (intent.kind) {
    case "item_action": {
      if (!inventoryIdentity(world, intent.inventory_slot, intent.expected_item, intent.expected_instance))
        return stale("That inventory item or instance changed. Choose it again.");
      const entry = ui.inventoryActions.find(row => row.slot === intent.inventory_slot && row.item === intent.expected_item && row.instance === intent.expected_instance);
      return denied(entry?.actions.find(action => action.id === intent.action)?.permission, "Item action");
    }
    case "production_select": {
      const menu = ui.production;
      if (!menu || menu.id !== intent.menu_id) return stale("That production menu has changed or closed.");
      const choice = menu.recipes.find(row => row.recipe === intent.recipe);
      return denied(intent.mode === "single" ? choice?.single : choice?.makeX, "Production");
    }
    case "production_select_all": {
      const menu = ui.production;
      if (!menu || menu.id !== intent.menu_id) return stale("That production menu has changed or closed.");
      return denied(menu.recipes.find(row => row.recipe === intent.recipe)?.all, "Make-All");
    }
    case "ui_dismiss":
      return ui.reward?.id === intent.presentation_id || ui.document?.id === intent.presentation_id
        ? null : stale("That presentation has changed or closed.");
    case "ui_document_page": {
      const document = ui.document;
      if (!document || document.id !== intent.document_id) return stale("That document has changed or closed.");
      if (document.nativeMap || intent.page >= document.pages.length)
        return { message: "That page is not supplied by the current document.", code: "ui.document.page.invalid" };
      return null;
    }
    case "ui_confirm":
      return ui.confirmation?.id === intent.confirmation_id ? null : stale("That confirmation has changed or closed.");
    case "bank_placeholder":
    case "bank_withdraw_entry": {
      const entry = ui.bank?.entries.find(row => row.id === intent.entry_id);
      return entry?.value && !entry.placeholder ? null : stale("That bank entry is no longer withdrawable.");
    }
    case "bank_create_tab":
    case "bank_move":
    case "bank_release_placeholder": {
      const entry = ui.bank?.entries.find(row => row.id === intent.entry_id);
      if (!entry) return stale("That bank entry has changed or disappeared.");
      if (intent.kind === "bank_release_placeholder" && !entry.placeholder) return stale("That entry is no longer a placeholder.");
      if (intent.kind === "bank_move") {
        if (!ui.bank!.tabs.some(tab => tab.tab === intent.tab)) return stale("The destination bank tab has disappeared.");
        if (intent.before_entry_id !== null && !ui.bank!.entries.some(row => row.id === intent.before_entry_id && row.tab === intent.tab))
          return stale("The destination bank entry changed tabs or disappeared.");
      }
      return null;
    }
    case "bank_select_tab":
    case "bank_collapse_tab":
      return ui.bank?.tabs.some(tab => tab.tab === intent.tab) ? null : stale("That bank tab has changed or disappeared.");
    case "bank_deposit_equipment":
      return denied(ui.bank?.depositEquipment, "Deposit worn items");
    case "bank_set_options":
      if (!Number.isInteger(intent.amount) || intent.amount < 1 || intent.amount > 4294967295)
        return { message: "A legacy bank amount must be a positive literal quantity, never an All sentinel.", code: "ui.bank.amount.unsupported" };
      return ui.bank ? null : stale("The bank is closed.");
    case "bank_set_amount":
      return ui.bank?.amountSelection ? null
        : { message: "The current bank has not supplied semantic quantity/All support.", code: "ui.bank.amount.unsupported" };
    case "bank_set_insert":
    case "bank_set_placeholders":
      return ui.bank ? null : stale("The bank is closed.");
    case "recovery_take": {
      const panel = ui.recovery?.management?.panels.find(row =>
        row.death === intent.death && row.storage === intent.storage);
      if (!panel || intent.items.length === 0 || new Set(intent.items.map(row => row.id)).size !== intent.items.length)
        return stale("Those recovery identities changed. Review the current source panel.");
      for (const selected of intent.items) {
        const entry = panel.entries.find(row => row.id === selected.id);
        if (!entry) return stale("That recovery item is not in the current source panel.");
      }
      if (intent.items.length === panel.entries.length && intent.items.every(row => row.amount.kind === "all"))
        return denied(panel.takeAll, "Take-All");
      for (const selected of intent.items) {
        const entry = panel.entries.find(row => row.id === selected.id)!;
        const problem = denied(entry.take, "Retrieve item");
        if (problem) return problem;
      }
      return null;
    }
    case "recovery_bank_all": {
      const management = ui.recovery?.management;
      if (!management) return stale("The source recovery context has closed.");
      const permission = denied(management.bankAll, "Bank-All");
      if (permission) return permission;
      if (intent.records.length === 0 || new Set(intent.records.map(row => row.death)).size !== intent.records.length)
        return stale("Those Bank-All records changed. Review the current source selection.");
      for (const record of intent.records) {
        const actual = management.bankAllRecords.find(row => row.death === record.death);
        if (!actual || record.items.length === 0 || new Set(record.items).size !== record.items.length ||
            record.items.some(id => !actual.items.includes(id)))
          return stale("Those Bank-All item identities are not in the supplied source selection.");
      }
      return null;
    }
    case "request_recovery_discard": {
      const recovery = world.recovery;
      if (!recovery || recovery.death !== intent.death || recovery.storage !== intent.storage ||
          intent.items.length === 0 || new Set(intent.items).size !== intent.items.length ||
          intent.items.some(id => !recovery.items.some(row => row.id === id))) return stale("Those recovery identities changed. Review the current items.");
      return denied(ui.recovery?.discard, "Discard recovery items");
    }
    case "coffer_offer": {
      if (!inventoryIdentity(world, intent.inventory_slot, intent.expected_item, intent.expected_instance))
        return stale("The offered item or instance changed. Choose it again.");
      const permission = denied(ui.recovery?.cofferOffer, "Coffer offer");
      if (permission) return permission;
      const entry = ui.recovery?.cofferItems.find(row => row.slot === intent.inventory_slot &&
        row.item === intent.expected_item && row.instance === intent.expected_instance);
      return entry?.actions.some(action => action.permission.allowed) ? null : denied(entry?.actions[0]?.permission, "Offer this item");
    }
    case "public_chat": {
      const permission = denied(ui.publicChat.permission, "Public chat");
      if (permission) return permission;
      if (intent.channel !== ui.publicChat.channel || !intent.text.trim() ||
          new TextEncoder().encode(intent.text).length > ui.publicChat.maximumBytes)
        return { message: `Public chat must use the declared channel and at most ${ui.publicChat.maximumBytes} UTF-8 bytes.`, code: "ui.chat.invalid" };
      return null;
    }
    case "open_death_preview":
      return denied(ui.interfaces.find(row => row.interface === "interface.items_kept_on_death" && row.visibility === "enabled")?.permission, "Items kept on death");
  }
}
