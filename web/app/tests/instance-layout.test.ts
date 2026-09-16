import assert from "node:assert/strict";
import { test } from "node:test";
import { rendererInstanceLayout, rendererWorldView, validateInstanceLayouts } from "../instance-layout.ts";
import type { SourceInstanceLayouts } from "../instance-layout.ts";
import { audioFixtureWorld } from "./player-audio-fixture.ts";
import { ACTOR_OBSERVER_CAPABILITY } from "../../shared/contracts.ts";
import type { ActorActionView, WorldView } from "../../shared/contracts.ts";
import { validateActorObservers } from "../gameplay-ui.ts";
import { deepFreeze } from "../errors.ts";

const template = "instance_template.death.office";
const region = "region.osrs.12633";
function layouts(): SourceInstanceLayouts {
  return { [template]: { chunkSize: 8, chunks: [[3168, 5720], [3168, 5728], [3176, 5720], [3176, 5728]].map(([x, y]) => ({
    sourceRegion: region, destinationRegion: region,
    sourceOrigin: { x: x!, y: y!, plane: 0 }, destinationOrigin: { x: x!, y: y!, plane: 0 }, quarterTurns: 0,
  })) } };
}
function world() {
  const value = audioFixtureWorld();
  return { ...value, player: { ...value.player, instance: "instance.opaque.fixture", region, tile: { x: 3169, y: 5721, plane: 0 } } };
}

test("unverified inventory motion remains null through immutable renderer composition, not another source action", () => {
  const dough: ActorActionView = {
    version: 1, id: "observer.fixture.dough", activity: "producing",
    actionId: "action.cooking.dough", target: null, recipeId: "recipe.cooking.dough",
    styleId: null, spellId: null, animation: null,
    startedAtTick: "9007199254740993", cycleStartedAtTick: "9007199254740994",
    nextActionTick: "9007199254740995", observedAtTick: "9007199254740994",
  };
  const cases: ActorActionView[] = [dough, ...[3008, 3010, 3012, 3014, 1933, 1927, 1929].map((item) => ({
    ...dough, id: `observer.fixture.empty.${item}`, activity: "using_item", actionId: null, recipeId: null,
  }))];
  for (const action of cases) {
    const original = audioFixtureWorld();
    const current: WorldView = deepFreeze({
      ...original, player: { ...original.player, running: false, movementTick: null, action, activity: action.activity },
    });
    const before = JSON.stringify(current);
    validateActorObservers([ACTOR_OBSERVER_CAPABILITY], current);
    const rendered = rendererWorldView(current, {});
    assert.equal(rendered.player, current.player);
    assert.equal(rendered.player.action?.id, action.id);
    assert.equal(rendered.player.action?.animation, null);
    assert.equal(rendered.player.animation, "", "No pottery, drink, milking or animation0 fallback is installed.");
    assert.equal(rendered.player.action?.nextActionTick, "9007199254740995");
    assert.equal(rendered.player.action?.recipeId, action.recipeId);
    assert.equal(JSON.stringify(current), before);
  }
});

test("only a separately supplied actual template selects compiler-validated instance chunks", () => {
  const definitions = layouts(), current = world();
  validateInstanceLayouts(definitions, new Set([region]));
  const layout = rendererInstanceLayout(current, definitions, template);
  assert.equal(layout?.template, template);
  assert.deepEqual(layout?.chunks, [
    { plane: 0, chunkX: 396, chunkY: 715, sourcePlane: 0, sourceChunkX: 396, sourceChunkY: 715, quarterTurns: 0 },
    { plane: 0, chunkX: 396, chunkY: 716, sourcePlane: 0, sourceChunkX: 396, sourceChunkY: 716, quarterTurns: 0 },
    { plane: 0, chunkX: 397, chunkY: 715, sourcePlane: 0, sourceChunkX: 397, sourceChunkY: 715, quarterTurns: 0 },
    { plane: 0, chunkX: 397, chunkY: 716, sourcePlane: 0, sourceChunkX: 397, sourceChunkY: 716, quarterTurns: 0 },
  ]);
  const rendered = rendererWorldView(current, definitions, template);
  assert.equal(rendered.player, current.player, "The authoritative snapshot is not replaced or advanced.");
  assert.equal(Object.hasOwn(current, "instanceLayout"), false);
  assert(Object.isFrozen(rendered.instanceLayout?.chunks));
  assert.equal(rendererInstanceLayout(audioFixtureWorld(), definitions, undefined), null);
});

test("opaque instance identifiers, matching region and tile never imply an unprovided template identity", () => {
  const current = world(), definitions = layouts();
  for (const id of ["instance.opaque.fixture", "instance.instance_template.death.office", "instance.death.office"]) {
    assert.throws(() => rendererInstanceLayout({ ...current, player: { ...current.player, instance: id } }, definitions, undefined),
      /not supplied.*template identity/);
  }
  assert.throws(() => rendererInstanceLayout(current, definitions, "instance_template.unpublished"), /no compiler-validated/);
  assert.throws(() => rendererInstanceLayout(audioFixtureWorld(), definitions, template), /ordinary-world/);
});

test("the real scene projection supplies the active template and cannot be overridden by an adapter", () => {
  const current = world();
  const observed = { ...current, scene: { region, instance: current.player.instance, instanceTemplate: template } };
  const rendered = rendererWorldView(observed, layouts());
  assert.equal(rendered.instanceLayout?.template, template);
  assert.equal(rendered.player.instance, "instance.opaque.fixture");
  assert.throws(() => rendererWorldView(observed, layouts(), "instance_template.other"), /cannot override/);
});

test("unsupported turns, chunk geometry and mismatched actor locations fail explicitly instead of drawing a normal scene", () => {
  const definitions = layouts(), current = world();
  definitions[template]!.chunks[0]!.quarterTurns = 1;
  validateInstanceLayouts(definitions, new Set([region]));
  assert.throws(() => rendererInstanceLayout(current, definitions, template), /Turned source/);
  definitions[template]!.chunks[0]!.quarterTurns = 0;
  definitions[template]!.chunks[0]!.destinationOrigin.x++;
  assert.throws(() => rendererInstanceLayout(current, definitions, template), /not aligned/);
  assert.throws(() => rendererInstanceLayout({ ...current, player: { ...current.player, tile: { x: 3200, y: 5750, plane: 0 } } },
    layouts(), template), /outside.*declared chunks/);
  const differentSize = layouts(); differentSize[template]!.chunkSize = 16;
  assert.throws(() => rendererInstanceLayout(current, differentSize, template), /8x8/);
});

test("public instance projection rejects unknown regions, invalid planes and duplicate destination chunks", () => {
  const definitions = layouts();
  assert.throws(() => validateInstanceLayouts(definitions, new Set()), /declared regions/);
  definitions[template]!.chunks[0]!.sourceOrigin.plane = 4;
  assert.throws(() => validateInstanceLayouts(definitions, new Set([region])), /declared regions/);
  definitions[template]!.chunks[0]!.sourceOrigin.plane = 0;
  definitions[template]!.chunks.push(definitions[template]!.chunks[0]!);
  assert.throws(() => validateInstanceLayouts(definitions, new Set([region])), /Duplicate source instance/);
});
