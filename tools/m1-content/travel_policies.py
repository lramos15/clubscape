"""Typed source experience arrivals, entitled departure and first-item-loss recovery policy."""

from common import (
    all_of, bound, counter_guard, location, requirement, source_record,
    tile, tutorial_at, unique_sources, unresolved,
)
from spawns import interaction, stage_from


def wire_travel_policies(inputs, world, content):
    mechanics = content["mechanics"]
    source = inputs.rule_source("rule.magic.home_teleport")
    branch_source = source + inputs.basis(inputs.rules["initial-state"]["account"]["basis"])
    arrivals = {
        "experience.brand_new": location(tile(3232, 3233)),
        "experience.returning": location(tile(3232, 3233)),
        "experience.experienced": location(tile(3222, 3218)),
    }
    for destination in arrivals.values():
        if not world.cell(**destination["tile"])["walkable"]:
            raise ValueError("Experience arrival candidate is not source-walkable")
    travel_id = "travel.tutorial.departure"
    guard = all_of(counter_guard("counter.tutorial.departure_authorized"),
                   {"kind": "not", "guard": counter_guard("counter.tutorial.departed")},
                   {"kind": "any", "guards": [tutorial_at("stage.tutorial.home_teleport"),
                                               tutorial_at("stage.tutorial.teleport_channel")]})
    mechanics["travels"][travel_id] = {
        "id": travel_id, "guard": guard,
        "destination": bound({"kind": "experience", "branches": arrivals}, branch_source + [
            source_record("research/m1-bindings/location-bindings.json",
                          "Separate player-arrival candidates in the correct Lumbridge source areas; "
                          "the Jon NPC anchor is not reused as the player tile. Exact arrival capture remains required.",
                          "inference", "240/cache2695")]),
        "channel_ticks": bound(24, source), "cooldown_ticks": bound(3000, source),
        "cooldown_start": bound("completed", source),
        "interruptions": ["movement", "combat", "another_action", "logout"],
        "completion_effects": [{"kind": "reconcile_containers", "reconciliation": "reconciliation.tutorial.departure"}],
        "source": branch_source,
    }
    mechanics["spells"]["spell.lumbridge_home_teleport"] = {
        "id": "spell.lumbridge_home_teleport", "interface": "interface.magic",
        "requirements": [requirement("skill.magic")], "guard": guard, "runes": [], "launch_xp": [],
        "action": {"kind": "teleport", "travel": travel_id}, "source": source,
    }
    wind = inputs.rule_source("rule.magic.wind_strike")
    mechanics["spells"]["spell.wind_strike"] = {
        "id": "spell.wind_strike", "interface": "interface.magic", "requirements": [requirement("skill.magic")],
        "guard": stage_from(inputs, "stage.tutorial.wind_strike", include_mainland=True),
        "runes": [{"item": "item.rune.air", "quantity": 1, "instance": None},
                  {"item": "item.rune.mind", "quantity": 1, "instance": None}],
        "launch_xp": [{"skill": "skill.magic", "amount_tenths": 55}],
        "action": {"kind": "combat", "style": "style.magic.wind_strike", "projectile": "projectile.wind_strike"},
        "source": wind,
    }
    death_source = inputs.rule_source("rule.death.first_office")
    instance = "instance_template.death.office"
    mechanics["instances"][instance] = {
        "id": instance, "chunk_size": 8,
        "chunks": [{"source_region": "region.osrs.12633", "source_origin": tile(x, y),
                    "destination_region": "region.osrs.12633", "destination_origin": tile(x, y), "quarter_turns": 0}
                   for x in (3168, 3176) for y in (5720, 5728)],
        "private_to_character": True,
        "source": death_source + [source_record(
            "assets/source/osrs/cache2695/world/12633.json.gz",
            "Private template copies original source chunks without rebasing coordinates or moving the Office into Lumbridge. "
            "This is a declared ClubScape instance mapping, not an observed original server instance ID/chunk-copy dump.",
            "inference", "240/cache2695")],
    }
    office_arrival = location(tile(3174, 5726), instance)
    if not world.cell(**office_arrival["tile"])["walkable"]:
        raise ValueError("Death Office player arrival candidate is not walkable; NPC anchor is not a landing")
    respawn = location(tile(3222, 3218))
    source = inputs.rule_source("rule.death.retention")
    mechanics["value_providers"]["value_provider.osrs.death"] = {
        "id": "value_provider.osrs.death", "method": "maximum_exchange_and_alchemy",
        "revision": "osrs-240-source-value-snapshot-unresolved",
        "values": unresolved("A pinned effective exchange/alchemy per-unit value snapshot covering every represented "
                             "item is not available. Item base_value/shop price is not a death-price fallback.", source),
        "source": source,
    }
    fees = inputs.rule_source("rule.death.reclaim")
    office = inputs.rule_source("rule.death.office_reclaim")
    repeats = inputs.rule_source("rule.death.repeat")
    mechanics["death"] = {
        "domain": "normal_unsafe_non_pvp", "value_provider": "value_provider.osrs.death",
        "retained_unskulled": 3, "protect_item_extra": 1,
        "ties": unresolved("Equal-value retained-item ordering is not captured; keep the source tie assumption visible.",
                           inputs.assumption("assumption.death_tie_break")),
        "respawn": bound(respawn, source + [source_record(
            "research/m1-bindings/location-bindings.json#location.lumbridge.castle_arrival",
            "Source-area walkable respawn candidate, not the Cook/Jon/Death NPC tile or an observed camera.",
            "inference", "240/cache2695")]),
        "first_office": bound(office_arrival, death_source + [source_record(
            "research/m1-bindings/location-bindings.json#location.death.player_arrival",
            "Player arrival is an explicit walkable source-Office candidate at3174,5726 inside the private chunk mapping; "
            "Death remains at his distinct, nonwalking3180,5727 map pin. Exact original arrival remains unobserved.",
            "inference", "240/cache2695")]),
        "restoration": unresolved("Exact HP/prayer/run restoration on death arrival and first-Office exit "
                                  "is not established by the source snapshots; do not silently refill or zero a vital.",
                                  death_source),
        "required_topics": ["fees", "timer", "kept_items"], "grave_active_ticks": 1500,
        "grave_pauses": ["first_death_office", "offline", "idle", "grave_interface"],
        "idle_after_milliseconds": 10000, "reclaim_range": 7, "require_line_of_sight": True,
        "grave_capacity": 120, "office_capacity": 120,
        "office_overflow": unresolved("Deletion order above120 Office entries is not source-bound.", office),
        "grave_fee": bound({"kind": "bands", "bands": [
            {"minimum_value": 0, "fee": 0}, {"minimum_value": 100000, "fee": 1000},
            {"minimum_value": 1000000, "fee": 10000}, {"minimum_value": 10000000, "fee": 100000}],
            "maximum_total": 500000}, fees),
        "office_fee": bound({"kind": "percentage", "free_below": 100000,
                             "rate": {"numerator": 5, "denominator": 100}, "rounding": "floor"}, office),
        "payment_order": ["coffer", "bank"], "currency": "item.coins",
        "repeat": unresolved("Old-grave location, refresh and 28-per-type excess rules are known, but the complete "
                             "restricted-resource/food/potion transfer sets and supply setting defaults are not bound.",
                             repeats),
        "source": unique_sources(source + death_source),
    }
    for name, destination, guard in (
        ("travel.death.enter", {"kind": "fixed", "location": office_arrival}, tutorial_at("stage.tutorial.mainland")),
        ("travel.death.exit_voluntary", {"kind": "fixed", "location": location(tile(3237, 3194))},
         all_of(tutorial_at("stage.tutorial.mainland"), {"kind": "life", "phase": "alive"})),
        ("travel.death.exit", {"kind": "previous_respawn"},
         all_of({"kind": "life", "phase": "first_death_office"}, counter_guard("counter.death.exit_confirmed"),
                {"kind": "death_topics", "topics": ["fees", "timer", "kept_items"]})),
    ):
        mechanics["travels"][name] = {
            "id": name, "guard": guard, "destination": bound(destination, death_source),
            "channel_ticks": unresolved("Source Office portal server transit phase is not measured.", death_source),
            "cooldown_ticks": bound(0, death_source), "cooldown_start": bound("completed", death_source),
            "interruptions": ["logout"], "completion_effects": [], "source": death_source,
        }
    for spawn in content["spawns"].values():
        obj = spawn["kind"].get("object")
        if obj == "object.death.entrance":
            spawn["interactions"] = [interaction("Enter", {"kind": "travel_via", "travel": "travel.death.enter"},
                                                 tutorial_at("stage.tutorial.mainland"))]
        elif obj == "object.death.portal":
            forced = content["mechanics"]["travels"]["travel.death.exit"]["guard"]
            voluntary = content["mechanics"]["travels"]["travel.death.exit_voluntary"]["guard"]
            spawn["interactions"] = [interaction("Use", {"kind": "effects", "effects": [
                {"kind": "conditional", "guard": forced, "effects": [{"kind": "travel_via", "travel": "travel.death.exit"}]},
                {"kind": "conditional", "guard": voluntary, "effects": [{"kind": "travel_via", "travel": "travel.death.exit_voluntary"}]},
            ]}, {"kind": "any", "guards": [forced, voluntary]})]
