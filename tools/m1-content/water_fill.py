"""Exact source-backed water recipe delta; never synthesize a missing object menu operation."""

from copy import deepcopy
import json
import subprocess

from common import BINDINGS, ROOT, bound, canonical, constant_chance, item_stack, load, sha, source_record, tutorial_at
from runtime_application import changed_paths


DIRECTORY = ROOT / "research/water-fill"
RECIPE = "recipe.water.bucket"
METHOD = "action.water.bucket"
SINK = "spawn.water_source.3205.3215.p0.t10.r0"
OBJECT = "object.water_source"


def semantic_hash(content):
    value = dict(content)
    value.pop("revision")
    return sha(canonical(value))


def baselines():
    document = load(DIRECTORY / "baselines.json")
    record = document["approval"]
    path = ROOT / record["path"]
    if sha(path.read_bytes()) != record["sha256"]:
        raise ValueError("Bounded water-fill approval fingerprint changed")
    approval = load(path)
    if approval["decision"] != "approved" or approval["owner_record"] != record["owner_record"]:
        raise ValueError("The exact bounded water-fill implementation authority is absent")
    return document


def freeze_inputs():
    for name, record in baselines()["profiles"].items():
        output = ROOT / record["input"]
        if output.exists():
            data = output.read_bytes()
        elif "commit" in record:
            data = subprocess.check_output(["git", "show", record["commit"] + ":" + record["repository_path"]], cwd=ROOT)
        else:
            data = (ROOT / record["public_supplied_copy"]).read_bytes()
        if sha(data) != record["source_sha256"]:
            raise ValueError("Public source-profile bytes changed: " + name)
        if not output.exists():
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)
        content = load(output)
        if content["schema_version"] != 4 or content["revision"] != record["revision"] or semantic_hash(content) != record["semantic_sha256_without_revision"]:
            raise ValueError("Public source-profile identity changed: " + name)


def baseline_content(profile):
    record = baselines()["profiles"][profile]
    path = ROOT / record["input"]
    if sha(path.read_bytes()) != record["source_sha256"]:
        raise ValueError("Frozen water baseline changed: " + profile)
    return load(path)


def recipe_definition():
    facts = load(DIRECTORY / "facts.json")
    if (facts["source_recipe_parameters"] != {"members": "No", "ticks": "1", "facilities": "Water",
                                             "mat1": "Bucket", "output1": "Bucket of water"}
            or facts["skill_requirements"] or facts["xp_rewards"] or facts["byproducts"]
            or facts["source_object_menu_operations"]):
        raise ValueError("The bounded original water rule changed")
    page, module, sink = facts["source_recipe"], facts["recipe_parameter_semantics"], facts["sink"]
    source = [
        source_record(page["url"], "Original unskilled item-on-water creation: one empty bucket1925 becomes one "
                      "bucket of water1929 in1 tick. No extra vessel, byproduct, tools or XP; not Humidify.",
                      "verified_reference", str(page["revision"])),
        source_record(module["url"], facts["quantity_and_xp_basis"], "verified_reference", str(module["revision"])),
        source_record(sink["collection"] + "#" + sink["source_asset"],
                      "Exact sink14868/model8245 with no object-menu operations; geometry and source actor-motion uncertainty are retained. "
                      "Definition SHA-256: " + sink["definition_sha256"], "verified_reference", "240/cache2695"),
    ]
    phase_source = source + [source_record("research/water-fill/facts.json", facts["cadence_qualification"],
                                           "inference", sha((DIRECTORY / "facts.json").read_bytes()))]
    target_source = source + [source_record(
        "research/water-fill/facts.json",
        "Conservative one-tile item-on contact uses the unchanged original footprint, rotation, access masks "
        "and collision/sight cells. This explicit non-menu rule permits one conversion only; it is not "
        "an observed server dispatch packet, automatic batching or an invented sink menu operation.",
        "inference", sha((DIRECTORY / "facts.json").read_bytes()))]
    return {
        "id": RECIPE, "name": "Fill a bucket with water",
        "inputs": [item_stack("item.bucket")], "outputs": [item_stack("item.water.bucket")],
        "failed_outputs": [], "tools": [], "requirements": [], "xp": [], "ticks": None,
        "success": constant_chance(), "target_objects": [OBJECT],
        "item_on_target": bound({"reach": 1, "guard": tutorial_at("stage.tutorial.mainland")}, target_source),
        "mechanics": {"method": METHOD, "guard": tutorial_at("stage.tutorial.mainland"), "chance_skill": None,
                      "cadence": {"single": bound(1, source), "first": bound(1, phase_source),
                                  "repeat": bound(1, phase_source), "menu_delay": bound(0, phase_source)},
                      "tool_ownership": "inventory", "failed_xp": [], "success_effects": [], "failure_effects": [],
                      "lifecycle": {"kind": "inventory_conversion"}},
        "source": source,
    }


def motion_binding():
    path = DIRECTORY / "facts.json"
    motion = load(path)["actor_motion"]
    if (motion["classification"] != "unresolved_source" or motion["sequence"] is not None
            or motion["no_animation_verified"]):
        raise ValueError("Water actor motion was promoted without original dispatch evidence")
    return {
        "rule": {"kind": "unverified", "reason": motion["reason"]},
        "source": [source_record(str(path.relative_to(ROOT)),
                                "Water-specific source motion is explicitly unverified, not silence or a similarly named sequence.",
                                "inference", sha(path.read_bytes()))],
    }


