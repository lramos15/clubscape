"""Safely apply source resolutions and the one owner-approved loot choice before serialization."""

from copy import deepcopy
import importlib.util

from common import BINDINGS, ROOT, bound, canonical, load, sha, source_record, unique_sources


DIRECTORY = ROOT / "research/runtime-bindings"
RESOLUTIONS = DIRECTORY / "resolutions.json"
CONTEXT = DIRECTORY / "application-context.json"
APPROVAL = ROOT / "milestones/m1-goblin-loot-approval.json"
POTION_ITEMS = [
    "item.energy_potion.one_dose", "item.energy_potion.two_dose",
    "item.energy_potion.three_dose", "item.energy_potion.four_dose",
]
ADDITIONAL_ITEMS = ("item.energy_potion.three_dose", "item.energy_potion.three_dose.noted")


def source_applier():
    path = ROOT / "tools/runtime-bindings/apply.py"
    specification = importlib.util.spec_from_file_location("clubscape_source_binding_application", path)
    if specification is None or specification.loader is None:
        raise ValueError("Cannot load the exact source binding applicator")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def source_records(document, row, note, status="inference"):
    sources = {record["id"]: record for record in document["sources"]}
    return [source_record(
        sources[identifier]["url"], note + " Snapshot SHA-256: " + sources[identifier]["sha256"]
        + ". Documentary/code evidence, not a live observation.",
        status, str(sources[identifier].get("revision") or sources[identifier]["sha256"]))
        for identifier in row["source_refs"]]


