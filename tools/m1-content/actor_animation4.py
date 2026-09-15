"""Source-scoped actor animation bindings; no scene-neighbour or numeric fallback selection."""
from common import ROOT, canonical, load, sha, source_record

INPUT = ROOT / "research/interface-contracts/animation-authority/inputs.json"


def bind_actor_animations(content):
    inputs = load(INPUT)
    native = inputs["native"]["sequences"]
    original = load(ROOT / "research/interface-contracts/actor-observer-bindings.json")
    used = set()

    def evidence(notes, status="verified_reference", fragment=""):
        return [source_record(str(INPUT.relative_to(ROOT)) + fragment, notes, status, sha(INPUT.read_bytes()))]

    def sequence(number, notes, status="verified_reference"):
        if str(number) not in native:
            raise ValueError(f"Missing exact source sequence metadata: {number}")
        used.add(number)
        return {"rule": {"kind": "sequence", "sequence": number},
                "source": evidence(notes, status, f"#native.sequences.{number}")}

    def unknown(reason):
        return {"rule": {"kind": "unverified", "reason": reason},
                "source": evidence("Explicit remaining source qualification, not silence or a fallback.", "inference")}

    actions = {method: sequence(number, f"Original named source motion for the exact method {method}.")
               for method, number in original["action_sequences"].items()}
    actions["action.life.death"] = sequence(836, "Original HUMAN_DEATH836; hold only through actual source dying/respawn life phases.")
    recipes = {}
    for id, recipe in content["recipes"].items():
        number = original["recipe_sequences"].get(id) or original["action_sequences"].get(recipe["mechanics"]["method"])
        if number is not None:
            recipes[id] = sequence(number, f"Exact source recipe/method binding for {id}, not an adjacent object.")
        elif id == "recipe.cooking.dough":
            recipes[id] = unknown(inputs["unknowns"][id])
        else:
            raise ValueError(f"New source recipe requires animation qualification: {id}")

    styles = {
        "style.unarmed.punch": sequence(422, "Original HUMAN_UNARMEDPUNCH; actual Punch attack.", "inference"),
        "style.unarmed.kick": sequence(423, "Original HUMAN_UNARMEDKICK; actual Kick attack.", "inference"),
        "style.unarmed.block": sequence(422, "Defensive Block is a source attack style, not a block-reaction event; its attack uses the unarmed punch.", "inference"),
        "style.magic.wind_strike": sequence(711, "Original HUMAN_CASTSTRIKE711 and the pinned Wind Strike source crop."),
    }
    weapon_types = {
        1351: {"slash": 395, "crush": 401},
        1265: {"stab": 400, "crush": 401},
        1205: {"stab": 386, "slash": 390},
        1277: {"stab": 386, "slash": 390},
        1237: {"stab": 428, "slash": 440, "crush": 429},
        841: {"ranged": 426},
    }
    weapons = {}
    for item, definition in content["items"].items():
        equipment = definition["equipment"]
        if not equipment or not equipment.get("weapon"):
            continue
        source_id = definition["source_id"]
        if source_id not in weapon_types:
            raise ValueError("Expanded M1 weapon universe requires a source motion binding")
        choices = {}
        for id in equipment["weapon"]["styles"]:
            attack_type = content["mechanics"]["combat_styles"][id]["attack_type"]
            if attack_type not in weapon_types[source_id]:
                raise ValueError("M1 style changed source attack type")
            number = weapon_types[source_id][attack_type]
            styles[id] = sequence(number, f"Qualified source dispatch for item{source_id}/{attack_type}: original named "
                f"sequence {native[str(number)]['symbol']}, native source weapon/style identity and "
                "pinned secondary dispatch corroboration. Do not copy inconsistent secondary numeric style indexes "
                "or select from a UI label/nearby actor. This is not a live Jagex packet observation.", "inference")
            choices[id] = number
        weapons[item] = choices
    if set(styles) != set(content["mechanics"]["combat_styles"]):
        raise ValueError("Every actual legal M1 style needs an explicit source rule")
    phases = [{"at_tick": phase["at_tick"], "sequence": phase["sequence"]}
              for phase in inputs["home_teleport"]["actor_phases"]]
    used.update(phase["sequence"] for phase in phases)
    spells = {
        "spell.wind_strike": sequence(711, "Original HUMAN_CASTSTRIKE711; actual committed spell launch, never nearby combat."),
        "spell.lumbridge_home_teleport": {
            "rule": {"kind": "channel", "duration_ticks": 24, "phases": phases},
            "source": evidence(inputs["home_teleport"]["qualification"], "inference", "#home_teleport"),
        },
    }
    if set(spells) != set(content["mechanics"]["spells"]):
        raise ValueError("Expanded spell universe requires source animation qualification")
    for item, definition in content["items"].items():
        if definition["healing"]:
            if definition["source_id"] not in (315, 2309):
                raise ValueError("A new edible item needs its source animation/adaptation binding")
            id = "action.item." + item.removeprefix("item.") + ".eat"
            actions[id] = sequence(12526, "Existing owner-approved ordinary shrimp/bread source12526 eating binding; "
                "actual committed consumption only, with the original73-cycle pose and no new gameplay delay.",
                "approved_adaptation")
    for item, operations in content["ui"]["item_actions"].items():
        for operation in operations:
            id = "action.item." + item.removeprefix("item.") + "." + operation["id"]
            if operation["action"]["kind"] == "drink":
                actions[id] = sequence(829, "Qualified ordinary drink/eat-motion reuse: native HUMAN_EAT829 "
                    "provides the hand-to-mouth consumption motion. This does not bind special rum/tea animations "
                    "or alter the source drink, food, combat or movement timers.", "inference")
            elif operation["action"]["kind"] == "empty":
                actions[id] = unknown(inputs["unknowns"]["empty_container"])
    sequences = {
        str(number): {"duration_cycles": native[str(number)]["frame_length_sum"],
                      "source": evidence(f"Exact cache2695 sequence{number} frame lengths and terminal hold frames; "
                          "source duration is not a gameplay action cooldown.", fragment=f"#native.sequences.{number}")}
        for number in sorted(used)
    }
    definition = {
        "version": 1, "sequences": sequences, "actions": actions, "recipes": recipes,
        "styles": styles, "spells": spells,
        "source": evidence("Bounded M1 actor motion authority; qualified dispatch/phase inferences remain explicit. "
                           "No new avatar, source gameplay rule, cadence, stock or RNG."),
    }
    proof = {
        "schema_version": 1, "input_sha256": sha(INPUT.read_bytes()), "required_sequences": sorted(used),
        "weapon_styles": weapons, "home_teleport_phases": phases, "channel_ticks_unchanged": 24,
        "unknowns": inputs["unknowns"], "all_legal_styles_bound": True,
        "source_binding_sha256": sha(canonical(definition)),
        "worlds_repin_attempted": False, "avatar_or_source_assets_modified": False, "milestone_accepted": False,
    }
    return definition, proof
