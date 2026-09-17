import assert from "node:assert/strict";
import { test } from "node:test";
import type { AudioAuthorityView } from "../../shared/contracts.ts";
import type { SourceAudioScene } from "../../audio/index.ts";
import { authoritativeMusicUnlocks, calibratedNativeVarpBits, sourceSceneAuthority, validateAudioAuthority, validateScene, validateWorldAuthority } from "../authority.ts";
import { audioFixtureWorld } from "./player-audio-fixture.ts";

function authority(): AudioAuthorityView {
  return {
    version: 1, profile: "source.audio.fixture",
    music: {
      history: "legacy_untracked", trackedFromTick: "9007199254740993", revision: "18446744073709551615",
      complete: false, unlockedGroups: [62], tracks: [
        { group: 62, status: "unlocked", confirmedAtTick: "9007199254740993", rule: "music.62.source" },
        { group: 76, status: "unknown", confirmedAtTick: null, rule: null },
      ],
    },
    varps: [{ id: 491, value: 0, knownBits: 20, binding: "source491.bits2and4", unavailableReason: null }],
  };
}

test("legacy history stays unknown with exact strings and cannot become a locked/native preference projection", () => {
  const value = authority(), world = { ...audioFixtureWorld(), audioAuthority: value };
  const before = JSON.stringify(value);
  validateAudioAuthority(value);
  assert.throws(() => authoritativeMusicUnlocks(world), /history is partially unknown/);
  assert.equal(JSON.stringify(value), before);
  assert.equal(value.music.revision, "18446744073709551615");
  assert.equal(value.music.tracks[1]!.status, "unknown");
  const complete = authority();
  complete.music.history = "from_creation";
  complete.music.complete = true;
  complete.music.tracks[1]!.status = "locked";
  assert.deepEqual(authoritativeMusicUnlocks({ ...world, audioAuthority: complete }), [62]);
});

test("audio authority requires its capability and coherent track confirmations, not all-unlocked defaults", () => {
  const world = audioFixtureWorld(), value = authority();
  assert.throws(() => validateWorldAuthority(["game.audio.authority.v1"], world), /capability\/version/);
  assert.throws(() => validateWorldAuthority([], { ...world, audioAuthority: value }), /capability\/version/);
  const current = { ...world, audioAuthority: value, scene: { region: world.player.region, instance: null, instanceTemplate: null } };
  validateWorldAuthority(["game.audio.authority.v1"], current);
  value.music.complete = true;
  assert.throws(() => validateAudioAuthority(value), /completeness/);
  value.music.complete = false;
  value.music.unlockedGroups = [62, 76];
  assert.throws(() => validateAudioAuthority(value), /unlock identities/);
  value.music.unlockedGroups = [62];
  value.music.tracks[1]!.status = "locked";
  assert.throws(() => validateAudioAuthority(value), /Legacy unknown/);
});

test("knownBits20 of zero491 supplies only the calibrated bits2/4, never a complete zero word", () => {
  const value = authority(), before = JSON.stringify(value);
  assert.deepEqual(calibratedNativeVarpBits(value, 491, 4),
    { id: 491, value: 0, knownBits: 4, binding: "source491.bits2and4" });
  assert.equal(calibratedNativeVarpBits(value, 491, 20).knownBits, 20);
  assert.throws(() => calibratedNativeVarpBits(value, 491, 1), /no published audio calibration/);
  assert.throws(() => calibratedNativeVarpBits(value, 491, 0xffffffff), /no published audio calibration/);
  assert.equal(JSON.stringify(value), before);
  value.varps = [{ id: 491, value: null, knownBits: 0, binding: "source491.bits2and4", unavailableReason: "No bound case" }];
  validateAudioAuthority(value);
  assert.throws(() => calibratedNativeVarpBits(value, 491, 4), /not supplied all calibrated bits/);
});

test("actual source scene consumers receive only their calibrated authoritative varp subset", () => {
  const world = { ...audioFixtureWorld(), audioAuthority: authority() };
  const scene: SourceAudioScene = {
    listener: { x: 3094 * 128 + 11, y: 3107 * 128 + 37 }, plane: 0, instance: null, owner: null,
    varps: new Map([[491, -1], [999, 123]]),
    emitters: [{ id: "placed.fixture", objectId: 34815, tile: world.player.tile, orientation: 0,
      instance: null, owner: null, present: true }],
  };
  const result = sourceSceneAuthority(world, scene);
  assert.deepEqual([...result.varps], [[491, 0]]);
  assert.equal(result.listener, scene.listener);
  assert.equal(result.emitters, scene.emitters);
  assert.equal(scene.varps.get(491), -1);
  assert.equal(world.audioAuthority.varps[0]!.knownBits, 20);
  assert.throws(() => sourceSceneAuthority(audioFixtureWorld(), scene), /without audio authority/);
});

test("scene fields preserve the real opaque/template pair and refuse mismatches", () => {
  const base = audioFixtureWorld();
  const world = { ...base, player: { ...base.player, instance: "instance.opaque" } };
  const scene = { region: world.player.region, instance: "instance.opaque", instanceTemplate: "instance_template.death.office" };
  validateScene(scene, world);
  assert.throws(() => validateScene({ ...scene, instanceTemplate: null }, world), /incomplete or inconsistent/);
  assert.throws(() => validateScene({ ...scene, instance: "instance.other" }, world), /incomplete or inconsistent/);
  assert.throws(() => validateScene({ ...scene, region: "region.other" }, world), /incomplete or inconsistent/);
});