def apply_source_bindings(content, inputs):
    report, context = load(RESOLUTIONS), load(CONTEXT)
    module = source_applier()
    before = deepcopy(content)
    from verify_assets import BASELINE, behavior_projection
    prior = deepcopy(before)
    for identifier in ADDITIONAL_ITEMS:
        if identifier not in prior["items"]:
            raise ValueError("Required original three-dose item extension is missing")
        del prior["items"][identifier]
    baseline = load(BASELINE)
    if sha(canonical(behavior_projection(prior))) != baseline["behavior_sha256"]:
        raise ValueError("Unrelated canonical behavior changed before exact source application")
    candidate, audit = module.prepare_with_audit(content, report, context)
    profile_sources = []
    for change in report["coupled_updates"]:
        if change["pointer"][:3] == ["initial_state", "runtime", "settings"]:
            profile_sources += source_records(
                load(DIRECTORY / "sources.json"), change,
                change["reason"] + " Supersedes the prior provisional creation-profile value.",
                "verified_reference" if change["classification"] == "known_fact" else "inference")
    candidate["initial_state"]["source"] = unique_sources(candidate["initial_state"]["source"] + profile_sources)
    bindable = [row for row in audit["bindings"] if row["action"] in ("applied", "already_applied")]
    if len(bindable) != 104 or len(audit["coupled_updates"]) != 7:
        raise ValueError("Canonical source application did not consume all104 bindings and7 coupled changes")
    core_loot_before = deepcopy(candidate["npcs"]["npc.goblin.level_2"]["combat"]["mechanics"]["loot"][:3])
    approval = load(APPROVAL)
    if (approval["decision"] != "approved" or approval["classification"] != "approved_adaptation"
            or approval["verified_osrs_probability"] is not False
            or approval["approved_policy"]["independent_event"] != {"numerator": 1, "denominator": 16}
            or approval["approved_policy"]["dose_weights"] != {"1": 1, "2": 1, "3": 1, "4": 1}):
        raise ValueError("The exact owner-approved goblin loot policy changed")
    for item, number in zip(POTION_ITEMS, (3014, 3012, 3010, 3008), strict=True):
        if candidate["items"][item]["source_id"] != number or not candidate["items"][item]["tradable"]:
            raise ValueError("Approved loot does not use the actual ordinary source potion identity")
    loot_path = "npcs.npc.goblin.level_2.combat.mechanics.loot[3]"
    original_loot = module.at(candidate, report["resolutions"][loot_path]["pointer"])
    if module.canonical_hash(original_loot) != report["resolutions"][loot_path]["original_binding_sha256"]:
        raise ValueError("Approved loot source target changed")
    # This independent 64-way draw has exactly the same joint distribution as
    # a 1/16 event followed by a uniform four-dose draw, without sharing the primary draw.
    potion_pool = {
        "kind": "exclusive", "total_weight": 64,
        "entries": [{"weight": 60, "items": []}] + [
            {"weight": 1, "items": [{"item": item, "minimum": 1, "maximum": 1}]} for item in POTION_ITEMS],
    }
    module.put(candidate, report["resolutions"][loot_path]["pointer"], potion_pool)
    npc = candidate["npcs"]["npc.goblin.level_2"]
    approval_source = source_record(
        "milestones/m1-goblin-loot-approval.json", "Owner-approved provisional independent1/16 event, "
        "uniform source1/2/3/4-dose energy potion. Separate exclusive64-way pool:60no-potion and one outcome "
        "per dose. Never shares/renormalizes the primary128 roll, never a members-only condition, "
        "and never claimed as verified OSRS odds. Invalid/replayed NPC lives must not resolve loot twice.",
        "approved_adaptation", "594a4fd")
    npc["source"] = unique_sources(npc["source"] + [approval_source])
    if npc["combat"]["mechanics"]["loot"][:3] != core_loot_before:
        raise ValueError("Approved supplement changed primary or guaranteed source loot")
    extended_values = extend_death_values(candidate, inputs)
    repeat = candidate["mechanics"]["death"]["repeat"]["value"]
    if "item.energy_potion.three_dose" not in repeat["supply_items"]:
        repeat["supply_items"].append("item.energy_potion.three_dose")
    repeat["source"] = unique_sources(repeat["source"] + [source_record(
        "milestones/m1-goblin-loot-approval.json",
        "Adding the approved actual3-dose variant also extends the already bound potion supply classification; "
        "the source death-supply rule itself is not an approved mechanical adaptation.",
        "inference", "594a4fd")])
    updates = []
    vital_row = report["resolutions"]["mechanics.vitals.level_up"]
    proposed_variant = vital_row["proposed_parent_variant"]
    # The source proposal is recorded now; only an actually declared shared enum
    # can be serialized. No substitute policy is chosen for a drained/boosted vital.
    enum_source = (ROOT / "crates/game-types/src/mechanics.rs").read_text()
    rust_name = "".join(word.capitalize() for word in proposed_variant.split("_"))
    enum_body = enum_source.split("pub enum LevelUpVitalPolicy {", 1)[1].split("}", 1)[0]
    vital_available = any(line.strip().rstrip(",") == rust_name for line in enum_body.splitlines())
    vital_provenance = source_records(load(DIRECTORY / "sources.json"), vital_row,
                                     "Raise current to new base only when current equaled old base; "
                                     "leave drained and boosted values unchanged. Do not consult this policy for unrelated skills.")
    if vital_available:
        expected = vital_row["original_binding_sha256"]
        if module.canonical_hash(candidate["mechanics"]["vitals"]["level_up"]) != expected:
            raise ValueError("Conditional vital source target changed")
        candidate["mechanics"]["vitals"]["level_up"] = bound(proposed_variant, vital_provenance)
        updates.append({"path": "mechanics.vitals.level_up", "action": "bound_declared_shared_enum",
                        "value": proposed_variant, "source": vital_provenance})
    else:
        updates.append({"path": "mechanics.vitals.level_up", "action": "awaiting_declared_shared_enum",
                        "proposed_value": proposed_variant, "source": vital_provenance})
    model_catalog = set(inputs.assets)
    item_extensions = {}
    for item in ADDITIONAL_ITEMS:
        definition = candidate["items"][item]
        raw = inputs.collections["item"][definition["source_id"]]
        base_item = inputs.collections["item"][raw["notedID"]] if raw["notedTemplate"] >= 0 else raw
        item_extensions[item] = {
            "source_id": definition["source_id"], "source_definition": definition["source"][0]["reference"],
            "source_payload_sha256": load(BINDINGS / "application-item-definitions.json.gz")["items"][str(definition["source_id"])]["payload_sha256"],
            "asset_published": definition["asset"] is not None,
            "raw_inventory_model": raw["inventoryModel"],
            "inventory_model": base_item["inventoryModel"],
            "missing_inventory_model": f"asset.source.osrs.cache2695.model.{base_item['inventoryModel']}" not in model_catalog,
        }
    changes = changed_paths(before, candidate)
    permitted = [tuple(row["pointer"]) for row in report["resolutions"].values() if row["apply_to_ordinary_profile"]]
    permitted += [tuple(change["pointer"]) for change in report["coupled_updates"]]
    permitted += [
        ("npcs", "npc.goblin.level_2", "combat", "mechanics", "loot", 3),
        ("npcs", "npc.goblin.level_2", "source"),
        ("mechanics", "death", "repeat"),
        ("mechanics", "vitals", "level_up"),
        ("initial_state", "source"),
    ]
    for change in changes:
        pointer = tuple(change["pointer"])
        if not any(pointer[:len(parent)] == parent for parent in permitted):
            raise ValueError(f"Unapproved non-target source application change: {pointer}")
    audit.update({
        "schema_version": 1, "task": "m1-apply-runtime-bindings",
        "resolutions_sha256": sha(RESOLUTIONS.read_bytes()), "application_context_sha256": sha(CONTEXT.read_bytes()),
        "bound_path_count": len(bindable), "coupled_update_count": len(audit["coupled_updates"]),
        "approved_loot": {"adaptation": approval["adaptation_id"], "approval_sha256": sha(APPROVAL.read_bytes()),
                          "path": loot_path, "event_chance": {"numerator": 1, "denominator": 16},
                          "conditional_weights": {item: 1 for item in POTION_ITEMS},
                          "lowered_pool": potion_pool, "independent_of_primary": True,
                          "primary_and_guaranteed_sha256": sha(canonical(core_loot_before)),
                          "classification": "approved_adaptation", "verified_osrs_probability": False,
                          "kill_identity_contract": "Actor credit, original NPC spawn/life and resolved-loot marker; "
                                                    "duplicate/invalidated life events must not pay again.",
                          "retained_tertiary_candidates": inputs.rules["activities"]["loot_tables"][0]["tertiary"],
                          "tertiary_hook_note": "Beginner-clue and member-only tertiary identity/ownership families remain "
                                               "explicit full-target candidates. The approved potion policy does not remove "
                                               "or certify those separately unbound runtime selectors."},
        "item_extensions": item_extensions, "death_value_extensions": extended_values,
        "vital_policy": updates[0], "changed_paths": changes,
        "coupled_profile_provenance": profile_sources,
        "baseline_behavior_sha256": baseline["behavior_sha256"],
        "applied_behavior_sha256": sha(canonical(behavior_projection(candidate))),
        "original_content_counts": baseline["counts"],
        "original_immutable_source_geometry_hashes": baseline["source_geometry_hashes"],
        "source_supported_inferences_not_owner_approvals": 98,
        "residuals": classify_residuals(candidate, report),
        "remaining_selector_hooks": [
            "NPC loot must use original spawn/life/credit/resolved state and the engine's source-owned ground origin; "
            "NpcCombatMechanics currently has no explicit ordinary-loot ground-policy selector.",
            "Preserve beginner-clue and member-only tertiary candidates with source family/ownership eligibility "
            "before enabling those full-target outcomes; the potion approval does not certify their selectors.",
            "Ground death supplies require their source character/world-switch persistence and offline-paused clock, "
            "not a wall-clock expiry inferred solely from the policy numbers.",
            "Engine-owned traversal, attack/recovery UI, engagement and initial setting selectors must consume the "
            "applied fields; this content task does not guess new shared field names.",
        ],
        "gameplay_executed": False, "presentation_approved": False,
    })
    return candidate, audit


