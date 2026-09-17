"""Disambiguate source questions and recovery after frozen binding application."""

from copy import deepcopy

from common import all_of, canonical, item_stack, sha


def normalize_tutorial_choices(content):
    plans = []
    for dialogue in content["dialogues"].values():
        nodes = {node["id"]: node for node in dialogue["nodes"]}
        if len(nodes) != len(dialogue["nodes"]):
            raise ValueError("Source dialogue node identities must be unique")
        if len(set(dialogue["entry_nodes"])) != len(dialogue["entry_nodes"]):
            raise ValueError("Source dialogue entry identities must be unique")
        groups = {}
        for key in dialogue["entry_nodes"]:
            if key not in nodes:
                raise ValueError("Source dialogue entry must name an existing node")
            if key.startswith("transition.tutorial."):
                node = nodes[key]
                groups.setdefault(canonical(node["guard"]), []).append(node)
        for guard, group in groups.items():
            if len(group) < 2:
                continue
            ids = {node["id"] for node in group}
            if any(set(node) != {"id", "text", "guard", "choices"} for node in group):
                raise ValueError("Source question normalization cannot discard node fields")
            if len({node["text"] for node in group}) != 1:
                raise ValueError("Equal-guard source entries have different questions")
            if any(not node["choices"] for node in group):
                raise ValueError("Source question branches must retain actual choices")
            choices = [deepcopy(choice) for node in group for choice in node["choices"]]
            if len({choice["id"] for choice in choices}) != len(choices):
                raise ValueError("Source question branches have duplicate choice identities")
            if any(choice["next_node"] is not None for choice in choices):
                raise ValueError("Explicit source dialogue continuations cannot be merged")
            if any(choice["next_node"] in ids for node in dialogue["nodes"] for choice in node["choices"]):
                raise ValueError("An explicit continuation targets a source question branch")
            plans.append((dialogue, group, choices, sha(guard)))
    audit = []
    for dialogue, group, choices, guard_hash in plans:
        retained = group[0]
        removed = {node["id"] for node in group[1:]}
        retained["choices"] = choices
        dialogue["nodes"] = [node for node in dialogue["nodes"] if node["id"] not in removed]
        dialogue["entry_nodes"] = [key for key in dialogue["entry_nodes"] if key not in removed]
        audit.append({
            "dialogue": dialogue["id"], "entry": retained["id"],
            "merged_entry_ids": [node["id"] for node in group],
            "original_choice_ids": [choice["id"] for choice in choices],
            "guard_sha256": guard_hash, "question_and_choice_payloads_preserved": True,
        })
    return audit


def normalize_tutorial_recovery(content):
    audit = []
    for dialogue in content["dialogues"].values():
        recovery = [node for node in dialogue["nodes"] if node["id"].startswith(("finish.", "replace."))]
        if not recovery:
            continue
        removed = {node["id"] for node in recovery}
        if not removed.issubset(dialogue["entry_nodes"]):
            raise ValueError("Expected source supply nodes to be independent original entries")
        primary = [node["guard"] for node in dialogue["nodes"]
                   if node["id"] in dialogue["entry_nodes"] and node["id"] not in removed]
        choices = []
        for node in recovery:
            if len(node["choices"]) != 1:
                raise ValueError("Source supply node must retain its one declared grant choice")
            choice = deepcopy(node["choices"][0])
            if choice["id"] != node["id"] or choice["next_node"] is not None or len(choice["effects"]) != 1:
                raise ValueError("Source supply choice identity/effects changed")
            effect = choice["effects"][0]
            if effect["kind"] != "grant":
                raise ValueError("Supply entry normalization cannot modify another kind of effect")
            grant = content["mechanics"]["grants"][effect["grant"]]
            if node["id"].startswith("replace."):
                missing = []
                for line in grant["lines"]:
                    if line["mode"] not in ("missing_only", "top_up"):
                        raise ValueError("Source recovery must not add already owned supplies")
                    threshold = 1 if line["mode"] == "missing_only" else line["quantity"]
                    missing.append({"kind": "not", "guard": {
                        "kind": "owns_items", "scope": line["ownership"],
                        "items": [item_stack(line["item"], threshold)],
                    }})
                choice["guard"] = all_of(choice["guard"], {"kind": "any", "guards": missing})
            choices.append(choice)
        guard = all_of(
            {"kind": "not", "guard": {"kind": "any", "guards": primary}},
            {"kind": "any", "guards": [choice["guard"] for choice in choices]},
        )
        key = "tutorial_supply_recovery"
        dialogue["nodes"] = [node for node in dialogue["nodes"] if node["id"] not in removed]
        dialogue["entry_nodes"] = [node for node in dialogue["entry_nodes"] if node not in removed]
        dialogue["nodes"].append({
            "id": key, "text": "Recover unlocked tutorial supplies.", "guard": guard,
            "choices": [{**choice, "guard": all_of(guard, choice["guard"])} for choice in choices],
        })
        dialogue["entry_nodes"].append(key)
        audit.append({"dialogue": dialogue["id"], "replaced_entry_ids": sorted(removed),
                      "entry": key, "grant_choices_preserved": [choice["id"] for choice in choices]})
    return audit
