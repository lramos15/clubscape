export const SOURCE_PACK_SHA256 = "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d";
export const BENCHMARK_CONTRACT_SHA256 = "bf7240b4729c9444db9e3baddedfcf0f2f4655e4695f2703cfb643376fc2523e";

export interface Tile { x: number; y: number; plane: number }
export type WorldTarget =
  | { kind: "spawn"; spawn: string }
  | { kind: "temporary_object"; object: string };
export type ItemTarget =
  | { kind: "inventory"; slot: number }
  | { kind: "world"; spawn: string }
  | { kind: "temporary_object"; object: string }
  | { kind: "ground"; ground_item_id: string };
export type Setting =
  | { setting: "run"; enabled: boolean }
  | { setting: "auto_retaliate"; enabled: boolean }
  | { setting: "death_auto_equip"; enabled: boolean }
  | { setting: "death_supply_piles"; enabled: boolean };
export type GameIntent =
  | GameplayUiIntent
  | { kind: "walk"; destination: Tile; running: boolean }
  | { kind: "interact"; target: string; action: string }
  | { kind: "interact_with"; target: WorldTarget; action: string }
  | { kind: "select_dialogue"; speaker: string; choice: string }
  | { kind: "open_interface"; interface: string }
  | { kind: "close_interface" }
  | { kind: "equip"; inventory_slot: number }
  | { kind: "unequip"; slot: string }
  | { kind: "drop"; inventory_slot: number; quantity: number }
  | { kind: "take_ground_item"; ground_item_id: string }
  | { kind: "use_item"; inventory_slot: number; target: ItemTarget }
  | { kind: "move_inventory"; from: number; to: number }
  | { kind: "eat"; inventory_slot: number }
  | { kind: "produce"; recipe: string; target: string | null; quantity: number }
  | { kind: "produce_at"; recipe: string; target: WorldTarget | null; quantity: number }
  | { kind: "produce_selected"; recipe: string; target: WorldTarget | null; quantity: number; mode: "single" | "make_x" }
  | { kind: "bank_deposit"; banker: string; inventory_slot: number; quantity: number }
  | { kind: "bank_withdraw"; banker: string; bank_slot: number; quantity: number; noted: boolean }
  | { kind: "shop_buy"; shop: string; item_index: number; quantity: number; expected_item?: string }
  | { kind: "shop_sell"; shop: string; inventory_slot: number; quantity: number }
  | { kind: "set_combat_style"; style: string }
  | { kind: "cast"; spell: string; target: string | null }
  | { kind: "set_prayer"; prayer: string; enabled: boolean }
  | { kind: "set_setting"; setting: Setting }
  | { kind: "confirm_appearance"; appearance: Record<string, number> }
  | { kind: "select_experience"; experience: string }
  | { kind: "open_grave"; death: string }
  | { kind: "open_death_office" }
  | { kind: "reclaim"; death: string; storage: "grave" | "death_office"; items: string[] }
  | { kind: "cancel_activity" }
  | { kind: "request_logout" };

