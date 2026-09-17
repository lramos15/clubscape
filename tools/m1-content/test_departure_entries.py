"""Equivalent source question branches remain one menu, not first-entry selection."""

from copy import deepcopy
import unittest

from common import CONTENT, load
from dialogue_entries import normalize_tutorial_choices
from state_oracles import Oracle


CHOICES = ["stay_on_island", "ask_ironman_tutor", "confirm_normal_mainland"]


def fixture():
    guard = {"kind": "tutorial_stage", "stage": "stage.test.confirmation"}
    nodes = [{
        "id": "transition.tutorial." + choice,
        "text": "One explicit source question.",
        "guard": deepcopy(guard),
        "choices": [{
            "id": choice, "text": choice, "guard": deepcopy(guard),
            "effects": [], "next_node": None,
        }],
    } for choice in CHOICES]
    return {"dialogues": {"dialogue.test": {
        "id": "dialogue.test", "nodes": nodes,
        "entry_nodes": [node["id"] for node in nodes], "source": ["unchanged fixture source"],
    }}}


class DepartureEntryTests(unittest.TestCase):
    def test_actual_departure_question_has_all_three_original_choices(self):
        content = load(CONTENT / "game-content.json.gz")
        model = Oracle(content)
        model.data["tutorial_stage"] = "stage.tutorial.departure_confirmation"
        dialogue = content["dialogues"]["dialogue.magic_instructor"]
        before_nodes = [node for node in dialogue["nodes"]
                        if node["id"] in dialogue["entry_nodes"] and model.guard(node["guard"])]
        before_choices = deepcopy([choice for node in before_nodes for choice in node["choices"]])
        before_other_content = deepcopy({key: value for key, value in content.items() if key != "dialogues"})
        normalize_tutorial_choices(content)
        entries = [node for node in dialogue["nodes"]
                   if node["id"] in dialogue["entry_nodes"] and model.guard(node["guard"])]
        self.assertEqual(len(entries), 1)
        self.assertEqual(entries[0]["text"], before_nodes[0]["text"])
        self.assertEqual(entries[0]["choices"], before_choices)
        self.assertEqual([choice["id"] for choice in entries[0]["choices"]], CHOICES)
        self.assertEqual({key: value for key, value in content.items() if key != "dialogues"},
                         before_other_content)
        for stage in ["stage.tutorial.departure_offer", "stage.tutorial.home_teleport",
                      "stage.tutorial.mainland"]:
            model.data["tutorial_stage"] = stage
            self.assertFalse(model.guard(entries[0]["guard"]))

    def test_grouping_preserves_order_guards_effects_and_question(self):
        content = fixture()
        dialogue = content["dialogues"]["dialogue.test"]
        dialogue["nodes"][1]["choices"][0]["guard"] = {
            "kind": "not", "guard": {"kind": "always"},
        }
        dialogue["nodes"][2]["choices"][0]["effects"] = [{
            "kind": "set_tutorial_stage", "stage": "stage.test.authorized",
        }]
        before = deepcopy(dialogue)
        audit = normalize_tutorial_choices(content)
        self.assertEqual(dialogue["entry_nodes"], [before["entry_nodes"][0]])
        self.assertEqual(dialogue["nodes"], [{
            **before["nodes"][0],
            "choices": [choice for node in before["nodes"] for choice in node["choices"]],
        }])
        self.assertEqual(dialogue["source"], before["source"])
        self.assertEqual(audit[0]["merged_entry_ids"], before["entry_nodes"])
        self.assertEqual(audit[0]["original_choice_ids"], CHOICES)

    def test_distinct_guards_are_not_turned_into_a_menu_or_priority_rule(self):
        content = fixture()
        for index, node in enumerate(content["dialogues"]["dialogue.test"]["nodes"]):
            node["guard"] = {"kind": "tutorial_stage", "stage": "stage.test." + str(index)}
        before = deepcopy(content)
        self.assertEqual(normalize_tutorial_choices(content), [])
        self.assertEqual(content, before)

    def test_incompatible_equal_guard_questions_fail_without_partial_mutation(self):
        for mutation in ["text", "duplicate_choice", "outgoing", "incoming", "extra_field"]:
            with self.subTest(mutation=mutation):
                content = fixture()
                dialogue = content["dialogues"]["dialogue.test"]
                first, second = dialogue["nodes"][:2]
                if mutation == "text":
                    second["text"] = "A different source question."
                elif mutation == "duplicate_choice":
                    second["choices"][0]["id"] = first["choices"][0]["id"]
                elif mutation == "outgoing":
                    second["choices"][0]["next_node"] = first["id"]
                elif mutation == "incoming":
                    dialogue["nodes"].append({
                        "id": "explicit.continuation", "text": "Different entry.",
                        "guard": {"kind": "always"},
                        "choices": [{
                            "id": "continue", "text": "Continue", "guard": {"kind": "always"},
                            "effects": [], "next_node": second["id"],
                        }],
                    })
                    dialogue["entry_nodes"].append("explicit.continuation")
                else:
                    second["source_extension"] = "must not be discarded"
                before = deepcopy(content)
                with self.assertRaises(ValueError):
                    normalize_tutorial_choices(content)
                self.assertEqual(content, before)

    def test_normalization_is_idempotent(self):
        content = fixture()
        self.assertEqual(len(normalize_tutorial_choices(content)), 1)
        before = deepcopy(content)
        self.assertEqual(normalize_tutorial_choices(content), [])
        self.assertEqual(content, before)

    def test_recovery_and_non_tutorial_entry_names_are_not_reinterpreted(self):
        content = fixture()
        dialogue = content["dialogues"]["dialogue.test"]
        for prefix in ["replace.", "finish.", "transition.cooks."]:
            copy = deepcopy(dialogue)
            for index, node in enumerate(copy["nodes"]):
                node["id"] = prefix + str(index)
            copy["entry_nodes"] = [node["id"] for node in copy["nodes"]]
            probe = {"dialogues": {copy["id"]: copy}}
            before = deepcopy(probe)
            self.assertEqual(normalize_tutorial_choices(probe), [])
            self.assertEqual(probe, before)


if __name__ == "__main__":
    unittest.main()
