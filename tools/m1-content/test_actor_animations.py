"""Source binding/phase qualifications remain distinct from renderer/avatar acceptance."""
from copy import deepcopy
import unittest

from actor_animation4 import INPUT, bind_actor_animations
from common import CONTENT, load


class ActorAnimationSources(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.content = load(CONTENT / "game-content.json.gz")
        cls.inputs = load(INPUT)
        cls.animations, cls.proof = bind_actor_animations(cls.content)

    def test_every_actual_legal_style_has_an_explicit_source_binding(self):
        self.assertEqual(set(self.animations["styles"]), set(self.content["mechanics"]["combat_styles"]))
        expected = {
            "style.unarmed.block": 422, "style.unarmed.kick": 423,
            "style.axe.bronze.slash.accurate": 395, "style.axe.bronze.crush.aggressive": 401,
            "style.pickaxe.bronze.stab.accurate": 400, "style.pickaxe.bronze.crush.aggressive": 401,
            "style.spear.bronze.stab.controlled": 428, "style.spear.bronze.slash.controlled": 440,
            "style.spear.bronze.crush.controlled": 429,
            "style.dagger.bronze.stab.accurate": 386, "style.dagger.bronze.slash.aggressive": 390,
            "style.shortbow.rapid": 426, "style.magic.wind_strike": 711,
        }
        for style, sequence in expected.items():
            self.assertEqual(self.animations["styles"][style]["rule"]["sequence"], sequence)

    def test_home_phases_preserve_the_frozen_whole_channel_and_native_holds(self):
        home = self.animations["spells"]["spell.lumbridge_home_teleport"]
        self.assertEqual(home["rule"]["duration_ticks"], 24)
        self.assertEqual([(p["at_tick"], p["sequence"]) for p in home["rule"]["phases"]],
                         [(0, 4847), (6, 4850), (12, 4853), (16, 4855), (21, 4857)])
        self.assertEqual(home["source"][0]["status"], "inference")
        for id, active in self.inputs["home_teleport"]["active_cycles_before_terminal_hold"].items():
            frames = self.inputs["native"]["sequences"][id]["frame_lengths"]
            self.assertEqual(sum(frames[:-1]), active)

    def test_existing_published_actor_slots_do_not_prove_missing_clip_dependencies(self):
        self.assertTrue({2305, 395, 400, 401, 428, 429, 440, 4847, 4850, 4853, 4855, 4857}
                        <= set(self.proof["required_sequences"]))
        self.assertEqual(self.inputs["native"]["sequences"]["2305"]["left_hand_item"], 6244)
        self.assertEqual(self.inputs["native"]["sequences"]["4847"]["right_hand_item"], 10214)

    def test_dough_and_empty_remain_qualified_unknown_not_potter_wheel_or_silence(self):
        self.assertEqual(self.animations["recipes"]["recipe.cooking.dough"]["rule"]["kind"], "unverified")
        self.assertNotIn(883, self.proof["required_sequences"])
        empty = [binding for id, binding in self.animations["actions"].items() if id.endswith(".empty")]
        self.assertTrue(empty)
        self.assertTrue(all(binding["rule"]["kind"] == "unverified" for binding in empty))

    def test_consumption_and_death_use_source_motion_not_gameplay_timers(self):
        self.assertEqual(self.animations["actions"]["action.life.death"]["rule"]["sequence"], 836)
        self.assertEqual(self.animations["actions"]["action.item.bread.eat"]["rule"]["sequence"], 12526)
        self.assertEqual(self.animations["actions"]["action.item.beer.drink"]["rule"]["sequence"], 829)
        self.assertEqual(self.animations["sequences"]["829"]["duration_cycles"], 73)
        self.assertEqual(self.animations["sequences"]["711"]["duration_cycles"], 74)

    def test_unqualified_new_weapon_or_attack_type_cannot_get_a_default_sequence(self):
        changed = deepcopy(self.content)
        changed["items"]["item.axe.bronze"]["source_id"] = 123456
        with self.assertRaisesRegex(ValueError, "weapon"):
            bind_actor_animations(changed)


if __name__ == "__main__":
    unittest.main()
