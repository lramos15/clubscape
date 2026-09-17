"""Shared completion boundary; checkpoints are never success-shaped substitutes."""

SEGMENTS = (
    "registration_login", "source_initial_character",
    "full_tutorial_learning_the_ropes", "onboarding_recovery", "lumbridge_copper",
    "inventory_equipment_bank_shop", "goblin_combat",
    "source_death_office_grave_recovery",
    "cooks_legitimate_acquisition_partial_delivery", "cooks_reward_and_range",
    "after_quest_recovery",
)


def full_journey_passed(report):
    return (
        report.get("scenario") == "m1_fresh_account"
        and report.get("status") == "passed"
        and report.get("full_journey_passed") is True
        and report.get("milestone_accepted") is False
        and len(report.get("tutorial_edges_passed", [])) == 70
        and all(
            report.get("segments", {}).get(segment, {}).get("status") == "passed"
            for segment in SEGMENTS
        )
    )