export interface ItemView {
  id: string;
  name: string;
  quantity: number;
  sourceId: number | null;
  iconAsset: string | null;
  instanceId: string | null;
  charges: number | null;
  actions: string[];
}
export interface SlotView { index: number; item: ItemView | null }
export interface SkillView { id: string; name: string; xpTenths: string; baseLevel: number; currentLevel: number; iconAsset: string | null }
export interface QuestView { id: string; name: string; stage: string; journal: string; completed: boolean }
export interface PlayerView {
  id: string;
  displayName: string;
  appearance: Record<string, number>;
  region: string;
  tile: Tile;
  instance: string | null;
  inventory: SlotView[];
  equipment: Array<{ slot: string; item: ItemView | null }>;
  skills: SkillView[];
  hitpoints: number;
  prayerPoints: number;
  runEnergy: number;
  questPoints: number;
  tutorialStage: string;
  tutorialInstruction: string;
  quests: QuestView[];
  unlockedInterfaces: string[];
  activePrayers: string[];
  activity: string;
  animation: string;
  settings: Setting[];
}
export interface EntityView {
  id: string;
  definitionId: string;
  sourceId: number | null;
  name: string;
  kind: "npc" | "object" | "player" | "temporary_object";
  tile: Tile;
  instance: string | null;
  hitpoints: number;
  maxHitpoints: number;
  available: boolean;
  animation: string;
  actions: Array<{ name: string; allowed: boolean; reason: string | null }>;
  appearance: Record<string, number>;
  equipment: Array<{ slot: string; sourceId: number | null }>;
}
export interface DialogueView {
  id: string;
  speaker: string;
  speakerName: string;
  portraitAsset: string | null;
  text: string;
  choices: Array<{ id: string; text: string }>;
}
export interface BankView { banker: string; capacity: number; slots: SlotView[]; allowNotes: boolean }
export interface ShopView {
  id: string;
  name: string;
  rows: Array<{ index: number; item: ItemView; stock: number; buyPrice: number | null; sellPrice: number | null }>;
}
export interface RecoveryView {
  death: string;
  storage: "grave" | "death_office";
  items: Array<{ id: string; item: ItemView; cost: number | null }>;
  remainingTicks: number | null;
}
export interface AudioEvent {
  id: string;
  kind: "sound" | "animation" | "music" | "jingle" | "quest_complete" | "level_up" | "interface_closed";
  sourceId: number | null;
  assetId: string | null;
  actorId: string | null;
  tile: Tile | null;
  sourceCycle: number | null;
  payload: Readonly<Record<string, string | number | boolean>>;
}
export interface WorldView {
  /** Absent only on older unsupported servers. game.ui.v1 requires version 1. */
  ui?: GameplayUiView;
  revision: string;
  tick: string;
  player: PlayerView;
  entities: EntityView[];
  groundItems: Array<{ id: string; tile: Tile; item: ItemView; canTake: boolean }>;
  dialogue: DialogueView | null;
  bank: BankView | null;
  shop: ShopView | null;
  recovery: RecoveryView | null;
  messages: Array<{ id: string; text: string; channel: string }>;
}

export const GAMEPLAY_UI_CAPABILITY = "game.ui.v1";
export type GameplayUiIntent =
  | { kind: "ui_dismiss"; presentation_id: string }
  | { kind: "production_select"; menu_id: string; recipe: string; quantity: number; mode: "single" | "make_x" }
  | { kind: "item_action"; inventory_slot: number; expected_item: string; expected_instance: string | null; action: string }
  | { kind: "bank_select_tab"; tab: number }
  | { kind: "bank_create_tab"; entry_id: string }
  | { kind: "bank_move"; entry_id: string; before_entry_id: string | null; tab: number }
  | { kind: "bank_collapse_tab"; tab: number }
  | { kind: "bank_set_insert"; enabled: boolean }
  | { kind: "bank_set_placeholders"; enabled: boolean }
  | { kind: "bank_release_placeholder"; entry_id: string }
  | { kind: "bank_deposit_equipment" }
  | { kind: "bank_withdraw_entry"; entry_id: string; quantity: number; noted: boolean }
  | { kind: "bank_set_options"; amount: number; noted: boolean }
  | { kind: "open_death_preview" }
  | { kind: "request_recovery_discard"; death: string; storage: "grave" | "death_office"; items: string[] }
  | { kind: "coffer_offer"; inventory_slot: number; expected_item: string; expected_instance: string | null; quantity: number }
  | { kind: "ui_confirm"; confirmation_id: string; accept: boolean }
  | { kind: "public_chat"; channel: "public"; text: string };
