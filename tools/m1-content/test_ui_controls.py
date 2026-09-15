"""Exact additive UI source associations; not new gameplay or presentation acceptance."""
import unittest

from common import BINDINGS, CONTENT, ROOT, load


class UiControlSources(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.levels = load(ROOT / "research/interface-contracts/level-up-native.json")
        cls.controls = load(ROOT / "research/interface-contracts/control-bindings.json")

    def test_level_chat_and_notification_keep_distinct_original_groups_and_styles(self):
        for identifier, binding in self.levels["canonical_associations"].items():
            self.assertEqual(self.content["interfaces"][identifier]["source_ids"], [binding["source_group"]])
        self.assertEqual(self.content["ui"]["level_up"]["interface"], "interface.level_up")
        chat = self.levels["groups"]["LevelupDisplay"]
        self.assertEqual(chat["symbols"]["TEXT1"], 15269889)
        self.assertEqual(chat["symbols"]["TEXT2"], 15269890)
        self.assertEqual(chat["symbols"]["CONTINUE"], 15269891)
        self.assertEqual(chat["widgets"]["TEXT1"]["definition"]["fontId"], 497)
        self.assertEqual(self.levels["groups"]["NotificationDisplay"]["symbols"]["MAIN_TEXT"], 43253768)
        self.assertEqual(self.levels["notification_script"]["source_id"], 3343)

    def test_recovery_bank_permission_is_not_guessed_from_an_office_unit_fee(self):
        office = self.controls["source_scripts"]["3492"]["definition"]
        self.assertTrue(any("each" in value for value in office["stringOperands"] if value))
        self.assertIn("Take-All", self.controls["source_scripts"]["3490"]["definition"]["stringOperands"])
        retrieval = self.controls["source_scripts"]["1987"]["definition"]
        self.assertIn("Bank-All", retrieval["stringOperands"])
        self.assertEqual(list(zip(retrieval["instructions"], retrieval["intOperands"]))[213:215],
                         [(1, 263), (0, 1)])
        recovery = self.content["ui"]["recovery"]
        self.assertEqual(recovery["office_bank"]["kind"], "unavailable")
        self.assertEqual(recovery["grave_bank"]["kind"], "unavailable")
        self.assertEqual(load(BINDINGS / "ui-bindings.json")["remaining_source_permissions"],
                         ["normal_grave_bank_all"])
        self.assertEqual(self.content["interfaces"]["interface.grave"]["source_ids"], [602])

    def test_additive_controls_do_not_rewrite_reward_values_or_source_bank_defaults(self):
        ui = self.content["ui"]
        self.assertFalse(ui["bank"]["initial_insert"])
        self.assertFalse(ui["bank"]["initial_placeholders"])
        self.assertEqual(ui["bank"]["maximum_tabs"], 9)
        self.assertEqual(ui["quest_rewards"]["quest.cooks_assistant"]["items"], [])
        self.assertEqual(ui["quest_rewards"]["quest.cooks_assistant"]["xp"],
                         [{"skill": "skill.cooking", "amount_tenths": 3000}])
        self.assertEqual(load(BINDINGS / "ui-bindings.json")["additive_capabilities"],
                         ["game.ui.amounts.v1", "game.ui.recovery.v1"])


if __name__ == "__main__":
    unittest.main()
