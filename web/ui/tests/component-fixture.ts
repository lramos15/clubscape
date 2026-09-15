import type { AppServices, AppState, GameIntent, ItemView, UiHandle, WorldView } from "../../shared/contracts.ts";
import { createUi, forwardWorldPointer, onUiCameraRequest } from "../index.ts";
import { TABS } from "../layout.ts";
import { testAssets } from "./source-fixture.ts";

export function immutable<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.freeze(value);
    for (const child of Object.values(value)) immutable(child);
  }
  return value;
}

function item(id: string, name: string, sourceId: number, quantity: number, actions: string[]): ItemView {
  return { id, name, sourceId, quantity, actions, iconAsset: null, instanceId: null, charges: null };
}

export function fixtureWorld(): WorldView {
  const inventory = Array.from({ length: 28 }, (_, index) => ({ index, item: null as ItemView | null }));
  const values = [
    item("item.pickaxe.bronze", "Bronze pickaxe", 1265, 1, ["Wield", "Drop"]),
    item("item.axe.bronze", "Bronze axe", 1351, 1, ["Wield", "Drop"]),
    item("item.shrimps", "Shrimps", 315, 2, ["Eat", "Drop"]),
    item("item.coins", "Coins", 995, 12345, ["Drop"]),
    item("item.rune.air", "Air rune", 556, 25, ["Drop"]),
    item("item.rune.mind", "Mind rune", 558, 25, ["Drop"]),
  ];
  values.forEach((value, index) => { inventory[index]!.item = value; });
  const skills = ["attack", "defence", "strength", "hitpoints", "ranged", "prayer", "magic", "cooking",
    "woodcutting", "fletching", "fishing", "firemaking", "crafting", "smithing", "mining", "herblore",
    "agility", "thieving", "slayer", "farming", "runecraft", "hunter", "construction", "sailing"]
    .map(id => ({ id: `skill.${id}`, name: id[0]!.toUpperCase() + id.slice(1), currentLevel: id === "hitpoints" ? 10 : 1,
      baseLevel: id === "hitpoints" ? 10 : 1, xpTenths: id === "hitpoints" ? "11540" : "0", iconAsset: null }));
  return {
    revision: "1", tick: "1",
    player: {
      id: "component-only", displayName: "Reference", appearance: { body_type: 0 }, region: "region.lumbridge",
      tile: { x: 3222, y: 3218, plane: 0 }, instance: null, inventory,
      equipment: [{ slot: "slot.weapon", item: item("item.sword.bronze", "Bronze sword", 1277, 1, ["Wield"]) }],
      skills, hitpoints: 10, prayerPoints: 1, runEnergy: 0, questPoints: 0, tutorialStage: "stage.tutorial.mainland",
      tutorialInstruction: "", quests: [{ id: "quest.cooks_assistant", name: "Cook's Assistant", stage: "stage.cooks.ingredients",
        journal: "<col=000080>Cook's Assistant</col><br><str>I spoke to the Cook.</str><br>I need to bring him a bucket of milk, an egg and a pot of flour.", completed: false }],
      unlockedInterfaces: TABS.map(tab => tab.interface), activePrayers: [], activity: "idle", animation: "idle",
      settings: [{ setting: "run", enabled: false }, { setting: "auto_retaliate", enabled: true }],
    },
    entities: [{ id: "spawn.cook", definitionId: "npc.cook", sourceId: 3308, name: "Cook", kind: "npc",
      tile: { x: 3223, y: 3218, plane: 0 }, instance: null, hitpoints: 10, maxHitpoints: 10, available: true,
      animation: "idle", actions: [{ name: "Talk-to", allowed: true, reason: null }, { name: "Attack", allowed: false, reason: "This NPC cannot be attacked." }],
      appearance: {}, equipment: [] }],
    groundItems: [], dialogue: null, bank: null, shop: null, recovery: null, messages: [],
  };
}

export class ComponentServices implements AppServices {
  current: Readonly<AppState>;
  readonly intents: GameIntent[] = [];
  readonly calls: Array<{ method: string; args: unknown[] }> = [];
  readonly errors: Array<{ message: string; errorId: string | null }> = [];
  readonly cameraRequests: number[] = [];
  rejection: { message: string; errorId: string } | null = null;
  private listeners = new Set<(state: Readonly<AppState>) => void>();

  constructor(phase: AppState["phase"] = "world") {
    this.current = immutable({ phase, world: phase === "world" ? fixtureWorld() : null,
      loading: null, accountName: null, error: null, soundEnabled: false });
  }
  state(): Readonly<AppState> { return this.current; }
  subscribe(listener: (state: Readonly<AppState>) => void): () => void {
    this.listeners.add(listener); return () => { this.listeners.delete(listener); };
  }
  get subscriptions(): number { return this.listeners.size; }
  publish(state: AppState): void {
    this.current = immutable(state);
    this.listeners.forEach(listener => listener(this.current));
  }
  patchWorld(patch: Partial<WorldView>): void {
    const world = { ...structuredClone(this.current.world!), ...patch };
    world.revision = String(BigInt(world.revision) + 1n);
    this.publish({ ...this.current, world });
  }
  private async accept(method: string, ...args: unknown[]): Promise<void> {
    this.calls.push({ method, args });
    if (this.rejection) {
      const details = this.rejection; this.rejection = null;
      throw Object.assign(new Error(details.message), { errorId: details.errorId });
    }
  }
  register(name: string, password: string): Promise<void> { return this.accept("register", name, password); }
  login(name: string, password: string): Promise<void> { return this.accept("login", name, password); }
  logout(): Promise<void> { return this.accept("logout"); }
  createCharacter(appearance: Record<string, number>): Promise<void> { return this.accept("createCharacter", appearance); }
  enterWorld(): Promise<void> { return this.accept("enterWorld"); }
  send(intent: GameIntent): Promise<void> { this.intents.push(structuredClone(intent)); return this.accept("send", intent); }
  setScreen(phase: "title" | "register" | "login"): void { this.calls.push({ method: "setScreen", args: [phase] }); this.publish({ ...this.current, phase, error: null }); }
  unlockAudio(): Promise<void> { return this.accept("unlockAudio"); }
  audioVolume(channel: "music" | "effects" | "area", value: number): void { this.calls.push({ method: "audioVolume", args: [channel, value] }); }
  report(error: Error, errorId?: string): void { this.errors.push({ message: error.message, errorId: errorId ?? null }); }
}

let current: { services: ComponentServices; ui: UiHandle } | null = null;
export async function mount(phase: AppState["phase"] = "world"): Promise<typeof current> {
  current?.ui.dispose();
  const canvas = document.querySelector("canvas")!;
  canvas.width = innerWidth; canvas.height = innerHeight;
  canvas.style.width = `${innerWidth}px`; canvas.style.height = `${innerHeight}px`;
  const services = new ComponentServices(phase), ui = await createUi(canvas, services, testAssets);
  onUiCameraRequest(ui, yaw => services.cameraRequests.push(yaw));
  current = { services, ui };
  Object.assign(window, { component: current, forwardWorldPointer, fixtureWorld });
  await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame);
  return current;
}
