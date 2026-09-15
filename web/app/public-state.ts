import type { BankView, EntityView, GameIntent, PlayerView, RecoveryView, ShopView, WorldView } from "../shared/contracts.ts";

export interface SourceDenial { code: number; name: string; message: string }
export interface SourcePermission { allowed: boolean | null; denial: SourceDenial | null }
export interface SourcePresence {
  kind: "connected" | "disconnecting" | "offline";
  connected: boolean;
  acceptsInput: boolean;
  presentInWorld: boolean;
}
export interface PublicRecovery extends RecoveryView {
  interfaceId: string | null;
  items: Array<RecoveryView["items"][number] & {
    cost: null;
    fullEntryFee: string;
    storage: "grave" | "death_office";
    layout: { kind: "inventory"; slot: number } | { kind: "equipment"; slot: string };
  }>;
}
export interface PublicWorld extends WorldView {
  player: PlayerView & { presence: SourcePresence | null; appearanceConfirmed: boolean };
  entities: Array<EntityView & {
    assetId: string | null; width: number; height: number; presence: SourcePresence | null;
    actions: Array<EntityView["actions"][number] & { denial: SourceDenial | null }>;
  }>;
  bank: (BankView & { interfaceId: string | null; deposit: SourcePermission; withdraw: SourcePermission }) | null;
  shop: (ShopView & {
    currency: string; interfaceId: string | null;
    rows: Array<ShopView["rows"][number] & { itemId: string }>;
  }) | null;
  recovery: PublicRecovery | null;
  recoveryContext: { views: PublicRecovery[] } | null;
}

export type ShopPurchaseIntent = Extract<GameIntent, { kind: "shop_buy" }> & { expected_item: string };
export type QuoteRequest =
  | { kind: "bank_deposit"; inventorySlot: number; quantity: number }
  | { kind: "bank_withdraw"; bankSlot: number; quantity: number; noted: boolean }
  | { kind: "shop_buy"; shop: string; itemIndex: number; expected_item: string; quantity: number }
  | { kind: "shop_sell"; shop: string; inventorySlot: number; itemId: string; quantity: number }
  | { kind: "recovery"; death: string; storage: "grave" | "death_office"; items: string[] };
export type QuoteView = { revision: string; tick: string } & (
  | { kind: "bank"; requested: number; transferred: WorldView["player"]["inventory"][number]["item"] }
  | { kind: "shop"; shop: string; itemId: string; requested: number; quantity: number;
    currency: string; totalPrice: number; stockAfter: number; partialReason: SourceDenial | null }
  | { kind: "recovery"; death: string; storage: "grave" | "death_office"; selected: string[]; fullSelectionFee: string }
);

export function presenceOf(world: WorldView | null): SourcePresence | null {
  return (world as PublicWorld | null)?.player.presence ?? null;
}
