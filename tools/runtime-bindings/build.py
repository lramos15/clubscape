#!/usr/bin/env python3
"""Author exact-path source resolutions without modifying integrated content."""

from collections import Counter
from copy import deepcopy
import gzip
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/runtime-bindings"
INPUT = ROOT / "research/m1-bindings/unresolved-bindings.json"
CONTENT = ROOT / "content/m1/game-content.json.gz"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + "\n")


def index_content(value):
    result = {}

    def visit(node, path="", pointer=()):
        result[path] = (pointer, node)
        if isinstance(node, dict):
            for key, child in node.items():
                visit(child, f"{path}.{key}" if path else key, pointer + (key,))
        elif isinstance(node, list):
            for number, child in enumerate(node):
                visit(child, f"{path}[{number}]", pointer + (number,))

    visit(value)
    return result


def main():
    content = json.loads(gzip.decompress(CONTENT.read_bytes()))
    unresolved = json.loads(INPUT.read_text())["bindings"]
    index = index_content(content)
    sources = json.loads((OUT / "sources.json").read_text())["sources"]
    by_id = {s["id"]: s for s in sources}
    wiki = {s["page"]: s["id"] for s in sources if s["kind"] == "wiki_revision"}
    resolutions = {}
    additions = []

    def refs(*names):
        return [wiki.get(name, name) for name in names]

    def source_records(ids, inference=False, note=""):
        return [{
            "reference": by_id[s]["url"],
            "revision": str(by_id[s].get("revision") or by_id[s]["sha256"]),
            "status": "inference" if inference else "verified_reference",
            "notes": (note + " Snapshot SHA-256: " + by_id[s]["sha256"]
                      + ". Documentary/code evidence, not a live gameplay observation.").strip(),
        } for s in ids]

    def resolution(row, group, classification, value_type, value, ids, reason,
                   impact, *, apply=True, raw_value=False, **extra):
        pointer, current = index[row["path"]]
        inference = classification == "source_supported_inference"
        if apply:
            replacement = (deepcopy(value) if raw_value else {
                "status": "bound", "value": deepcopy(value),
                "source": source_records(ids, inference, reason),
            })
        else:
            replacement = None
        resolutions[row["path"]] = {
            "path": row["path"], "pointer": list(pointer), "rule_group": group,
            "classification": classification, "value_type": value_type,
            "proposed_value": deepcopy(value), "replacement": replacement,
            "apply_to_ordinary_profile": apply,
            "source_refs": ids,
            "source_identity": [{
                "id": s, "url": by_id[s]["url"], "revision": by_id[s].get("revision"),
                "retrieved_at": by_id[s]["retrieved_at"], "sha256": by_id[s]["sha256"],
            } for s in ids],
            "reason": reason, "acceptance_impact": impact,
            "original_binding_sha256": hashlib.sha256(
                json.dumps(current, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
            "original_reason": row["reason"], "source_observation_claimed": False,
            **extra,
        }

    def related(identifier, pointer, value, ids, reason, inference=False):
        node = content
        existed = True
        for key in pointer:
            if isinstance(node, dict) and key not in node:
                existed = False
                break
            node = node[key]
        additions.append({
            "id": identifier, "pointer": pointer, "operation": "replace" if existed else "add",
            "value": value, "classification": "source_supported_inference" if inference else "known_fact",
            "source_refs": ids, "reason": reason,
            "original_value_sha256": (hashlib.sha256(json.dumps(node, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
                                      if existed else None),
        })

    retained = OUT / "inputs"
    retained.mkdir(exist_ok=True)
    raw_price_path = OUT / by_id["runelite.guide_prices"]["local_raw"]
    price_bytes = (raw_price_path.read_bytes() if raw_price_path.exists()
                   else gzip.decompress((retained / "guide-prices.json.gz").read_bytes()))
    if hashlib.sha256(price_bytes).hexdigest() != by_id["runelite.guide_prices"]["sha256"]:
        raise ValueError("Price bytes no longer match the pinned source receipt")
    price_feed = json.loads(price_bytes)
    (retained / "guide-prices.json.gz").write_bytes(gzip.compress(price_bytes, mtime=0))
    guide_prices = {r["id"]: r for r in price_feed}
    values, value_rows = {}, []
    for identifier, definition in sorted(content["items"].items()):
        canonical = definition["unnoted_variant"] or identifier
        normal = content["items"][canonical]
        source_id = normal["source_id"]
        guide = guide_prices.get(source_id)
        alchemy = normal["base_value"] * 3 // 5
        if canonical == "item.coins":
            effective, method = 1, "currency_unit"
        elif not normal["tradable"]:
            effective, method = normal["base_value"], "untradeable_kept_value"
        elif guide is not None:
            effective, method = max(guide["price"], alchemy), "maximum_guide_and_high_alchemy"
        else:
            effective, method = alchemy, "non_exchange_tradeable_high_alchemy"
        values[identifier] = effective
        value_rows.append({
            "item": identifier, "canonical_item": canonical, "source_item_id": source_id,
            "source_base_value": normal["base_value"], "tradable": normal["tradable"],
            "guide_price": guide["price"] if guide else None,
            "wiki_transaction_price_not_used": guide.get("wikiPrice") if guide else None,
            "high_alchemy": alchemy, "method": method, "effective_death_value": effective,
            "classification": "source_supported_inference" if not normal["tradable"] else "known_fact",
            "note": ("General Value-page rule; exceptional untradeable overrides remain full-target work."
                     if not normal["tradable"] else
                     "Missing guide row is explicit; no invented GE quote or shop-price substitution."),
        })
    value_sources = refs("runelite.guide_prices", "code.runelite.ItemClient",
                        "code.runelite.ItemManager", "Items Kept on Death",
                        "High Level Alchemy", "Value", "Coins")
    write("death-values.json", {
        "schema_version": 1, "source_refs": value_sources, "rows": value_rows,
        "price_field": "price (Jagex guide), never wikiPrice (actively traded price)",
        "coverage": len(value_rows), "content_sha256": digest(CONTENT),
        "source_definitions": "research/m1-bindings/definitions.json.gz",
        "source_definitions_sha256": digest(ROOT / "research/m1-bindings/definitions.json.gz"),
        "source_version": "selected cache240 definitions plus the dated public guide-price snapshot",
        "live_death_screen_verified": False,
    })

    provision_items = [
        ("item.axe.bronze", 1), ("item.pickaxe.bronze", 1), ("item.tinderbox", 1),
        ("item.fishing_net.small", 1), ("item.shrimps.cooked", 1), ("item.dagger.bronze", 1),
        ("item.sword.bronze", 1), ("item.shield.wooden", 1), ("item.shortbow", 1),
        ("item.arrow.bronze", 25), ("item.rune.air", 25), ("item.rune.mind", 15),
        ("item.bucket", 1), ("item.pot", 1), ("item.bread", 1), ("item.rune.water", 6),
        ("item.rune.earth", 4), ("item.rune.body", 2),
    ]
    tutorial_types = {i for i, _ in provision_items} | {
        "item.shrimps.raw", "item.shrimps.burnt", "item.logs.normal", "item.ashes",
        "item.flour.pot", "item.water.bucket", "item.bread.dough", "item.bread.burnt",
        "item.ore.tin", "item.ore.copper", "item.bar.bronze", "item.hammer", "item.bones.tutorial",
    }
    tutorial_types |= {content["items"][i]["noted_variant"] for i in list(tutorial_types)
                       if content["items"][i]["noted_variant"] is not None}
    assert "item.coins" not in tutorial_types
    departure_sources = refs("Learning the Ropes", "Tutorial Island", "Transcript:Learning the Ropes")
    departure_policies = [
        {"kind": "remove_items", "container": container, "items": sorted(tutorial_types)}
        for container in ["inventory", "equipment", "bank"]
    ]
    provision_id = "grant.tutorial.departure.provisions"
    provision_entitlement = "entitlement.tutorial.departure.provisions"
    provision_provenance = source_records(
        departure_sources, True,
        "Standard noncurrency provisions after documented tutorial cleanup. Preserve coins wherever held; no second bank seed.")
    provision_grant = {
        "id": provision_id, "target": "inventory", "capacity": "atomic",
        "entitlement": provision_entitlement,
        "lines": [{"item": i, "quantity": q, "mode": "add", "ownership": "inventory"} for i, q in provision_items],
        "source": provision_provenance,
    }
    provision_entitlement_value = {
        "id": provision_entitlement, "purpose": {"kind": "grant", "grant": provision_id},
        "source": provision_provenance,
    }
    related("departure_provision_grant", ["mechanics", "grants", provision_id], provision_grant,
            departure_sources, "Coupled with noncurrency reconciliation; never apply reconciliation alone.", True)
    related("departure_provision_entitlement", ["mechanics", "entitlements", provision_entitlement],
            provision_entitlement_value, departure_sources,
            "Separate once-only grant entitlement prevents replay from recreating supplies.", True)
    related("departure_provision_effect", ["mechanics", "travels", "travel.tutorial.departure", "completion_effects"],
            [{"kind": "reconcile_containers", "reconciliation": "reconciliation.tutorial.departure"},
             {"kind": "grant", "grant": provision_id}],
            departure_sources, "Transport, cleanup, provision claim and stage commit are one transaction.", True)
    related("death_provider_method", ["mechanics", "value_providers", "value_provider.osrs.death", "method"],
            "fixed_source_table", value_sources,
            "The finite table includes documented untradeable-value/currency rules as well as max(GE guide, HA).")
    related("death_provider_revision", ["mechanics", "value_providers", "value_provider.osrs.death", "revision"],
            "osrs240-guide-" + by_id["runelite.guide_prices"]["sha256"][:16], value_sources,
            "Replace the stale '-unresolved' provider identity with the exact feed identity.")
    related("initial_death_auto_equip", ["initial_state", "runtime", "settings", "death_auto_equip"],
            True, refs("Update:Grid Master Rewards, Poll & New Player Improvements"),
            "The official2025-10-22 ALL-player default list explicitly enables Auto-equip Worn Items from Gravestone; replace the old false candidate.")
    related("initial_death_supply_piles", ["initial_state", "runtime", "settings", "death_supply_piles"],
            True, refs("Update:Death Changes", "Items Kept on Death", "Items"),
            "The documented death default drops food/potions and modern rules expose an opt-out. Select that inherited default rather than the previous unobserved false candidate; current fresh-account toggle is still an inference.", True)

    for row in unresolved:
        path = row["path"]
        if path.endswith(".stackable.rule"):
            resolution(row, "conditional_stack_alternatives", "inactive_dependency", "ConditionalStackRule", None,
                       refs("Items", "Tutorial Island"),
                       "Fresh-origin ensouled head is a members alternative; bottomless milk requires a separate boss/quest acquisition, not the ordinary bucket route. Keep mode2 and the distinct identities; do not coerce to bool.",
                       "Not an ordinary M1 prerequisite. Keep these definitions and unresolved conditional rules for the full target; only require them when such an instance is actually owned.",
                       apply=False, inactive_proof="No ordinary tutorial grant, ingredient route or F2P primary loot pool produces either item.",
                       full_target_requirement="Bind per-container instance/origin/charge-aware stacking before enabling these acquisitions.")
        elif path.endswith("wind_strike.maximum_hit"):
            resolution(row, "wind_strike_level_table", "codebug_parent_fixed", "MaximumHitFormula",
                       {"kind": "level_table", "skill": "skill.magic", "basis": "current", "hits": {"1": 2, "5": 4, "9": 6, "13": 8}},
                       refs("Wind Strike"), "The numeric-key decoder was fixed by parent7150464 already in base. Restore the existing known source table, not a fixed damage substitute.",
                       "Immediately bindable; exercise the actual strict decoder with string JSON keys.",
                       parent_fix="7150464", existing_source_refs=row["source"],
                       no_additional_research_required=True)
        elif path == "mechanics.death.office_overflow":
            resolution(row, "office_capacity_unreachable", "inactive_dependency", "RecoveryOverflow", None,
                       refs("Death's Office", "Grave"),
                       "The source capacity is 120 distinct bank-like entries. This finite ordinary content has at most116 item IDs, with the only per-instance charged/origin alternatives excluded. Merging identical ordinary items cannot exceed120 keys.",
                       "Do not fail ordinary recovery by requiring an eviction policy before overflow is possible. Preserve the future overflow gate; never silently choose deletion of oldest or cheapest items.",
                       apply=False, inactive_proof={"ordinary_merge_key_upper_bound": len(content["items"]), "capacity": 120,
                                                    "requires": "merge identical plain stacks; no new item universe or per-instance rare items"},
                       full_target_requirement="Source says incoming expired-grave items can be deleted at capacity; exact eviction/admission order is not established.")
        elif path == "mechanics.death.repeat":
            supplies = ["item.bread", "item.shrimps.cooked", "item.beer",
                        "item.energy_potion.one_dose", "item.energy_potion.two_dose", "item.energy_potion.four_dose"]
            resolution(row, "repeat_death", "source_supported_inference", "RepeatDeathPolicy",
                       {"keep_old_grave_location": True, "refresh_timer_if_contents_change": True,
                        "old_unstackable_per_item_limit": 28,
                        "old_items_to_office": ["item.bones", "item.bones.tutorial", "item.ore.copper", "item.ore.tin"],
                        "supply_items": supplies, "supply_ground_policy": "ground_policy.death_supplies",
                        "source": source_records(refs("Grave", "Items"), True,
                                                 "Finite M1 resource/supply projection; beer classified as a consumed drink supply.")},
                       refs("Grave", "Items"),
                       "Apply the documented old-grave sequence to represented item types: old bones/ores to Office, ordinary unstackable excess above28 to Office, old food/potions to private supply piles, then new losses. Bank notes are not unstackable ores.",
                       "Enables real repeat deaths without requiring unrelated full-game item lists. Add energy potion(3) if/when the unresolved loot supplement adds it; beer supply classification is a localized inference.",
                       required_semantics=["Supply piles follow character/world-switching and pause expiry offline.", "Refresh only when contents change.", "Do not move the existing grave within this non-Wilderness domain."])
        elif path == "mechanics.death.restoration":
            resolution(row, "death_restore_once", "source_supported_inference", "DeathVitalRestoration",
                       {"on_arrival": {"hitpoints": {"kind": "to_base_maximum"},
                                       "prayer": {"kind": "to_base_maximum"},
                                       "run_energy": {"kind": "set", "amount": 10000}},
                        "on_first_office_exit": {}},
                       refs("code.rsmod.PlayerDeath", "Prayer", "Energy", "Transcript:Death (NPC)"),
                       "Contemporary PlayerDeath resets stats on death arrival; Prayer documents reset on dying and Energy lists death as a full energy restoration. Office exit is not a second death: explicitly preserve its current vitals rather than heal twice.",
                       "Restores only source base maxima on the actual death event. Office exit drain preservation is an inference; empty exit map means explicit no second restoration, not missing data.",
                       required_semantics=["Disable active prayers and reset prayer fractional drain at death.", "No XP/quest/bank reset.", "Ordinary travel/Home Teleport is not a death heal."])
        elif path == "mechanics.death.ties":
            resolution(row, "equal_value_retention", "source_supported_inference", "RetentionTiePolicy",
                       "inventory_then_equipment", refs("Items Kept on Death", "Items Kept on Death (Interface)"),
                       "Value order is established; exact equal-value ordering is not. Retain deterministic original inventory slots followed by equipment slots, matching the earlier documented stable-slot assumption without inventing value differences.",
                       "Only exact-value ties change which three units remain carried; the other items stay recoverable. All counts/value ranks remain conserved.",
                       refinement="Replace this one comparator if an authoritative current tie-order reference is found; no owner approval is inferred.")
        elif ".ground_policies." in path:
            if "death_supplies" in path:
                value, group, classification = None, "private_death_supplies", "known_fact"
                ids = refs("Items", "Grave")
                reason = "Supply piles behave as untradeable ground items: never public, character-bound across worlds, offline-paused timer. Bound(null) is the intentional never-public Option value."
            elif "fire_ashes" in path and path.endswith("expires_after"):
                value, group, classification = 300, "fire_ashes_expiry", "known_fact"
                ids = refs("Money making guide/Collecting ashes", "Ashes", "Game tick")
                reason = "The collecting-ashes guide explicitly gives about three minutes after fire expiry:300 nominal600ms ticks, not the fire's own lifetime."
            elif "fire_ashes" in path:
                value, group, classification = 0, "fire_ashes_visibility", "source_supported_inference"
                ids = refs("Ashes", "Money making guide/Collecting ashes", "Fire")
                reason = "Ashes are generated by an expired shared fire, not manually dropped inventory. Treat this world byproduct as immediately public; the visibility inference is explicit, not inferred from the generic NPC-drop timer."
            elif path.endswith("expires_after"):
                value, group, classification = 200, "ammunition_expiry", "source_supported_inference"
                ids = refs("Drops", "Arrows")
                reason = "Use automatic tradeable-drop lifetime200 ticks for spent ammunition; Drops includes ranged ammunition and distinguishes these drops from300-tick manual inventory drops."
            else:
                value, group, classification = 100, "ammunition_visibility", "source_supported_inference"
                ids = refs("Drops", "Arrows", "Drop")
                reason = "Use the automatic-drop private100-tick period for recoverable spent arrows. Do not conflate it with inventory Drop's fresh-account restrictions or immediate ashes."
            resolution(row, group, classification, "Option<u32>", value, ids, reason,
                       "Each origin retains its own lifetime. Tutorial-wide50-tick cleanup and source owner restrictions still apply where specified; ground clocks are not grave clocks.",
                       required_semantics=("Character-scoped, offline-paused active6000-tick lifetime for death supplies."
                                           if "death_supplies" in path else
                                           "Preserve explicit origin and owner; manual Drop is a different operation from automatic ammunition/byproduct creation."))
        elif ".projectiles." in path:
            magic = "wind_strike" in path
            resolution(row, "projectile_wind_strike" if magic else "projectile_bow", "source_supported_inference", "ProjectileTiming",
                       {"launch_delay_ticks": 0, "base_flight_ticks": 2,
                        "ticks_per_tile": {"numerator": 1, "denominator": 3 if magic else 6},
                        "rounding": "nearest_ties_up", "damage_on_launch": False, "recheck_target_on_impact": True},
                       refs("Hit delay", "Game tick"),
                       ("Published Magic table is1+floor((distance+1)/3); published bow table is1+floor((distance+3)/6). "
                        "Both get +1 for player-to-NPC processing order. Positive-integer nearest rounding encodes the offsets exactly. "
                        "Launch epoch is the accepted attack tick; zero means no additional whole-tick warmup, not zero flight."),
                       "Binds real distance-dependent PvM damage timing. It is not a visual client-cycle projectile offset or a general PvP formula. Impact target checking is a stated adapter inference.",
                       required_semantics=["Use source same-edge Chebyshev distance; do not use NPC center or Euclidean distance.",
                                           "Do not add the NPC processing-order tick a second time.",
                                           "On impact recheck original actor/life and presence, not launch range/LOS or accuracy.",
                                           "Do not hit a new respawn using an old projectile.",
                                           "Client-cycle animation/projectile start offsets remain rendering data, not this hit-delay contract."])
        elif ".reconciliations." in path:
            resolution(row, "departure_currency_conserving", "source_supported_inference",
                       "Vec<ContainerReconciliation>", departure_policies, departure_sources,
                       "The bank has25 coins BEFORE departure, so it is not a second departure coin reward. Remove documented tutorial noncurrency types and note variants from carried/equipped/banked storage, then issue the documented18-kind kit once. Preserve coin balances and positions, including legitimate withdrawal/deposit/loss.",
                       "Unlike the old provisional replace-bank-with25 policy, this cannot restore lost coins or duplicate withdrawn coins. Noncurrency cleanup across bank/equipment and kit placement in inventory remain labeled inferences; no observed departure dump or owner approval is claimed.",
                       coupled_update_ids=["departure_provision_grant", "departure_provision_entitlement", "departure_provision_effect"],
                       domain=["Normal account finishing tutorial for the first time.", "Only source-legitimate tutorial item acquisition; no moderator/imported precompleted inventory."],
                       required_semantics=["Apply transport, cleanup, provision grant, both entitlement ledgers and tutorial completion atomically.",
                                           "Never run these policies on a departed account.", "Never replace bank or inventory currency with a constant25.",
                                           "Do not apply this binding without the coupled grant/effect updates."],
                       supersedes_provisional_choice="research/journey-rules/decisions.json#assumption.departure_reconciliation",
                       not_superseded="Original source snapshots, prior evidence and approval records remain unchanged.")
        elif ".travels." in path and path.endswith(".channel_ticks"):
            office = ".travel.death." in path
            stairs = ".travel.castle_" in path
            mill = ".travel.mill_" in path
            value = 0 if stairs else 1
            ids = (refs("Transcript:Death (NPC)", "Game tick") if office else
                   refs("code.rsmod.SpiralStaircaseScript") if stairs else
                   refs("code.rsmod.WindmillLadderScript", "code.rsmod.LadderScript") if mill else
                   refs("code.rsmod.DungeonLadderScript", "code.rsmod.LadderScript"))
            group = "office_portal_phase" if office else "spiral_stairs_phase" if stairs else "direct_ladder_phase"
            reason = ("Office portal gets one explicit server interaction tick; the source defines portal gating, not a long Home Teleport channel. This is a localized phase inference."
                      if office else
                      "Contemporary Lumbridge spiral-stair code telejumps at the accepted interaction with no additional channel; source movement/approach and dialogue costs are separate."
                      if stairs else
                      "Contemporary direct Climb-up/down handlers animate, delay1 then telejump. Surface/dungeon offsets and mill floor relationships remain the integrated source links.")
            resolution(row, group, "source_supported_inference", "u32", value, ids, reason,
                       "Do not impose24-tick Home Teleport on stairs/portals. Zero stair channel is supported by code, not a shortcut through world geometry.",
                       timing_epoch="After valid approach and explicit direction selection.",
                       caveats=["Choice-dialogue ladder branches use an additional tick (2 after choice in the corroborating code); preserve that UI phase rather than changing every direct-op channel.",
                                "Do not reinterpret the landing candidates as observed exact live tiles."])
        elif ".value_providers." in path:
            resolution(row, "death_value_snapshot", "source_supported_inference", "BTreeMap<ItemId,u64>", values, value_sources,
                       "A public dated RuneLite price feed is available. Its price field is Jagex guide price, distinct from wikiPrice. Resolve note aliases; apply max(guide, floor(3*base/5)) for ordinary exchange items, explicit HA for non-GE tradeables, currency1 and the Value-page untradeable rule.",
                       "All116 represented IDs have auditable rows. These are frozen source values, not current player sale proceeds. Special full-target untradeable exceptions are not inferred.",
                       coupled_update_ids=["death_provider_method", "death_provider_revision"],
                       value_audit="research/runtime-bindings/death-values.json",
                       feed_sha256=by_id["runelite.guide_prices"]["sha256"])
        elif path == "mechanics.vitals.level_up":
            resolution(row, "level_up_conditional_current", "codebug_parent_required",
                       "LevelUpVitalPolicy (requires conditional variant)", None,
                       refs("code.rsmod.PlayerSkillXP", "code.rsmod.PlayerSkillXPTest", "Skills"),
                       "Contemporary skill-XP code updates current level only when it equaled the old base. Drained or boosted current levels remain unchanged. None of PreserveCurrent/IncreaseByBaseDifference/RestoreToBase expresses that conditional rule.",
                       "Do not bind RestoreToBase (free full heal) or blindly add a base difference. Add the narrowly specified conditional variant; do not consult vital policy for Mining/Cooking-only level gains.",
                       apply=False, proposed_parent_variant="raise_if_at_old_base_otherwise_preserve",
                       rule_classification="source_supported_inference",
                       oracle={"old_base": 10, "new_base": 11, "current_to_expected": {"5": 5, "10": 11, "15": 15}},
                       primary_route_impact="Required Cook/Mining level gains are not vital-level gains; defer this lookup until actual HP/Prayer level gain, without promising the full level-up contract.")
        elif ".navigation.step_ticks" in path:
            resolution(row, "npc_walk_step", "source_supported_inference", "u32", 1,
                       refs("code.rsmod.NpcMovementProcessor", "code.rsmod.MoveSpeed", "code.rsmod.NpcWanderModeProcessor", "Pathfinding"),
                       "Mobile NPC walking processes at most one legal tile per server tick. This is movement cadence, not a requirement to pick a fresh random destination or move every tick. The contemporary source separates a1/8 idle wander decision from walking.",
                       "Unblocks path-following/chasing and ordinary movement. Do not turn a one-tick step into continuous forced wandering, expand current radii, move scenery-bound actors, or remove source clipping.",
                       existing_wander_radius=content["npcs"][index[path][0][1]]["navigation"]["wander_radius"],
                       unresolved_precision="Idle wander radii remain the existing explicit anchors/candidates; they are not verified by a step-timing binding.",
                       suggested_separate_wander_decision={"numerator": 1, "denominator": 8, "classification": "source_supported_inference"})
        elif ".combat.mechanics.damage" in path:
            resolution(row, "npc_zero_inclusive_damage", "known_fact", "DamagePolicy",
                       {"successful_minimum": 0, "cap_to_remaining_hitpoints": True},
                       refs("code.weirdgloop.NPCVsPlayerCalc", "code.weirdgloop.HitDist",
                            "Giant rat (Tutorial Island)", "Chicken (Tutorial Island)", "Goblin"),
                       "The wiki-maintained incoming-damage calculator constructs the NPC distribution uniformly from0 through max inclusive. Do not apply the post-rebalance player's minimum-successful1 to NPCs. Tutorial chicken max0 must stay0.",
                       "Goblin/rat successful accuracy rolls may still deal0; chicken can never deal1. Tutorial nonfatal player-HP protection is an additional cap, not an NPC damage boost.")
        elif path.endswith(".loot[3]"):
            resolution(row, "goblin_modern_supplement", "owner_decision", "LootPool", None,
                       refs("Goblin", "Energy potion", "Update:Summer Sweep Up - Hunter & Skilling", "Drops"),
                       "The official2026-08-19 update explicitly adds1-to4-dose energy potions at a 'fairly common' rate to goblins. The energy-potion article's1-to3 summary conflicts with that primary update. Neither publishes a numeric probability; the old numeric primary table already sums128.",
                       "Do not bind an empty pool, move all potion drops behind a members guard, or claim bones/coins-only goblin loot is current. One material ordinary-route probability/eligibility choice remains.",
                       apply=False, decisions=[
                           {"option": "retain_exact_source_gate", "consequence": "Goblins cannot claim complete current loot fidelity until a public quantitative drop table/log supplies supplement rate/dose distribution. Combat formulas and all other resolved rules remain usable."},
                           {"option": "explicitly_accept_provisional_supplement", "candidate": {"independent_of_primary": True, "event_chance": {"numerator": 1, "denominator": 16},
                                                                                          "conditional_dose_weights": {"1": 1, "2": 1, "3": 1, "4": 1}},
                            "consequence": "A transparent provisional6.25% extra-potion event, not a verified source rate; needs an explicit decision rather than silently changing monetary drops. Keep the128-weight primary table intact.",
                            "not_applied": True},
                       ],
                       required_parent_content=["Bind source energy potion(3), item3010, if dose3 is enabled; it is absent from the current content.",
                                                "Preserve beginner-clue and member-only tertiary candidates separately with ownership/eligibility checks; do not use this as permission to remove them.",
                                                "Do not gate ordinary F2P potion drops as member-only."],
                       approval_requested_for="Only accepting a quantitatively unsupported live loot distribution, not ordinary engineering defaults.",
                       source_account_required=False)
        elif path.endswith(".combat.mechanics.respawn"):
            chicken = "tutorial_chicken" in path
            resolution(row, "tutorial_chicken_respawn" if chicken else "tutorial_rat_respawn",
                       "source_supported_inference", "TickDuration", {"kind": "fixed", "ticks": 25 if chicken else 30},
                       refs("Chicken" if chicken else "Giant rat", "Chicken (Tutorial Island)" if chicken else "Giant rat (Tutorial Island)"),
                       "Use the ordinary species' published respawn baseline (chicken25 ticks, giant rat30) for the tutorial variant until contrary variant-specific timing is found. Do not claim the tutorial page explicitly gives this parameter.",
                       "Finite ordinary-rate respawn, not instant replenishment or a forced kill. Refine only these two parameters from better public variant evidence.")
        elif ".cadence.first" in path:
            resolution(row, "bread_make_x_first", "source_supported_inference", "u32", 3,
                       refs("Bread", "Cooking", "Game tick/Action lengths"),
                       "Bread recipe states1 tick single and4 using Make-X; the shared cooking queue documents first3, then4. Use3 for first queued bread as the same production family, without changing success/XP.",
                       "Only first queued-item phase is inferred. Keep single bread1, subsequent4 and menu-delay0 as already represented; do not add another menu tick twice.")
        elif ".cadence.single" in path:
            resolution(row, "shrimp_single_cook", "source_supported_inference", "u32", 1,
                       refs("Cooking", "Shrimps", "Bread", "Game tick/Action lengths"),
                       "Cooking's2-tick method explains that one suitable raw item skips the selection interface and cooks without queued delay; use the discrete one-tick single-item conversion convention also explicit for bread. Make-X remains first3/repeat4.",
                       "This is single-item handling, not a1-tick bulk cooking boost. Fire/range selection and walking/interaction cost remain separate; no success guarantee is introduced.")
        elif ".pricing.overstock" in path:
            resolution(row, "shop_linear_overstock", "source_supported_inference", "OverstockPricing",
                       "linear_to_clamp", refs("Shop", "General store", "Module:Shop calculator",
                                               "code.rsmod.StandardGpCostCalculations", "code.rsmod.StandardGpCostCalculationBuyTest"),
                       "Prefer the Shop article's worked stock-delta example, corroborated by contemporary shop cost code, over the calculator module's conflicting division branch. The code applies the same signed stock difference above and below base stock.",
                       "No owner redesign is needed to choose the documented linear interpretation. Preserve per-unit repricing, source caps/minimum prices and finite stock; mark it inferred, not live-observed.")
        elif ".restock.phase" in path:
            resolution(row, "shop_world_epoch_restock", "source_supported_inference", "RestockPhase",
                       {"kind": "world_epoch"}, refs("code.rsmod.ShopRestockProcess", "code.rsmod.ShopRestockScript", "General store"),
                       "Contemporary restock code checks mapClock % restockRate==0 per modified line, including unstocked normalization100. Buying again does not restart an item's countdown.",
                       "Preserve the world tick epoch over server restart; run each boundary once. Transaction/stock version checks remain mandatory. This is corroborating code, not a Jagex server observation.")
        elif ".fishing_spot." in path and path.endswith(".respawn"):
            rule = index[path][0]
            resolution(row, "nondepleting_fishing_no_respawn", "inactive_dependency", "TickDuration", None,
                       refs("Fishing spot (small net, bait)", "Transcript:Learning the Ropes"),
                       "The integrated source gather rule has depletion chance0/1. A depletion-respawn timer is therefore never evaluated for these spots. Do not invent a1-tick/zero-tick water respawn just to make a global readiness scan green.",
                       "Ordinary shrimp fishing is usable without a nonexistent respawn timer. Source fishing-spot relocation, if separately enabled, is a distinct mechanic and remains in full scope.",
                       apply=False, inactive_proof={"depletion": {"numerator": 0, "denominator": 1}, "relocation_is_separate": True},
                       parent_action="Guard respawn.require() behind actual depletion; preserve this inactive binding for future nonzero-depletion variants.")
        else:
            raise ValueError("Unclassified exact source path: " + path)

    assert len(resolutions) == len(unresolved) == 112
    counts = Counter(r["classification"] for r in resolutions.values())
    profile_paths = [
        ("initial_state.tile", "source_supported_inference",
         "Retain the existing starting-house tile3094,3106. It is a source-area starting candidate, not an observed exact account spawn/camera.",
         refs("Learning the Ropes", "Tutorial Island")),
        ("mechanics.travels.travel.tutorial.departure.destination", "source_supported_inference",
         "Retain the current experience branches: brand-new/returning normal accounts south of Jon at3232,3233; experienced at3222,3218 in the castle area. Area/branch is documented; exact tile is a bounded existing candidate.",
         refs("Tutorial Island", "Lumbridge Home Teleport", "Transcript:Learning the Ropes")),
        ("mechanics.death.respawn", "source_supported_inference",
         "Retain the source-area Lumbridge respawn candidate. Do not claim one deterministic tile is the source's whole respawn area.",
         refs("code.rsmod.PlayerDeath", "Death")),
        ("mechanics.death.first_office", "source_supported_inference",
         "Retain separate walkable player arrival3174,5726 rather than placing the player on scenery-bound Death. Exact arrival pixel/camera is not established.",
         refs("Death's Office", "Transcript:Death (NPC)")),
        ("initial_state.runtime.settings.run_enabled", "source_supported_inference",
         "Retain explicit run-off until the lesson/user toggles it; do not turn off the actual run-energy system. The new-player update does not specify this toggle.",
         refs("Transcript:Learning the Ropes", "Energy")),
        ("initial_state.runtime.settings.auto_retaliate", "source_supported_inference",
         "Retain explicit on as a starter profile inference. The source documents the control/retaliation timing, not the fresh-account default.",
         refs("Auto Retaliate")),
    ]
    write("profile-resolutions.json", {
        "schema_version": 1, "scope": "Supplemental already-bound/profile fields; not additional members of the112 unresolved-path inventory.",
        "retained": [{
            "path": path, "pointer": list(index[path][0]), "value": index[path][1],
            "classification": classification, "reason": reason, "source_refs": ids,
            "observed_source_state": False,
        } for path, classification, reason, ids in profile_paths],
        "corrected_via_coupled_updates": ["initial_death_auto_equip", "initial_death_supply_piles"],
        "new_player_update_facts": {
            "source_refs": refs("Update:Grid Master Rewards, Poll & New Player Improvements",
                                "Update:New Player Improvements Round 2"),
            "classification": "known_fact",
            "all_players": {"afk_minutes": 25, "death_auto_equip": True,
                            "player_attack_options": "always_right_click",
                            "npc_attack_options": "left_click_where_available",
                            "hide_roofs": True, "rune_picking": True, "ammo_picking": True,
                            "music_percent": 10, "sound_percent": 20, "area_sound_percent": 15,
                            "source_default_layout": "resizable_modern"},
            "quest_tracking": "Quest Guide unlocks Learning the Ropes in Quest List; Wind Strike on chicken completes it and pays1QP.",
            "presentation_constraint": "Section30 explicitly chooses Resizable-Classic for M1. Do not switch the product to Modern or self-approve another presentation layout because the default list says Modern.",
        },
        "world_binding_note": "Retaining the integrated source-area landing candidates is a localized inference, not a request to create a source account or permission to relocate source areas.",
    })
    write("resolutions.json", {
        "schema_version": 1, "task": "m1-runtime-source-bindings",
        "base": "e9073ab", "profile": "ordinary_normal_f2p_tutorial_lumbridge_cooks_death",
        "created_at": max(s["retrieved_at"] for s in sources),
        "inputs": {
            "unresolved_path": "research/m1-bindings/unresolved-bindings.json", "unresolved_sha256": digest(INPUT),
            "content_path": "content/m1/game-content.json.gz", "content_sha256": digest(CONTENT),
            "types_sha256": digest(ROOT / "crates/game-types/src/mechanics.rs"),
            "parent_numeric_key_fix": "7150464",
        },
        "classification_meanings": {
            "known_fact": "Explicit published source/data/calculator-model fact; not a live-server observation.",
            "source_supported_inference": "Usable reversible engineering binding with stated source/rationale/domain; never promoted to verified observation or owner approval.",
            "codebug_parent_fixed": "Existing known value unblocked by a parent fix already in base.",
            "codebug_parent_required": "Narrow required behavior cannot be expressed by a current enum; precise proposed variant/oracle supplied.",
            "inactive_dependency": "Unreachable or alternative-only in the ordinary route under stated guards; original definition and full-target requirement remain.",
            "owner_decision": "Only a quantitatively unsupported material live behavior choice remains, with explicit alternatives/consequences.",
        },
        "counts": dict(sorted(counts.items())), "path_count": 112,
        "resolutions": resolutions, "coupled_updates": additions,
        "ordinary_readiness": {
            "applyable_path_bindings": sum(r["apply_to_ordinary_profile"] for r in resolutions.values()),
            "inactive_bindings_require_scope_aware_validation": True,
            "material_decisions": ["npcs.npc.goblin.level_2.combat.mechanics.loot[3]"],
            "narrow_parent_type_work": ["mechanics.vitals.level_up"],
            "note": "Source disposition coverage is complete. Current loot remains the primary ordinary-journey gate; HP/Prayer-only level-up needs the specified conditional variant. No server/browser/presentation acceptance is claimed.",
        },
        "original_evidence_modified": False,
        "gameplay_executed": False, "presentation_approved": False,
    })
    print(json.dumps({"paths": len(resolutions), "groups": len({r["rule_group"] for r in resolutions.values()}),
                      "classifications": counts, "coupled_updates": len(additions)}))


if __name__ == "__main__":
    main()