def classify_residuals(content, resolutions):
    remaining = []
    def visit(value, path=""):
        if isinstance(value, dict):
            if value.get("status") == "unresolved" or value.get("kind") == "unresolved":
                remaining.append((path, value))
            for key, child in value.items():
                visit(child, (path + "." if path else "") + key)
        elif isinstance(value, list):
            for index, child in enumerate(value):
                visit(child, f"{path}[{index}]")
    visit(content)
    grants = {line["item"] for grant in content["mechanics"]["grants"].values() for line in grant["lines"]}
    produced = {stack["item"] for recipe in content["recipes"].values() for stack in recipe["outputs"] + recipe["failed_outputs"]}
    rare = {"item.ensouled_goblin_head", "item.milk.bottomless_bucket"}
    initial = {stack["item"] for stack in content["initial_state"]["inventory"]["slots"] if stack}
    initial.update(stack["item"] for stack in content["initial_state"]["bank"]["slots"] if stack)
    initial.update(stack["item"] for stack in content["initial_state"]["equipment"].values())
    ordinary_loot = set()
    def members_guard(guard):
        if guard["kind"] == "members_world":
            return content["mechanics"]["world_members"]
        if guard["kind"] == "not" and guard["guard"]["kind"] == "members_world":
            return not content["mechanics"]["world_members"]
        return True
    def loot_items(pool):
        if pool["kind"] == "conditional":
            if members_guard(pool["guard"]):
                for nested in pool["pools"]:
                    loot_items(nested)
        elif pool["kind"] == "exclusive":
            ordinary_loot.update(item["item"] for entry in pool["entries"] for item in entry["items"])
        else:
            ordinary_loot.update(item["item"] for item in pool.get("items", []))
    for npc in content["npcs"].values():
        if npc["combat"] and npc["combat"]["mechanics"]:
            for pool in npc["combat"]["mechanics"]["loot"]:
                loot_items(pool)
    ordinary_acquired = initial | grants | produced | ordinary_loot
    source_item_spawns = {spawn["kind"]["stack"]["item"] for spawn in content["spawns"].values()
                          if spawn["kind"]["kind"] == "item"}
    shop_stock = {row["item"] for shop in content["shops"].values() for row in shop["stock"]}
    ordinary_acquired |= source_item_spawns | shop_stock
    introduced_instances = False
    def collect_effects(value):
        nonlocal introduced_instances
        if isinstance(value, dict):
            if value.get("kind") == "give_items":
                ordinary_acquired.update(stack["item"] for stack in value["items"])
                introduced_instances |= any(stack.get("instance") is not None for stack in value["items"])
            for child in value.values():
                collect_effects(child)
        elif isinstance(value, list):
            for child in value:
                collect_effects(child)
    for field in ("spawns", "dialogues", "tutorial", "quests", "recipes", "mechanics"):
        collect_effects(content[field])
    changed = True
    while changed:
        before = set(ordinary_acquired)
        for identifier in before:
            item = content["items"][identifier]
            ordinary_acquired.update(value for value in (item["noted_variant"], item["unnoted_variant"]) if value)
            for action in (content.get("ui") or {}).get("item_actions", {}).get(identifier, []):
                if action["action"]["kind"] in ("drink", "empty"):
                    ordinary_acquired.add(action["action"]["replacement"])
        changed = ordinary_acquired != before
    conditional = {item for item, definition in content["items"].items() if isinstance(definition["stackable"], dict)}
    active, inactive = [], []
    module = source_applier()
    for path, value in remaining:
        row = resolutions["resolutions"].get(path)
        if row is None:
            raise ValueError("Unclassified canonical unresolved binding: " + path)
        if row["classification"] == "inactive_dependency":
            group = row["rule_group"]
            if group == "conditional_stack_alternatives":
                if ordinary_acquired & rare:
                    raise ValueError("Rare conditional item became acquired by the ordinary profile")
                proof = {"ordinary_acquisition_absent": True, "rare_items": sorted(rare),
                         "precondition": "Invalidate if a grant, recipe, ordinary loot, shop or spawn enables this acquisition."}
            elif group == "nondepleting_fishing_no_respawn":
                rule = module.at(content, row["pointer"][:-2])
                if rule["depletion"] != {"numerator_at_level_1": 0, "numerator_at_level_99": 0,
                                         "denominator": 1, "domain": {"kind": "constant"}}:
                    raise ValueError("Nondepleting fishing proof changed")
                proof = {"constant_depletion": 0, "respawn_policy_reads_on_this_method": 0,
                         "precondition": "Recheck if depletion or relocation starts reading this policy."}
            elif group == "office_capacity_unreachable":
                capacity = content["mechanics"]["death"]["office_capacity"]
                excluded = conditional - ordinary_acquired
                upper_bound = len(content["items"]) - len(excluded)
                if upper_bound > capacity:
                    raise ValueError("Item universe exceeds the conditional Office capacity proof")
                if introduced_instances or any(content["items"][item]["charges"] is not None or isinstance(content["items"][item]["stackable"], dict)
                       for item in ordinary_acquired):
                    raise ValueError("Ordinary item instances invalidate Office merge/capacity proof")
                proof = {"represented_item_keys": len(content["items"]), "office_capacity": capacity,
                         "ordinary_item_key_upper_bound": upper_bound, "excluded_unacquired_conditional_keys": sorted(excluded),
                         "ordinary_acquired_instance_items": 0,
                         "precondition": "Office must merge ordinary non-instanced item keys. Recheck on universe, acquisition "
                                         "or instance/merge-rule changes; never unconditionally discard overflow."}
            else:
                raise ValueError("Unknown inactive proof: " + group)
            inactive.append({"path": path, "rule_group": group, "reason": value["reason"], "proof": proof})
        else:
            active.append({"path": path, "rule_group": row["rule_group"], "reason": value["reason"],
                           "required_branch": "Actual HP/Prayer base-level gain; not Mining/Cooking-only gains."
                                              if path == "mechanics.vitals.level_up" else "Reachable source operation."})
    return {"unresolved_total": len(remaining), "active_or_conditionally_active_count": len(active),
            "active_or_conditionally_active": sorted(active, key=lambda value: value["path"]),
            "inactive_full_target_count": len(inactive),
            "inactive_full_target": sorted(inactive, key=lambda value: value["path"]),
            "policy": "A source binding is required only when its proved branch is reachable. Inactive definitions "
                      "remain in the full target; no unresolved value is replaced by a success-shaped default."}


