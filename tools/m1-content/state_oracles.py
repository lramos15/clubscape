"""Strict post-event reference-state checks for authored data, not a production game executor."""

from bisect import bisect_right
from copy import deepcopy

from common import counter_value, item_stack


class OracleRefusal(ValueError):
    pass


class OracleUnresolved(OracleRefusal):
    pass


class Oracle:
    def __init__(self, content):
        self.content = content
        self.data = deepcopy(content["initial_state"])
        self.data.update({"entitlements": {}, "death_topics": set(), "life": "alive"})
        self.data["world_counters"] = {key: deepcopy(value["initial"]) for key, value in content["mechanics"]["counters"].items()
                                       if value["scope"] == "world"}
        self.data["object_states"] = {key: value["initial"] for key, value in content["mechanics"]["object_transforms"].items()}

    def count(self, item, scope="inventory"):
        inventory = sum(stack["quantity"] for stack in self.data["inventory"]["slots"] if stack and stack["item"] == item)
        equipment = sum(stack["quantity"] for stack in self.data["equipment"].values() if stack["item"] == item)
        bank = sum(stack["quantity"] for stack in self.data["bank"]["slots"] if stack and stack["item"] == item)
        return {"inventory": inventory, "equipment": equipment, "inventory_and_equipment": inventory + equipment, "bank": bank}[scope]

    def counter(self, identifier):
        definition = self.content["mechanics"]["counters"][identifier]
        return (self.data["runtime"]["counters"] if definition["scope"] == "character" else self.data["world_counters"])[identifier]

    def free(self, container="inventory"):
        if container == "inventory":
            return self.data["inventory"]["slots"].count(None)
        if container == "bank":
            return self.data["bank"]["capacity"] - sum(stack is not None for stack in self.data["bank"]["slots"])
        raise OracleRefusal("Unmodeled capacity scope")

    def grant_claimed(self, identifier):
        value = self.data["entitlements"].get(identifier)
        return value is not None and (value.get("claimed", False) or value.get("complete", False))

    def guard(self, guard, event=None):
        kind = guard["kind"]
        if kind == "always":
            return True
        if kind == "all":
            return all(self.guard(value, event) for value in guard["guards"])
        if kind == "any":
            return any(self.guard(value, event) for value in guard["guards"])
        if kind == "not":
            return not self.guard(guard["guard"], event)
        if kind == "tutorial_stage":
            return self.data["tutorial_stage"] == guard["stage"]
        if kind == "quest_stage":
            return self.data["quests"][guard["quest"]]["stage"] == guard["stage"]
        if kind in ("has_items", "owns_items"):
            return all(self.count(stack["item"], guard.get("scope", "inventory")) >= stack["quantity"] for stack in guard["items"])
        if kind == "equipped":
            return self.count(guard["item"], "equipment") > 0
        if kind == "free_capacity":
            return self.free(guard["container"]) >= guard["slots"]
        if kind == "entitlement_claimed":
            return self.grant_claimed(guard["entitlement"])
        if kind == "counter":
            value, predicate = self.counter(guard["counter"]), guard["predicate"]
            if predicate["kind"] == "equals":
                return value == predicate["value"]
            return value["type"] == "integer" and predicate["minimum"] <= value["value"] <= predicate["maximum"]
        if kind == "within":
            point, target = self.data["tile"], guard["tile"]
            return point["plane"] == target["plane"] and max(abs(point["x"] - target["x"]), abs(point["y"] - target["y"])) <= guard["distance"]
        if kind == "experience":
            return self.data["runtime"]["settings"]["experience"] == guard["experience"]
        if kind == "setting":
            name = {"run": "run_enabled", "auto_retaliate": "auto_retaliate", "death_auto_equip": "death_auto_equip",
                    "death_supply_piles": "death_supply_piles"}[guard["setting"]["setting"]]
            return self.data["runtime"]["settings"][name] == guard["setting"]["enabled"]
        if kind == "interface_unlocked":
            return guard["interface"] in self.data["interfaces"]
        if kind == "skill_at_least":
            requirement = guard["requirement"]
            state = self.data["skills"][requirement["skill"]]
            level = state["current_level"] if requirement["basis"] == "current" else bisect_right(
                self.content["skills"][requirement["skill"]]["xp_thresholds_tenths"], state["xp_tenths"])
            return level >= requirement["level"]
        if kind == "life":
            return self.data["life"] == guard["phase"]
        if kind == "death_topics":
            return self.data["death_topics"].issuperset(guard["topics"])
        if kind == "members_world":
            return self.content["mechanics"]["world_members"] is True
        if kind == "event":
            return self.event_condition(guard["condition"], event)
        if kind == "charges":
            return any(stack and stack["item"] == guard["item"] and stack.get("instance") and
                       stack["instance"]["charges"]["kind"] == guard["charge_kind"] and
                       stack["instance"]["charges"]["remaining"] >= guard["minimum"]
                       for stack in self.data["inventory"]["slots"])
        raise OracleRefusal(f"Oracle does not silently accept guard {kind}")

    @staticmethod
    def event_condition(condition, event):
        if event is None:
            return False
        names = {"interaction": "interacted", "dialogue_choice": "dialogue_selected", "production": "production_resolved",
                 "combat": "combat_resolved", "kill": "npc_killed", "spell": "spell_resolved",
                 "interface": "interface_presented", "teleport": "teleport", "setting": "setting_changed",
                 "inspection": "inspected", "death": "death_occurred", "recovery": "recovery_completed",
                 "death_topic": "death_topic_completed"}
        kind = condition["kind"]
        if event["kind"] != names[kind]:
            return False
        for field, expected in condition.items():
            if field == "kind":
                continue
            if field == "output":
                if expected and not any(stack["item"] == expected for stack in event["outputs"]):
                    return False
            elif field == "outcomes":
                if event["outcome"] not in expected:
                    return False
            elif field == "facility":
                if expected is not None and event["facility"] != {"kind": "spawn", "spawn": expected}:
                    return False
            elif field == "target" and expected is None:
                continue
            elif field == "phase":
                if event["phase"]["kind"] != expected:
                    return False
            elif event.get(field) != expected:
                return False
        return True

    def give(self, item, quantity, container="inventory"):
        definition = self.content["items"][item]
        if definition["charges"] is not None or isinstance(definition["stackable"], dict):
            raise OracleUnresolved("Ordinary quantity mutation cannot invent charged/conditional instances")
        if quantity <= 0:
            raise OracleRefusal("Positive quantities required")
        slots = self.data[container]["slots"]
        stacked = container == "bank" or definition["stackable"]
        if stacked:
            for stack in slots:
                if stack and stack["item"] == item:
                    if stack["quantity"] + quantity > 2147483647:
                        raise OracleRefusal("Stack overflow")
                    stack["quantity"] += quantity
                    return
        needed = 1 if stacked else quantity
        if needed > self.free(container):
            raise OracleRefusal("Container full")
        for _ in range(needed):
            stack = item_stack(item, quantity if stacked else 1)
            if None in slots:
                slots[slots.index(None)] = stack
            else:
                slots.append(stack)

    def take(self, item, quantity, container="inventory"):
        if self.count(item, container) < quantity:
            raise OracleRefusal("Not owned")
        slots = self.data[container]["slots"]
        for index, stack in enumerate(slots):
            if not stack or stack["item"] != item:
                continue
            amount = min(quantity, stack["quantity"])
            stack["quantity"] -= amount
            quantity -= amount
            if stack["quantity"] == 0:
                slots[index] = None
            if quantity == 0:
                break

    def grant(self, identifier):
        definition = self.content["mechanics"]["grants"][identifier]
        claim = (self.data["entitlements"].setdefault(definition["entitlement"],
                 {"delivered": {}, "satisfied": set(), "complete": False}) if definition["entitlement"] else
                 {"delivered": {}, "satisfied": set(), "complete": False})
        for line in definition["lines"]:
            item = line["item"]
            if item in claim["satisfied"]:
                continue
            owned = self.count(item, line["ownership"])
            wanted = (line["quantity"] if line["mode"] == "add" else
                      (line["quantity"] if owned == 0 else 0) if line["mode"] == "missing_only" else
                      max(0, line["quantity"] - owned))
            if wanted:
                try:
                    self.give(item, wanted, definition["target"])
                except OracleRefusal:
                    if definition["capacity"] == "ordered_partial":
                        break
                    raise
                claim["delivered"][item] = claim["delivered"].get(item, 0) + wanted
            claim["satisfied"].add(item)
        claim["complete"] = len(claim["satisfied"]) == len(definition["lines"])

    def effects(self, effects, event=None):
        saved = deepcopy(self.data)
        try:
            self._effects(effects, event)
        except OracleRefusal:
            self.data = saved
            raise

    def _effects(self, effects, event):
        for effect in effects:
            kind = effect["kind"]
            if kind in ("give_items", "take_items"):
                for stack in effect["items"]:
                    (self.give if kind == "give_items" else self.take)(stack["item"], stack["quantity"])
            elif kind == "grant":
                self.grant(effect["grant"])
            elif kind == "once":
                if self.grant_claimed(effect["entitlement"]):
                    raise OracleRefusal("Entitlement already claimed")
                self._effects(effect["effects"], event)
                self.data["entitlements"][effect["entitlement"]] = {"claimed": True}
            elif kind == "conditional":
                if self.guard(effect["guard"], event):
                    self._effects(effect["effects"], event)
            elif kind == "set_tutorial_stage":
                self.data["tutorial_stage"] = effect["stage"]
            elif kind == "set_quest_stage":
                self.data["quests"][effect["quest"]]["stage"] = effect["stage"]
            elif kind == "unlock_interface":
                if effect["interface"] not in self.data["interfaces"]:
                    self.data["interfaces"].append(effect["interface"])
            elif kind == "add_quest_points":
                self.data["quest_points"] += effect["amount"]
            elif kind == "restore_vital":
                field = {"hitpoints": "hitpoints", "prayer": "prayer_points", "run_energy": "run_energy"}[effect["vital"]]
                maximum = 10000 if field == "run_energy" else self.data["skills"]["skill.hitpoints" if field == "hitpoints" else "skill.prayer"]["current_level"]
                restore = effect["restoration"]
                self.data[field] = (maximum if restore["kind"] == "to_base_maximum" else
                                    restore["amount"] if restore["kind"] == "set" else min(maximum, self.data[field] + restore["amount"]))
            elif kind == "award_xp":
                stage = self.content["tutorial"][self.data["tutorial_stage"]]
                for reward in effect["rewards"]:
                    skill, amount = reward["skill"], reward["amount_tenths"]
                    state = self.data["skills"][skill]
                    thresholds = self.content["skills"][skill]["xp_thresholds_tenths"]
                    if bisect_right(thresholds, state["xp_tenths"]) >= stage["xp_stop_levels"].get(skill, 65535):
                        continue
                    maximum = min(self.content["skills"][skill]["maximum_xp_tenths"], stage["xp_caps_tenths"].get(skill, 2**64 - 1))
                    state["xp_tenths"] += min(amount, max(0, maximum - state["xp_tenths"]))
                    state["current_level"] = bisect_right(thresholds, state["xp_tenths"])
            elif kind in ("set_counter", "add_counter"):
                definition = self.content["mechanics"]["counters"][effect["counter"]]
                value = effect["value"] if kind == "set_counter" else counter_value(self.counter(effect["counter"])["value"] + effect["delta"])
                schema = definition["value_type"]
                if value["type"] != schema["type"] or (schema["type"] == "integer" and not schema["minimum"] <= value["value"] <= schema["maximum"]):
                    raise OracleRefusal("Counter overflow or type mismatch")
                target = self.data["runtime"]["counters"] if definition["scope"] == "character" else self.data["world_counters"]
                target[effect["counter"]] = deepcopy(value)
            elif kind == "complete_death_topic":
                self.data["death_topics"].add(effect["topic"])
            elif kind == "transform_object":
                if effect["state"] not in self.content["mechanics"]["object_transforms"][effect["transform"]]["states"]:
                    raise OracleRefusal("Unknown transform state")
                self.data["object_states"][effect["transform"]] = effect["state"]
            elif kind in ("message", "inspect"):
                continue
            elif kind in ("travel_via", "reconcile_containers"):
                raise OracleUnresolved("Reference effects do not bypass source-unresolved transport or reconciliation")
            else:
                raise OracleRefusal(f"Oracle does not silently implement {kind}")

    def event(self, event):
        stage = self.content["tutorial"][self.data["tutorial_stage"]]
        matches = [transition for transition in stage["transitions"] if transition["event"] == event["kind"] and
                   (transition["target"] is None or transition["target"] == primary_target(event)) and self.guard(transition["guard"], event)]
        if len(matches) > 1:
            raise OracleRefusal("Ambiguous source transition")
        if matches:
            self.effects(matches[0]["effects"], event)
        return bool(matches)


def primary_target(event):
    fields = {"dialogue_selected": "speaker", "interface_opened": "interface", "interface_closed": "interface",
              "interface_presented": "interface", "experience_selected": "experience", "gathered": "target",
              "equipped": "slot", "production_resolved": "recipe", "spell_resolved": "spell",
              "teleport": "travel", "temporary_object_created": "definition", "inspected": "target",
              "npc_killed": "target"}
    return event.get(fields[event["kind"]]) if event["kind"] in fields else None