def without_water(content):
    result = deepcopy(content)
    if RECIPE in result["recipes"]:
        if result["recipes"][RECIPE] != recipe_definition():
            raise ValueError("Water addition differs from its exact audited definition")
        del result["recipes"][RECIPE]
        animations = result["ui"].get("actor_animations")
        if animations is not None:
            if animations["recipes"].get(RECIPE) != motion_binding():
                raise ValueError("Water motion differs from its exact unverified qualification")
            del animations["recipes"][RECIPE]
    return result


def contact_candidates(content):
    spawn = content["spawns"][SINK]
    definition = content["objects"][OBJECT]
    if (spawn["kind"] != {"kind": "object", "object": OBJECT}
            or spawn["placement"] != {"layer": "game_object", "shape": 10, "quarter_turns": 0}
            or definition["source_id"] != 14868
            or definition["clip"] != {"access_blocked_sides": 0, "blocks_movement": True, "blocks_projectiles": False}):
        raise ValueError("Source sink contact profile changed")
    cells = {(cell["tile"]["x"], cell["tile"]["y"], cell["tile"]["plane"]): cell
             for region in content["regions"].values() for cell in region["cells"]}
    x, y, plane = (spawn["tile"][axis] for axis in ("x", "y", "plane"))
    width, height = definition["size_x"], definition["size_y"]
    candidates = set()
    for dx in range(width):
        for dy in range(height):
            target = (x + dx, y + dy, plane)
            for ox, oy, towards, opposite in ((-1, 0, 2, 8), (1, 0, 8, 2), (0, -1, 1, 4), (0, 1, 4, 1)):
                point = (target[0] + ox, target[1] + oy, plane)
                near, far = cells.get(point), cells[target]
                if near and near["walkable"] and not (near["blocked_movement"] & towards
                    or near["blocked_sight"] & towards or far["blocked_sight"] & opposite):
                    candidates.add(point)
    return [{"x": x, "y": y, "plane": plane} for x, y, plane in sorted(candidates)]


def verify_delta(content):
    before = without_water(content)
    profiles = baselines()["profiles"]
    matches = [name for name, record in profiles.items()
               if semantic_hash(before) == record["semantic_sha256_without_revision"]]
    if len(matches) != 1 or RECIPE not in content["recipes"]:
        raise ValueError("Water delta modified an unrelated source gameplay/geometry/UI/audio/actor field")
    original = baseline_content(matches[0])
    changes = changed_paths(original, content)
    allowed = {("revision",), ("recipes", RECIPE)}
    if original["ui"].get("actor_animations") is not None:
        allowed.add(("ui", "actor_animations", "recipes", RECIPE))
    if {tuple(change["pointer"]) for change in changes} != allowed:
        raise ValueError("Water delta exceeds its recipe, applicable unverified motion and explicit revision")
    if content["spawns"][SINK]["interactions"]:
        raise ValueError("A source-absent sink menu operation was manufactured")
    return {
        "profile": matches[0], "base": profiles[matches[0]],
        "changed_json_paths": ["/" + "/".join(str(part).replace("~", "~0").replace("/", "~1") for part in row["pointer"]) for row in changes],
        "remainder_semantic_sha256": semantic_hash(before), "candidate_semantic_sha256": semantic_hash(content),
        "recipe_sha256": sha(canonical(content["recipes"][RECIPE])),
        "source_contact_candidates": contact_candidates(content),
        "contact_qualification": "Original1x2 footprint, source rotation/access masks and explicit walk/sight cells. "
                                 "Conservative cardinal one-tile item-on contact interpretation, not an observed arrival or accepted use.",
        "preexisting_source_unknowns_preserved": ["normal_grave_bank_all", "recipe.cooking.dough.motion", "empty_container.motion"],
        "water_actor_motion": load(DIRECTORY / "facts.json")["actor_motion"],
        "item_on_dispatch_declared": True,
        "dispatch_contract": "RecipeDefinition.item_on_target is an explicit source-bound reach/guard for one conversion. "
                             "The shared target admission retains availability, instance, footprint, source-side, "
                             "collision/sight and near-face checks without a menu-visible Production operation.",
        "native_execution_required": True,
        "migration_admitted": False, "actual_account_read_or_changed": False,
    }


def apply_water_fill(content, bindings):
    if content["schema_version"] != 4:
        raise ValueError("Water closure only accepts actual content4")
    if RECIPE in content["recipes"]:
        raise ValueError("Water recipe was already added before the audited extension")
    if semantic_hash(content) != baselines()["profiles"]["current5b"]["semantic_sha256_without_revision"]:
        raise ValueError("Canonical generation drifted from the complete current5b source profile before water")
    content["recipes"][RECIPE] = recipe_definition()
    from actor_animation4 import bind_actor_animations
    animations, proof = bind_actor_animations(content)
    content["ui"]["actor_animations"] = animations
    bindings["actor_animations"] = proof
    bindings["water_fill"] = verify_delta(content)
    bindings["application"]["water_fill"] = bindings["water_fill"]
    return content


if __name__ == "__main__":
    freeze_inputs()
    print(json.dumps({"public_source_baselines_verified": sorted(baselines()["profiles"])}))