def extend_death_values(content, inputs):
    audit = load(DIRECTORY / "death-values.json")
    sources = load(DIRECTORY / "sources.json")
    price_source = next(source for source in sources["sources"] if source["id"] == "runelite.guide_prices")
    import gzip
    feed_bytes = gzip.decompress((DIRECTORY / "inputs/guide-prices.json.gz").read_bytes())
    if sha(feed_bytes) != price_source["sha256"]:
        raise ValueError("Pinned guide-price feed changed")
    import json
    feed = {entry["id"]: entry for entry in json.loads(feed_bytes)}
    values = content["mechanics"]["value_providers"]["value_provider.osrs.death"]["values"]["value"]
    rows = []
    for item in ADDITIONAL_ITEMS:
        definition = content["items"][item]
        canonical_item = definition["unnoted_variant"] or item
        normal = content["items"][canonical_item]
        quoted = feed.get(normal["source_id"])
        if quoted is None:
            raise ValueError("Approved potion has no pinned guide-price quote")
        value = max(quoted["price"], 3 * normal["base_value"] // 5)
        values[item] = value
        rows.append({"item": item, "canonical_item": canonical_item, "source_id": normal["source_id"],
                     "guide_price": quoted["price"], "wiki_price_not_used": quoted.get("wikiPrice"),
                     "high_alchemy": 3 * normal["base_value"] // 5, "effective_death_value": value,
                     "source_feed_sha256": price_source["sha256"]})
    if set(values) != set(content["items"]):
        raise ValueError("Applied death table does not cover every actual item definition")
    provider = content["mechanics"]["value_providers"]["value_provider.osrs.death"]
    provider["values"]["source"] = unique_sources(provider["values"]["source"] + [source_record(
        "research/m1-bindings/application-item-definitions.json.gz",
        "Original3010/3011 definitions extend the same fixed guide/high-alchemy table. "
        "Guide123 is used, not active wikiPrice108; notes resolve to their original unnoted item.",
        "inference", price_source["sha256"])])
    return {"original_value_rows": audit["coverage"], "current_value_rows": len(values), "added": rows}


def changed_paths(before, after, pointer=()):
    if before == after:
        return []
    if isinstance(before, dict) and isinstance(after, dict):
        result = []
        for key in sorted(before.keys() | after.keys()):
            if key not in before or key not in after:
                result.append({"pointer": list(pointer + (key,)), "operation": "add" if key not in before else "remove"})
            else:
                result.extend(changed_paths(before[key], after[key], pointer + (key,)))
        return result
    if isinstance(before, list) and isinstance(after, list) and len(before) == len(after):
        return [change for index, (old, new) in enumerate(zip(before, after, strict=True))
                for change in changed_paths(old, new, pointer + (index,))]
    return [{"pointer": list(pointer), "operation": "replace"}]
