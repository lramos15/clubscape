"""Source lesson/recovery entries stay exclusive without changing grant quantities."""

from copy import deepcopy
import unittest

from common import CONTENT, counter_value, item_stack, load
from dialogue_entries import normalize_tutorial_recovery
from progression import completed_counter
from state_oracles import Oracle, OracleRefusal


class DialogueEntryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        normalize_tutorial_recovery(cls.content)

    def model(self, stage="stage.tutorial.survival_tools"):
        model = Oracle(self.content)
        model.data["tutorial_stage"] = stage
        model.data["runtime"]["counters"][completed_counter("transition.tutorial.net")] = counter_value(True)
        model.data["entitlements"]["entitlement.tutorial.net"] = {"claimed": True}
        return model

    def entries(self, model, npc="survival_expert"):
        dialogue = self.content["dialogues"]["dialogue." + npc]
        return [node for node in dialogue["nodes"]
                if node["id"] in dialogue["entry_nodes"] and model.guard(node["guard"])]

    def choices(self, model, npc="survival_expert"):
        entries = self.entries(model, npc)
        self.assertLessEqual(len(entries), 1)
        return [] if not entries else [choice for choice in entries[0]["choices"]
                                      if model.guard(choice["guard"])]

    def test_owned_net_does_not_overlap_the_next_source_lesson(self):
        model = self.model()
        model.give("item.fishing_net.small", 1)
        model.give("item.shrimps.raw", 1)
        self.assertEqual([node["id"] for node in self.entries(model)], ["transition.tutorial.survival_tools"])
        self.assertEqual([choice["id"] for choice in self.choices(model)], ["woodcutting_firemaking_intro"])

    def test_primary_lesson_has_precedence_even_when_an_earlier_tool_is_missing(self):
        model = self.model()
        self.assertEqual([node["id"] for node in self.entries(model)], ["transition.tutorial.survival_tools"])

    def test_missing_net_recovers_once_without_an_extra_initial_grant(self):
        model = self.model("stage.tutorial.catch_shrimp")
        choice = self.choices(model)[0]
        self.assertEqual(choice["id"], "replace.net")
        before_stage = model.data["tutorial_stage"]
        model.effects(choice["effects"])
        self.assertEqual(model.count("item.fishing_net.small"), 1)
        self.assertEqual(model.data["tutorial_stage"], before_stage)
        self.assertEqual(self.choices(model), [])

    def test_multiple_missing_unlocked_supplies_have_one_entry_and_separate_exact_grants(self):
        model = self.model("stage.tutorial.cut_logs")
        model.data["runtime"]["counters"][completed_counter("transition.tutorial.survival_tools")] = counter_value(True)
        model.data["entitlements"]["entitlement.tutorial.survival_tools"] = {"claimed": True}
        self.assertEqual([node["id"] for node in self.entries(model)], ["tutorial_supply_recovery"])
        choices = {choice["id"]: choice for choice in self.choices(model)}
        self.assertEqual(set(choices), {"replace.net", "replace.survival_tools"})
        model.effects(choices["replace.net"]["effects"])
        model.effects(choices["replace.survival_tools"]["effects"])
        self.assertEqual([model.count(item) for item in (
            "item.fishing_net.small", "item.axe.bronze", "item.tinderbox")], [1, 1, 1])
        self.assertEqual(self.choices(model), [])

    def test_recovery_counts_equipment_and_does_not_bypass_full_inventory(self):
        model = self.model("stage.tutorial.cut_logs")
        model.data["runtime"]["counters"][completed_counter("transition.tutorial.survival_tools")] = counter_value(True)
        model.data["entitlements"]["entitlement.tutorial.survival_tools"] = {"claimed": True}
        model.data["equipment"]["slot.weapon"] = item_stack("item.axe.bronze")
        model.give("item.fishing_net.small", 1)
        model.give("item.tinderbox", 1)
        self.assertEqual(self.choices(model), [])
        model.data["inventory"]["slots"] = [item_stack("item.shrimps.raw") for _ in range(28)]
        before = deepcopy(model.data)
        choices = {choice["id"]: choice for choice in self.choices(model)}
        with self.assertRaises(OracleRefusal):
            model.effects(choices["replace.net"]["effects"])
        self.assertEqual(model.data, before)

    def test_departure_and_locked_lessons_never_enable_recovery(self):
        model = self.model("stage.tutorial.mainland")
        self.assertEqual(self.choices(model), [])
        model = Oracle(self.content)
        model.data["tutorial_stage"] = "stage.tutorial.catch_shrimp"
        self.assertEqual(self.choices(model), [])


if __name__ == "__main__":
    unittest.main()