export interface UiPermission { allowed: boolean; code: string | null; reason: string | null }
export interface AbilityUiView { id: string; name: string; selected: boolean; visible: boolean; permission: UiPermission }
export interface InventoryActionsUiView {
  slot: number; item: string; instance: string | null;
  actions: Array<{ id: string; label: string; permission: UiPermission }>;
}
export interface GameplayUiView {
  version: 1;
  activeInterface: string | null;
  production: {
    id: string; interface: string; target: WorldTarget;
    recipes: Array<{ recipe: string; name: string; outputs: ItemView[]; single: UiPermission; makeX: UiPermission }>;
  } | null;
  reward: {
    id: string; kind: "quest" | "level_up"; interface: string; title: string; lines: string[]; items: ItemView[];
    xp: Array<{ skill: string; amountTenths: string }>; questPoints: number; quest: string | null;
    skill: string | null; level: number | null; continuation: GameplayUiIntent;
  } | null;
  confirmation: { id: string; kind: string; title: string; lines: string[]; items: ItemView[]; credit: string | null } | null;
  interfaces: Array<{ interface: string; visibility: "hidden" | "locked" | "enabled"; highlighted: boolean; permission: UiPermission }>;
  combatStyle: string | null;
  combatStyles: AbilityUiView[];
  prayers: AbilityUiView[];
  spells: AbilityUiView[];
  equipment: {
    bonuses: { attack: Record<string, number>; defence: Record<string, number>; meleeStrength: number; rangedStrength: number; magicDamagePercent: number; prayer: number };
    weightGrams: string; slots: string[];
  };
  inventoryActions: InventoryActionsUiView[];
  bank: {
    revision: string; capacity: number; selectedTab: number; insertMode: boolean; placeholders: boolean; amount: number; noted: boolean;
    tabs: Array<{ tab: number; firstEntry: string | null; entries: number }>;
    entries: Array<{ id: string; slot: number; tab: number; item: string; value: ItemView | null; placeholder: boolean }>;
    depositEquipment: UiPermission;
    unavailableContainers: Array<{ id: string; label: string; permission: UiPermission }>;
  } | null;
  keptOnDeath: { scope: "normal_unsafe_non_pvp"; kept: ItemView[]; lost: ItemView[]; fullGraveFee: string; fullOfficeFee: string; valueRevision: string } | null;
  recovery: { cofferBalance: string; discard: UiPermission; cofferOffer: UiPermission; cofferItems: InventoryActionsUiView[] } | null;
  appearance: {
    choices: Record<string, Array<{ value: number; label: string | null; permission: UiPermission }>>;
    base: { asset: string; sourceNpc: number; adaptation: string } | null; confirmed: boolean;
  };
  publicChat: {
    permission: UiPermission; maximumBytes: number; channel: "public";
    messages: Array<{ id: string; actor: string; sender: string; channel: "public"; text: string; colour: number; effect: number }>;
  };
}
export interface AppState {
  phase: "capability_check" | "title" | "register" | "login" | "connecting" | "character" | "world" | "reconnecting" | "error";
  loading: { completed: number; total: number; label: string } | null;
  accountName: string | null;
  error: { message: string; errorId: string | null; recoverable: boolean } | null;
  world: WorldView | null;
  soundEnabled: boolean;
}
export interface ClientAssets {
  readonly baseUrl: string;
  url(assetId: string): string;
  image(assetId: string): Promise<HTMLImageElement>;
  json(assetId: string): Promise<unknown>;
}
export interface AppServices {
  state(): Readonly<AppState>;
  subscribe(listener: (state: Readonly<AppState>) => void): () => void;
  register(loginName: string, password: string): Promise<void>;
  login(loginName: string, password: string): Promise<void>;
  logout(): Promise<void>;
  createCharacter(appearance: Record<string, number>): Promise<void>;
  enterWorld(): Promise<void>;
  send(intent: GameIntent): Promise<void>;
  setScreen(screen: "title" | "register" | "login"): void;
  unlockAudio(): Promise<void>;
  audioVolume(channel: "music" | "effects" | "area", value: number): void;
  report(error: Error, errorId?: string): void;
}
export interface UiHandle {
  update(state: Readonly<AppState>): void;
  resize(width: number, height: number): void;
  dispose(): void;
  capturesPointer(x: number, y: number): boolean;
}
export type CreateUi = (canvas: HTMLCanvasElement, services: AppServices, assets: ClientAssets) => Promise<UiHandle>;

export interface AudioHandle {
  unlock(): Promise<void>;
  update(world: WorldView | null, events: readonly AudioEvent[]): void;
  volume(channel: "music" | "effects" | "area", value: number): void;
  mute(value: boolean): void;
  disconnected(): void;
  dispose(): Promise<void>;
}
export type CreateAudio = (assets: ClientAssets, report: (error: Error) => void) => Promise<AudioHandle>;

export interface RenderCamera {
  x: number; height: number; y: number;
  pitch: number; yaw: number; unitsPerTurn: 16384;
  zoom: number; near: number; far: number;
}
export interface RenderFrame {
  sequence: number;
  submittedAtMs: number;
  completedAtMs: number;
  drawCalls: number;
  primitives: number;
  cpuEncodeMs?: number;
  gpuDurationMs?: number;
}
export type ScenePick = { kind: "tile"; tile: Tile } | { kind: "entity"; id: string; tile: Tile };
export interface RendererHandle {
  resize(width: number, height: number): void;
  loadScene(sceneId: string): Promise<void>;
  update(world: WorldView): void;
  camera(value: RenderCamera): void;
  frame(nowMs: number): Promise<RenderFrame | null>;
  pick(x: number, y: number): ScenePick | null;
  dispose(): void;
}
export interface RendererConfig {
  assetBaseUrl: string;
  manifestUrl: string;
  sourcePackSha256: string;
  width: number;
  height: number;
}
export type CreateRenderer = (canvas: HTMLCanvasElement, config: RendererConfig) => Promise<RendererHandle>;
