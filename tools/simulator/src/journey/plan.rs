use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use clubscape_protocol::game::{self, world_input::Action};
use serde_json::{Value, json};

use super::{
    Receipt, Runner, evidence,
    source::{Source, Tile},
};

pub(super) const TUTORIAL_STAGES: &[&str] = &[
    "appearance",
    "experience",
    "guide_greeting",
    "settings_open",
    "guide_settings",
    "starting_exit",
    "survival_greeting",
    "inventory_open",
    "catch_shrimp",
    "skills_open",
    "survival_tools",
    "cut_logs",
    "light_fire",
    "cook_shrimp",
    "survival_exit",
    "chef_entry",
    "chef_greeting",
    "make_dough",
    "bake_bread",
    "chef_exit",
    "run_toggle",
    "quest_entry",
    "quest_greeting",
    "journal_open",
    "quest_explanation",
    "quest_ladder",
    "mining_greeting",
    "mine_first",
    "mine_second",
    "smelt_bronze",
    "mining_hammer",
    "anvil_open",
    "smith_dagger",
    "mining_exit",
    "combat_greeting",
    "equipment_open",
    "equipment_stats_open",
    "equip_dagger",
    "melee_supply",
    "equip_melee",
    "combat_open",
    "enter_rat_pen",
    "melee_rat",
    "leave_rat_pen",
    "ranged_supply",
    "equip_ranged",
    "ranged_rat",
    "combat_exit",
    "bank_open",
    "bank_close",
    "poll_inspect",
    "account_entry",
    "account_greeting",
    "account_open",
    "account_explanation",
    "account_exit",
    "chapel_entry",
    "prayer_greeting",
    "prayer_open",
    "prayer_explanation",
    "chapel_exit",
    "magic_entry",
    "magic_greeting",
    "magic_open",
    "magic_supply",
    "wind_strike",
    "departure_offer",
    "departure_confirmation",
    "home_teleport",
    "teleport_channel",
    "mainland",
];

const BANK: &str = "spawn.lumbridge.bank_booth.3208.3221.p2.t10.r2";
const LOWER_STAIRS: &str = "spawn.lumbridge.castle_stairs.3204.3207.p0.t10.r3";
const MIDDLE_STAIRS: &str = "spawn.lumbridge.castle_stairs.middle.3204.3207.p1.t10.r3";
const UPPER_STAIRS: &str = "spawn.lumbridge.castle_stairs.top.3205.3208.p2.t10.r3";
const SHOPKEEPER: &str = "spawn.shopkeeper";
const SHOP: &str = "shop.lumbridge.general_store";
const COOK: &str = "spawn.cook";
const GOBLIN: &str = "spawn.goblin.level_2.3246.3235.p0";
const MILL_LOWER: &str = "spawn.mill.ladder_lower.3164.3307.p0.t10.r0";
const MILL_MIDDLE: &str = "spawn.mill.ladder_middle.3164.3307.p1.t10.r0";
const MILL_UPPER: &str = "spawn.mill.ladder_upper.3164.3307.p2.t10.r0";

pub(super) fn validate_source_path(source: &Source) -> Result<()> {
    ensure!(
        TUTORIAL_STAGES.len() == 71,
        "Explicit tutorial path must cover all source states"
    );
    let actual: BTreeSet<_> = source.tutorial["states"]
        .as_array()
        .context("Missing source states")?
        .iter()
        .map(|stage| stage["id"].as_str().context("Missing source stage ID"))
        .collect::<Result<_>>()?;
    let planned: BTreeSet<_> = TUTORIAL_STAGES
        .iter()
        .map(|stage| format!("stage.tutorial.{stage}"))
        .collect();
    ensure!(
        actual == planned.iter().map(String::as_str).collect(),
        "Unknown or omitted tutorial source stage"
    );
    for pair in TUTORIAL_STAGES.windows(2) {
        source.tutorial_edge(
            &format!("stage.tutorial.{}", pair[0]),
            &format!("stage.tutorial.{}", pair[1]),
        )?;
    }
    Ok(())
}

impl Runner {
    pub(super) async fn tutorial(&mut self) -> Result<()> {
        for pair in TUTORIAL_STAGES.windows(2) {
            let from = format!("stage.tutorial.{}", pair[0]);
            let to = format!("stage.tutorial.{}", pair[1]);
            self.label(&format!("tutorial.{}", pair[0]))?;
            self.evidence.check(
                "expected_tutorial_stage_before",
                json!(from),
                json!(self.player()?.tutorial_stage),
            )?;
            let source_edge = self.source.tutorial_edge(&from, &to)?.clone();
            self.evidence.append(
                "source_action_plan",
                json!({
                    "from": from, "to": to, "source_transition": source_edge["id"],
                    "source_event": source_edge["event_ref"], "basis": source_edge["basis"]
                }),
            )?;
            self.expected_stage = Some((from.clone(), to.clone()));
            self.tutorial_stage(pair[0]).await?;
            self.wait_for("source_tutorial_transition", 40, |runner| {
                Ok(runner.player()?.tutorial_stage == to)
            })
            .await?;
            self.evidence.check(
                "expected_tutorial_stage_after",
                json!(to),
                json!(self.player()?.tutorial_stage),
            )?;
            self.evidence.report["tutorial_edges_passed"].as_array_mut().context("Missing edge ledger")?
                .push(json!({"from": from, "to": to, "source_transition": source_edge["id"], "tick": self.snapshot.tick}));
            self.expected_stage = None;
            self.evidence.flush()?;
            if pair[0] == "catch_shrimp" {
                let receipt = self
                    .onboarding_receipt
                    .clone()
                    .context("Missing legitimate net-grant operation")?;
                self.recovery_checkpoint("onboarding", &receipt).await?;
            }
        }
        self.check_departure()?;
        self.evidence.passed("full_tutorial_learning_the_ropes")
    }

    async fn tutorial_stage(&mut self, stage: &str) -> Result<()> {
        match stage {
            "appearance" => {
                self.input(Action::ConfirmAppearance(game::ConfirmAppearance {
                    appearance: [("body_type".to_owned(), 0)].into_iter().collect(),
                }))
                .await?;
            }
            "experience" => {
                self.input(Action::SelectExperience(game::SelectExperience {
                    experience: "experience.brand_new".into(),
                }))
                .await?;
            }
            "guide_greeting" => {
                self.speak("spawn.gielinor_guide", "greeting_and_settings")
                    .await?;
            }
            "settings_open" => self.open_tab("interface.settings").await?,
            "guide_settings" => {
                self.speak("spawn.gielinor_guide", "settings_and_next_instructor")
                    .await?;
            }
            "starting_exit" => {
                self.cross(
                    "spawn.tutorial.start_door.3098.3107.p0.t0.r0",
                    Tile::new(3098, 3107, 0),
                )
                .await?
            }
            "survival_greeting" => {
                self.onboarding_receipt =
                    Some(self.speak("spawn.survival_expert", "fishing_intro").await?);
                self.check_grant("grant.tutorial.net")?;
            }
            "inventory_open" => self.open_tab("interface.inventory").await?,
            "catch_shrimp" => {
                self.gather("spawn.tutorial.fishing_spot.3099.3090.p0", "Net", "fishing")
                    .await?
            }
            "skills_open" => self.open_tab("interface.skills").await?,
            "survival_tools" => {
                self.speak("spawn.survival_expert", "woodcutting_firemaking_intro")
                    .await?;
                self.check_grant("grant.tutorial.survival_tools")?;
            }
            "cut_logs" => {
                self.gather(
                    "spawn.tree.tutorial.3105.3093.p0.t10.r3",
                    "Chop down",
                    "woodcutting",
                )
                .await?
            }
            "light_fire" => self.light_fire().await?,
            "cook_shrimp" => {
                self.cook(
                    "recipe.cooking.shrimps.fire",
                    None,
                    "item.shrimps.raw",
                    "item.shrimps.cooked",
                    "item.shrimps.burnt",
                    300,
                )
                .await?
            }
            "survival_exit" => {
                self.cross(
                    "spawn.tutorial.survival_gate.3089.3091.p0.t0.r2",
                    Tile::new(3089, 3091, 0),
                )
                .await?
            }
            "chef_entry" => {
                self.cross(
                    "spawn.tutorial.chef_entry.3079.3084.p0.t0.r0",
                    Tile::new(3078, 3084, 0),
                )
                .await?
            }
            "chef_greeting" => {
                self.speak("spawn.master_chef", "bread_intro").await?;
                self.check_grant("grant.tutorial.chef_ingredients")?;
            }
            "make_dough" => self.dough().await?,
            "bake_bread" => {
                self.cook(
                    "recipe.cooking.bread.range",
                    Some("spawn.range.tutorial.3075.3081.p0.t10.r3"),
                    "item.bread.dough",
                    "item.bread",
                    "item.bread.burnt",
                    400,
                )
                .await?
            }
            "chef_exit" => {
                self.cross(
                    "spawn.tutorial.chef_exit.3072.3090.p0.t0.r2",
                    Tile::new(3072, 3090, 0),
                )
                .await?
            }
            "run_toggle" => self.setting(game::SettingKind::Run, true).await?,
            "quest_entry" => {
                self.cross(
                    "spawn.tutorial.quest_entry.3086.3126.p0.t0.r3",
                    Tile::new(3086, 3125, 0),
                )
                .await?
            }
            "quest_greeting" => {
                self.speak("spawn.quest_guide", "quest_intro").await?;
            }
            "journal_open" => self.open_tab("interface.quests").await?,
            "quest_explanation" => {
                self.speak("spawn.quest_guide", "quest_journal_explanation")
                    .await?;
            }
            "quest_ladder" => {
                self.travel(
                    "spawn.tutorial.quest_ladder.3088.3119.p0.t10.r3",
                    "Climb-down",
                )
                .await?
            }
            "mining_greeting" => {
                self.speak("spawn.mining_instructor", "mining_intro")
                    .await?;
                self.check_grant("grant.tutorial.pickaxe")?;
            }
            "mine_first" => {
                self.gather("spawn.rock.tin.tutorial.3074.9503.p0.t10.r2", "Mine", "tin")
                    .await?
            }
            "mine_second" => {
                self.gather(
                    "spawn.rock.copper.tutorial.3083.9501.p0.t10.r0",
                    "Mine",
                    "copper",
                )
                .await?
            }
            "smelt_bronze" => {
                self.produce_checked(
                    "recipe.smelting.bronze",
                    Some("spawn.furnace.tutorial.3078.9495.p0.t10.r3"),
                    &[
                        ("item.ore.tin", -1),
                        ("item.ore.copper", -1),
                        ("item.bar.bronze", 1),
                    ],
                    Some(("skill.smithing", 62)),
                )
                .await?
            }
            "mining_hammer" => {
                self.speak("spawn.mining_instructor", "smithing_intro")
                    .await?;
                self.check_grant("grant.tutorial.hammer")?;
            }
            "anvil_open" => {
                self.interact("spawn.anvil.tutorial.3075.9497.p0.t10.r0", "Smith")
                    .await?;
            }
            "smith_dagger" => {
                self.produce_checked(
                    "recipe.smithing.bronze_dagger",
                    Some("spawn.anvil.tutorial.3075.9497.p0.t10.r0"),
                    &[
                        ("item.bar.bronze", -1),
                        ("item.dagger.bronze", 1),
                        ("item.hammer", 0),
                    ],
                    Some(("skill.smithing", 125)),
                )
                .await?
            }
            "mining_exit" => {
                self.cross(
                    "spawn.tutorial.mine_gate.3094.9502.p0.t0.r2",
                    Tile::new(3095, 9502, 0),
                )
                .await?
            }
            "combat_greeting" => {
                self.speak("spawn.combat_instructor", "equipment_intro")
                    .await?;
            }
            "equipment_open" => self.open_tab("interface.equipment").await?,
            "equipment_stats_open" => self.open_tab("interface.equipment_stats").await?,
            "equip_dagger" => self.equip("item.dagger.bronze").await?,
            "melee_supply" => {
                self.speak("spawn.combat_instructor", "melee_gear").await?;
                self.check_grant("grant.tutorial.melee_gear")?;
            }
            "equip_melee" => {
                self.equip("item.sword.bronze").await?;
                self.equip("item.shield.wooden").await?;
            }
            "combat_open" => self.open_tab("interface.combat").await?,
            "enter_rat_pen" => {
                self.cross(
                    "spawn.tutorial.rat_gate.3111.9518.p0.t0.r0",
                    Tile::new(3110, 9518, 0),
                )
                .await?
            }
            "melee_rat" => {
                self.style("style.sword.bronze.stab.accurate").await?;
                self.fight("spawn.tutorial_rat.3109.9518.p0", false, true)
                    .await?;
            }
            "leave_rat_pen" => {
                self.cross(
                    "spawn.tutorial.rat_gate.3111.9518.p0.t0.r0",
                    Tile::new(3111, 9518, 0),
                )
                .await?
            }
            "ranged_supply" => {
                self.speak("spawn.combat_instructor", "ranged_intro")
                    .await?;
                self.check_grant("grant.tutorial.ranged_gear")?;
            }
            "equip_ranged" => {
                self.equip("item.shortbow").await?;
                self.equip("item.arrow.bronze").await?;
            }
            "ranged_rat" => {
                self.walk(Tile::new(3112, 9518, 0)).await?;
                self.style("style.shortbow.accurate").await?;
                self.fight("spawn.tutorial_rat.3107.9521.p0", true, true)
                    .await?;
            }
            "combat_exit" => {
                self.travel(
                    "spawn.tutorial.combat_ladder.3111.9526.p0.t10.r0",
                    "Climb-up",
                )
                .await?
            }
            "bank_open" => {
                self.interact("spawn.tutorial.bank_booth.3122.3124.p0.t10.r0", "Bank")
                    .await?;
                self.check_bank(25)?;
            }
            "bank_close" => {
                self.input(Action::CloseInterface(game::Empty {})).await?;
            }
            "poll_inspect" => {
                self.interact("spawn.tutorial.poll_booth.3119.3121.p0.t10.r0", "Use")
                    .await?;
            }
            "account_entry" => {
                self.cross(
                    "spawn.tutorial.account_entry.3125.3124.p0.t0.r0",
                    Tile::new(3125, 3124, 0),
                )
                .await?
            }
            "account_greeting" => {
                self.speak("spawn.account_guide", "account_intro").await?;
            }
            "account_open" => self.open_tab("interface.account").await?,
            "account_explanation" => {
                self.speak("spawn.account_guide", "membership_worlds_bonds_security")
                    .await?;
            }
            "account_exit" => {
                self.cross(
                    "spawn.tutorial.account_exit.3130.3124.p0.t0.r0",
                    Tile::new(3130, 3124, 0),
                )
                .await?
            }
            "chapel_entry" => self.walk(Tile::new(3125, 3107, 0)).await?,
            "prayer_greeting" => {
                self.speak("spawn.brother_brace", "prayer_intro").await?;
            }
            "prayer_open" => self.open_tab("interface.prayer").await?,
            "prayer_explanation" => {
                self.input(Action::SetPrayer(game::SetPrayer {
                    prayer: "prayer.thick_skin".into(),
                    enabled: true,
                }))
                .await?;
                self.evidence.check(
                    "actual_prayer_enabled",
                    json!(true),
                    json!(
                        self.player()?
                            .active_prayers
                            .iter()
                            .any(|id| id == "prayer.thick_skin")
                    ),
                )?;
                self.input(Action::SetPrayer(game::SetPrayer {
                    prayer: "prayer.thick_skin".into(),
                    enabled: false,
                }))
                .await?;
                self.speak("spawn.brother_brace", "prayer_activation_points_bones")
                    .await?;
            }
            "chapel_exit" => {
                self.cross(
                    "spawn.tutorial.chapel_exit.3122.3102.p0.t0.r1",
                    Tile::new(3122, 3102, 0),
                )
                .await?
            }
            "magic_entry" => self.walk(Tile::new(3140, 3087, 0)).await?,
            "magic_greeting" => {
                self.speak("spawn.magic_instructor", "magic_intro").await?;
            }
            "magic_open" => self.open_tab("interface.magic").await?,
            "magic_supply" => {
                self.speak("spawn.magic_instructor", "wind_strike_intro")
                    .await?;
                self.check_grant("grant.tutorial.runes")?;
            }
            "wind_strike" => {
                self.walk(Tile::new(3140, 3089, 0)).await?;
                let air = self.count("item.rune.air")?;
                let mind = self.count("item.rune.mind")?;
                self.input(Action::Cast(game::Cast {
                    spell: "spell.wind_strike".into(),
                    target: Some("spawn.tutorial_chicken.3140.3093.p0".into()),
                }))
                .await?;
                self.wait_for("real_wind_strike_hit_or_splash", 60, |runner| {
                    Ok(runner.player()?.tutorial_stage == "stage.tutorial.departure_offer")
                })
                .await?;
                self.cancel().await?;
                self.evidence.check(
                    "one_air_rune_spent",
                    json!(air - 1),
                    json!(self.count("item.rune.air")?),
                )?;
                self.evidence.check(
                    "one_mind_rune_spent",
                    json!(mind - 1),
                    json!(self.count("item.rune.mind")?),
                )?;
                self.evidence.check(
                    "learning_the_ropes_qp_before_departure",
                    json!(1),
                    json!(self.player()?.quest_points),
                )?;
                self.evidence.check(
                    "learning_the_ropes_completed",
                    json!("stage.learning_the_ropes.completed"),
                    json!(self.quest("quest.learning_the_ropes")?),
                )?;
            }
            "departure_offer" => {
                self.speak("spawn.magic_instructor", "offer_mainland")
                    .await?;
            }
            "departure_confirmation" => {
                self.speak("spawn.magic_instructor", "confirm_normal_mainland")
                    .await?;
            }
            "home_teleport" => {
                self.evidence.report["before_departure_skills"] =
                    evidence::stable_player(self.player()?)["skills"].clone();
                self.input(Action::Cast(game::Cast {
                    spell: "spell.lumbridge_home_teleport".into(),
                    target: None,
                }))
                .await?;
            }
            "teleport_channel" => {
                self.wait_for("uninterrupted_source_home_teleport", 80, |runner| {
                    Ok(runner.player()?.tutorial_stage == "stage.tutorial.mainland")
                })
                .await?;
            }
            _ => bail!("Unknown tutorial stage {stage}; no fallback/skip is permitted"),
        }
        Ok(())
    }

    fn check_grant(&mut self, id: &str) -> Result<()> {
        let grant = self.source.tutorial["grant_definitions"]
            .as_array()
            .context("Missing source grants")?
            .iter()
            .find(|grant| grant["id"] == id)
            .context("Missing required source grant")?
            .clone();
        for stack in grant["items"]
            .as_array()
            .context("Missing source grant contents")?
        {
            let item = stack["item_ref"]
                .as_str()
                .context("Missing source item reference")?;
            self.evidence
                .check(id, stack["quantity"].clone(), json!(self.owned(item)?))?;
        }
        Ok(())
    }

    fn check_departure(&mut self) -> Result<()> {
        let kit = self.source.source_rule("rule.tutorial.departure")?["kit"]
            .as_array()
            .context("Missing independent departure kit")?
            .clone();
        let expected: BTreeMap<_, _> = kit
            .iter()
            .map(|stack| {
                Ok((
                    stack["item_ref"]
                        .as_str()
                        .context("Missing source kit item")?
                        .to_owned(),
                    stack["quantity"]
                        .as_u64()
                        .context("Missing source kit quantity")?,
                ))
            })
            .collect::<Result<_>>()?;
        self.evidence.check(
            "source_departure_non_currency_kit",
            json!(expected),
            json!(self.owned_counts()?),
        )?;
        self.evidence.check(
            "source_departure_hp_xp_not_reset",
            json!(11540),
            json!(self.xp("skill.hitpoints")?),
        )?;
        self.evidence.check(
            "source_departure_all_earned_xp_preserved",
            self.evidence.report["before_departure_skills"].clone(),
            evidence::stable_player(self.player()?)["skills"].clone(),
        )?;
        self.evidence.check(
            "source_brand_new_arrival_inference",
            self.source.oracle["tutorial"]["arrival_position"].clone(),
            json!(self.tile()?),
        )?;
        self.evidence.check(
            "source_departure_qp",
            json!(1),
            json!(self.player()?.quest_points),
        )?;
        Ok(())
    }

    fn owned_counts(&self) -> Result<BTreeMap<String, u64>> {
        let mut counts = BTreeMap::new();
        for stack in self
            .player()?
            .inventory
            .iter()
            .filter_map(|slot| slot.stack.as_ref())
            .chain(
                self.player()?
                    .equipment
                    .iter()
                    .filter_map(|slot| slot.stack.as_ref()),
            )
        {
            *counts.entry(stack.item.clone()).or_default() += u64::from(stack.quantity);
        }
        Ok(counts)
    }

    async fn walk(&mut self, goal: Tile) -> Result<()> {
        self.go_to(BTreeSet::from([goal])).await
    }

    async fn go_to(&mut self, goals: BTreeSet<Tile>) -> Result<()> {
        for _ in 0..160 {
            self.bound()?;
            let start = self.tile()?;
            let current = self
                .source
                .navigation_with_states(&self.observed_states, false)?;
            if let Some(route) = current.route(start, &goals) {
                if route.len() == 1 {
                    return Ok(());
                }
                self.walk_chunk(&route).await?;
                continue;
            }
            let potential = self
                .source
                .navigation_with_states(&self.observed_states, true)?;
            let route = potential.route(start, &goals)
                .with_context(|| format!("No bounded source route from {start:?} to {goals:?}; missing travel/collision is not a teleport"))?;
            let edge = route
                .windows(2)
                .find(|pair| !current.step(pair[0], pair[1]))
                .context("Route planner could not identify the source barrier")?;
            let mut choice = None;
            for (id, transform) in self.source.content["mechanics"]["object_transforms"]
                .as_object()
                .context("Missing transforms")?
            {
                let spawn = transform["spawn"]
                    .as_str()
                    .context("Missing transform spawn")?;
                if self
                    .observed_states
                    .get(id)
                    .is_some_and(|state| state == "object_state.open")
                    || self.source.interaction(spawn, "Open").is_err()
                {
                    continue;
                }
                let Some(collision) =
                    transform["states"]["object_state.open"]["collision"].as_array()
                else {
                    continue;
                };
                let touches = collision.iter().any(|cell| {
                    serde_json::from_value::<Tile>(cell["tile"].clone())
                        .is_ok_and(|tile| tile == edge[0] || tile == edge[1])
                });
                if !touches {
                    continue;
                }
                let access =
                    self.source
                        .target_goals(spawn, "Open", self.entities.get(spawn), &current)?;
                if let Some(approach) = current.route(start, &access)
                    && choice
                        .as_ref()
                        .is_none_or(|(_, previous): &(String, Vec<Tile>)| {
                            approach.len() < previous.len()
                        })
                {
                    choice = Some((spawn.to_owned(), approach));
                }
            }
            let (door, approach) = choice
                .context("Source path crosses a barrier with no reachable, declared Open action")?;
            if approach.len() > 1 {
                self.walk_chunk(&approach).await?;
            } else {
                self.interact_here(&door, "Open").await?;
                self.poll().await?;
            }
        }
        bail!(
            "Source navigation exhausted 160 observed walk/door steps; position={:?}",
            self.tile()?
        )
    }

    async fn walk_chunk(&mut self, path: &[Tile]) -> Result<()> {
        let destination = path[path.len().saturating_sub(1).min(20)];
        let running = self
            .player()?
            .settings
            .iter()
            .any(|setting| setting.setting == game::SettingKind::Run as i32 && setting.enabled);
        self.input(Action::Walk(game::Walk {
            destination: Some(destination.wire()),
            running,
        }))
        .await?;
        self.wait_for("source_collision_walk", 100, |runner| {
            if runner.tile()? == destination {
                return Ok(true);
            }
            ensure!(
                runner.player()?.activity != "idle",
                "Server stopped short of requested source tile {destination:?}: actual={:?}",
                runner.tile()?
            );
            Ok(false)
        })
        .await
    }

    async fn approach(&mut self, target: &str, action: &str) -> Result<()> {
        self.evidence
            .append("source_target", self.source.source_identity(target)?)?;
        for _ in 0..4 {
            let navigation = self
                .source
                .navigation_with_states(&self.observed_states, true)?;
            let goals =
                self.source
                    .target_goals(target, action, self.entities.get(target), &navigation)?;
            self.go_to(goals.clone()).await?;
            self.poll().await?;
            let current = self
                .source
                .navigation_with_states(&self.observed_states, false)?;
            let current_goals =
                self.source
                    .target_goals(target, action, self.entities.get(target), &current)?;
            if current_goals.contains(&self.tile()?) {
                return Ok(());
            }
        }
        bail!(
            "Moving source target {target} did not remain reachable within four observed approaches"
        )
    }

    async fn interact_here(&mut self, target: &str, action: &str) -> Result<Receipt> {
        self.source.interaction(target, action)?;
        let entity = self.entities.get(target).with_context(|| {
            format!("Source target {target} absent from authoritative interest view")
        })?;
        ensure!(
            entity.actions_evaluated,
            "Guarded interaction permissions are unavailable for {target}; declared action names are not authority"
        );
        ensure!(
            entity.available && entity.actions.iter().any(|name| name == action),
            "Source target {target} does not currently offer {action}; actual={:?}",
            entity.actions
        );
        self.input(Action::Interact(game::Interact {
            target: target.into(),
            action: action.into(),
        }))
        .await
    }

    async fn interact(&mut self, target: &str, action: &str) -> Result<Receipt> {
        self.approach(target, action).await?;
        self.interact_here(target, action).await
    }

    async fn cross(&mut self, door: &str, goal: Tile) -> Result<()> {
        let transform = self.source.content["mechanics"]["object_transforms"]
            .as_object()
            .context("Missing source transforms")?
            .iter()
            .find(|(_, definition)| definition["spawn"] == door)
            .map(|(id, _)| id.clone())
            .context("Required tutorial source door has no transform")?;
        if self
            .observed_states
            .get(&transform)
            .is_none_or(|state| state != "object_state.open")
        {
            self.interact(door, "Open").await?;
        }
        self.walk(goal).await
    }

    async fn speak(&mut self, speaker: &str, choice: &str) -> Result<Receipt> {
        self.interact(speaker, "Talk-to").await?;
        self.choose(speaker, choice).await
    }

    async fn choose(&mut self, speaker: &str, choice: &str) -> Result<Receipt> {
        let dialogue = self
            .snapshot
            .dialogue
            .as_ref()
            .context("Actual guarded NPC dialogue was not projected")?;
        ensure!(
            dialogue.speaker == speaker,
            "Wrong authoritative dialogue speaker"
        );
        ensure!(
            dialogue.choices.iter().any(|option| option.id == choice),
            "Required source dialogue choice {choice} is unavailable for {speaker}; node={}, actual choices={:?}",
            dialogue.id,
            dialogue
                .choices
                .iter()
                .map(|option| &option.id)
                .collect::<Vec<_>>()
        );
        self.input(Action::DialogueChoice(game::DialogueChoice {
            speaker: speaker.into(),
            choice: choice.into(),
        }))
        .await
    }

    async fn open_tab(&mut self, interface: &str) -> Result<()> {
        ensure!(
            self.player()?
                .unlocked_interfaces
                .iter()
                .any(|id| id == interface),
            "Required source interface {interface} has not unlocked"
        );
        self.input(Action::OpenInterface(game::OpenInterface {
            interface: interface.into(),
        }))
        .await?;
        Ok(())
    }

    async fn setting(&mut self, setting: game::SettingKind, enabled: bool) -> Result<()> {
        self.input(Action::SetSetting(game::SetSetting {
            setting: setting as i32,
            enabled,
        }))
        .await?;
        self.evidence.check(
            "source_setting_changed",
            json!(Some(enabled)),
            json!(
                self.player()?
                    .settings
                    .iter()
                    .find(|value| value.setting == setting as i32)
                    .map(|value| value.enabled)
            ),
        )
    }

    async fn equip(&mut self, item: &str) -> Result<()> {
        let slot = self.slot(item)?;
        let before = self.owned(item)?;
        self.input(Action::Equip(game::InventorySlot { slot }))
            .await?;
        self.evidence.check(
            "equipment_ownership_conserved",
            json!(before),
            json!(self.owned(item)?),
        )?;
        ensure!(
            self.player()?
                .equipment
                .iter()
                .any(|slot| slot.stack.as_ref().is_some_and(|stack| stack.item == item)),
            "Source equipment operation did not equip {item}"
        );
        Ok(())
    }

    async fn style(&mut self, style: &str) -> Result<()> {
        self.input(Action::SetCombatStyle(game::SetCombatStyle {
            style: style.into(),
        }))
        .await?;
        self.evidence.check(
            "source_combat_style",
            json!(style),
            json!(self.player()?.combat_style),
        )
    }

    async fn travel(&mut self, target: &str, action: &str) -> Result<()> {
        let expected = self.source.travel_destination(target, action)?;
        self.evidence.append(
            "source_travel_inference",
            json!({
                "spawn": target, "action": action, "expected": expected,
                "oracle": "research/m1-bindings/travel-bindings.json#landing_candidates"
            }),
        )?;
        self.interact(target, action).await?;
        self.wait_for("source_floor_or_region_travel", 80, |runner| {
            Ok(runner.tile()? == expected)
        })
        .await
    }

    async fn gather(&mut self, target: &str, action: &str, activity: &str) -> Result<()> {
        let oracle = self.source.oracle["activities"][activity].clone();
        let item = oracle["item"]
            .as_str()
            .context("Missing independent gather item")?;
        let skill = oracle["skill"]
            .as_str()
            .context("Missing independent gather skill")?;
        let per_item = oracle["xp_tenths_per_item"]
            .as_u64()
            .context("Missing source gather XP")?;
        let before_items = self.count(item)?;
        let before_xp = self.xp(skill)?;
        self.interact(target, action).await?;
        self.wait_for(
            "stochastic_source_gather",
            self.source.number("/activities/gather_tick_budget")?,
            |runner| Ok(runner.count(item)? > before_items),
        )
        .await?;
        self.cancel().await?;
        let count = self
            .count(item)?
            .checked_sub(before_items)
            .context("Gather removed owned resources")?;
        let xp = self
            .xp(skill)?
            .checked_sub(before_xp)
            .context("Gather removed earned XP")?;
        self.evidence.check(
            "actual_gather_source_xp",
            json!(count * per_item),
            json!(xp),
        )?;
        self.evidence.append(
            "stochastic_observation",
            json!({
                "source_rule": oracle["source_rule"], "target": target, "items": count,
                "xp_tenths": xp, "rng_controlled": false,
                "distribution_certified": false, "failed_rolls_not_replaced": true
            }),
        )
    }

    async fn production_input(&mut self, recipe: &str, target: Option<&str>) -> Result<()> {
        if let Some(target) = target {
            let action = self.source.spawn(target)?["interactions"]
                .as_array()
                .context("Missing facility actions")?
                .iter()
                .find(|action| {
                    action["action"]["recipes"]
                        .as_array()
                        .is_some_and(|recipes| recipes.iter().any(|id| id == recipe))
                })
                .and_then(|action| action["name"].as_str())
                .context("Source facility does not offer selected recipe")?
                .to_owned();
            self.approach(target, &action).await?;
        }
        self.input(Action::ProduceSelected(game::ProduceSelected {
            recipe: recipe.into(),
            target: target.map(|spawn| game::WorldTarget {
                target: Some(game::world_target::Target::Spawn(spawn.into())),
            }),
            quantity: 1,
            mode: game::ProductionMode::Single as i32,
        }))
        .await?;
        Ok(())
    }

    async fn produce_checked(
        &mut self,
        recipe: &str,
        target: Option<&str>,
        deltas: &[(&str, i64)],
        xp: Option<(&str, u64)>,
    ) -> Result<()> {
        let counts = deltas
            .iter()
            .map(|(item, _)| Ok(((*item).to_owned(), self.count(item)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let before_xp = xp.map(|(skill, _)| self.xp(skill)).transpose()?;
        self.production_input(recipe, target).await?;
        self.wait_for("source_production_consumption", 80, |runner| {
            Ok(deltas
                .iter()
                .filter(|(_, delta)| *delta > 0)
                .all(|(item, delta)| {
                    runner
                        .count(item)
                        .is_ok_and(|actual| actual >= counts[*item] + *delta as u64)
                })
                && runner.player()?.activity == "idle")
        })
        .await?;
        for (item, delta) in deltas {
            let expected = counts[*item]
                .checked_add_signed(*delta)
                .context("Impossible source production input count")?;
            self.evidence.check(
                recipe,
                json!({(*item): expected}),
                json!({(*item): self.count(item)?}),
            )?;
        }
        if let Some((skill, amount)) = xp {
            self.evidence.check(
                "production_source_xp",
                json!(before_xp.context("Missing XP baseline")? + amount),
                json!(self.xp(skill)?),
            )?;
        }
        Ok(())
    }

    async fn dough(&mut self) -> Result<()> {
        self.produce_checked(
            "recipe.cooking.dough",
            None,
            &[
                ("item.flour.pot", -1),
                ("item.water.bucket", -1),
                ("item.bread.dough", 1),
                ("item.pot", 1),
                ("item.bucket", 1),
            ],
            Some(("skill.cooking", 0)),
        )
        .await
    }

    async fn light_fire(&mut self) -> Result<()> {
        let before_xp = self.xp("skill.firemaking")?;
        let start = self.snapshot.tick;
        let site = self.tile()?;
        let logs = self.count("item.logs.normal")?;
        let previous: BTreeSet<_> = self
            .snapshot
            .dynamic_objects
            .iter()
            .map(|object| object.id.clone())
            .collect();
        self.production_input("recipe.firemaking.normal", None)
            .await?;
        // Pending ignition is separate from the public activity string. Real source ticks
        // own its repeated failed rolls; an idle label does not authorize restarting it.
        self.wait_for(
            "source_firemaking_with_retained_ground_log",
            512,
            |runner| {
                Ok(runner.snapshot.dynamic_objects.iter().any(|object| {
                    object.definition_id == "temporary_object.fire.normal"
                        && !previous.contains(&object.id)
                        && object.tile.as_ref().map(Tile::from) == Some(site)
                }))
            },
        )
        .await?;
        self.evidence.check(
            "source_firemaking_one_log",
            json!(logs - 1),
            json!(self.count("item.logs.normal")?),
        )?;
        self.evidence.check(
            "source_firemaking_xp",
            json!(before_xp + 400),
            json!(self.xp("skill.firemaking")?),
        )?;
        self.evidence.append(
            "firemaking_observed",
            json!({
                "start_tick": start, "observed_ticks": self.snapshot.tick - start,
                "source_tick_budget": 512, "rng_controlled": false,
                "server_owns_ignition_retries": true, "private_pending_fire_state_read": false
            }),
        )
    }

    async fn cook(
        &mut self,
        recipe: &str,
        target: Option<&str>,
        raw: &str,
        success: &str,
        burn: &str,
        success_xp: u64,
    ) -> Result<()> {
        let (raw_count, success_count, burn_count, xp) = (
            self.count(raw)?,
            self.count(success)?,
            self.count(burn)?,
            self.xp("skill.cooking")?,
        );
        if let Some(target) = target {
            self.production_input(recipe, Some(target)).await?;
        } else {
            let fire = self
                .snapshot
                .dynamic_objects
                .iter()
                .find(|object| object.definition_id == "temporary_object.fire.normal")
                .context("No actual temporary fire in the authoritative view")?
                .clone();
            let tile = fire
                .tile
                .as_ref()
                .map(Tile::from)
                .context("Fire location missing")?;
            self.go_to(BTreeSet::from([
                Tile::new(tile.x.saturating_sub(1), tile.y, tile.plane),
                Tile::new(tile.x + 1, tile.y, tile.plane),
                Tile::new(tile.x, tile.y.saturating_sub(1), tile.plane),
                Tile::new(tile.x, tile.y + 1, tile.plane),
            ]))
            .await?;
            self.input(Action::ProduceSelected(game::ProduceSelected {
                recipe: recipe.into(),
                quantity: 1,
                mode: game::ProductionMode::Single as i32,
                target: Some(game::WorldTarget {
                    target: Some(game::world_target::Target::TemporaryObject(fire.id)),
                }),
            }))
            .await?;
        }
        self.wait_for(
            "stochastic_cooking_outcome_not_guaranteed_success",
            80,
            |runner| Ok(runner.count(success)? > success_count || runner.count(burn)? > burn_count),
        )
        .await?;
        self.cancel().await?;
        let success_delta = self.count(success)? - success_count;
        let burn_delta = self.count(burn)? - burn_count;
        self.evidence.check(
            "one_real_cooking_outcome",
            json!(1),
            json!(success_delta + burn_delta),
        )?;
        self.evidence.check(
            "cooking_consumed_raw_item",
            json!(raw_count - 1),
            json!(self.count(raw)?),
        )?;
        self.evidence.check(
            "success_or_burn_source_xp",
            json!(xp + success_delta * success_xp),
            json!(self.xp("skill.cooking")?),
        )
    }

    async fn fight(&mut self, target: &str, ranged: bool, tutorial: bool) -> Result<()> {
        if !ranged {
            self.approach(target, "Attack").await?;
        } else {
            let target_tile = self
                .entities
                .get(target)
                .and_then(|entity| entity.tile.as_ref())
                .map(Tile::from)
                .context("Required ranged source target is not visible")?;
            ensure!(
                self.tile()?.distance(target_tile) <= 7,
                "Selected rat is outside accurate shortbow range"
            );
        }
        self.wait_for("source_npc_respawn", 100, |runner| {
            Ok(runner
                .entities
                .get(target)
                .is_some_and(|entity| entity.available && entity.hitpoints > 0))
        })
        .await?;
        let before_events = self.event_ids.clone();
        let before_xp = self.xp(if ranged {
            "skill.ranged"
        } else {
            "skill.attack"
        })?;
        self.interact_here(target, "Attack").await?;
        let started = self.snapshot.tick;
        let budget = self.source.number("/activities/combat_tick_budget")?;
        loop {
            let killed = self.events.iter().any(|event| {
                event.kind == "npc_killed"
                    && event.target == target
                    && !before_events.contains(&event.event_id)
            });
            if killed {
                break;
            }
            ensure!(
                self.snapshot.tick - started < budget,
                "Stochastic combat exhausted {budget} source ticks for {target}"
            );
            ensure!(
                self.player()?.hitpoints > 0 && self.player()?.instance.is_none(),
                "Required successful combat instead caused a real death"
            );
            if !tutorial
                && self.player()?.hitpoints <= 4
                && let Some(item) = ["item.bread", "item.shrimps.cooked"]
                    .into_iter()
                    .find(|item| self.count(item).is_ok_and(|count| count > 0))
            {
                self.input(Action::Eat(game::InventorySlot {
                    slot: self.slot(item)?,
                }))
                .await?;
            }
            self.poll().await?;
        }
        self.cancel().await?;
        let after_xp = self.xp(if ranged {
            "skill.ranged"
        } else {
            "skill.attack"
        })?;
        ensure!(
            after_xp > before_xp,
            "Credited source combat did not grant skill XP"
        );
        if !tutorial {
            self.evidence.check(
                "source_goblin_attack_xp_for_five_hp",
                json!(200),
                json!(after_xp - before_xp),
            )?;
        } else {
            self.evidence.check(
                "source_tutorial_no_hitpoints_xp",
                json!(11540),
                json!(self.xp("skill.hitpoints")?),
            )?;
        }
        self.evidence.append(
            "source_combat_completed",
            json!({
                "target": target, "ranged": ranged, "ticks": self.snapshot.tick - started,
                "xp_delta_tenths": after_xp - before_xp, "rng_controlled": false,
                "source_identity": self.source.source_identity(target)?
            }),
        )
    }

    fn check_bank(&mut self, coins: u64) -> Result<()> {
        ensure!(
            self.player()?.bank_open,
            "Bank contents were not authorized/projected"
        );
        let actual: u64 = self
            .player()?
            .bank
            .iter()
            .filter_map(|slot| slot.stack.as_ref())
            .filter(|stack| stack.item == "item.coins")
            .map(|stack| u64::from(stack.quantity))
            .sum();
        self.evidence
            .check("source_bank_coins", json!(coins), json!(actual))?;
        self.evidence.check(
            "source_base_bank_capacity",
            json!(400),
            json!(self.player()?.bank_capacity),
        )?;
        Ok(())
    }

    pub(super) async fn open_mainland_bank(&mut self) -> Result<()> {
        match self.tile()?.plane {
            0 => {
                self.travel(LOWER_STAIRS, "Climb-up").await?;
                self.travel(MIDDLE_STAIRS, "Climb-up").await?;
            }
            1 => self.travel(MIDDLE_STAIRS, "Climb-up").await?,
            2 => {}
            _ => bail!("Unexpected floor for legitimate Lumbridge bank route"),
        }
        self.interact(BANK, "Bank").await?;
        ensure!(
            self.player()?.bank_open,
            "Real bank interaction did not project authorized contents"
        );
        Ok(())
    }

    async fn leave_bank(&mut self) -> Result<()> {
        self.input(Action::CloseInterface(game::Empty {})).await?;
        self.travel(UPPER_STAIRS, "Climb-down").await?;
        self.travel(MIDDLE_STAIRS, "Climb-down").await
    }

    async fn deposit(&mut self, item: &str, quantity: u32) -> Result<()> {
        self.input(Action::BankDeposit(game::BankDeposit {
            banker: BANK.into(),
            inventory_slot: self.slot(item)?,
            quantity,
        }))
        .await?;
        Ok(())
    }

    async fn withdraw(&mut self, item: &str, quantity: u32) -> Result<()> {
        let bank_slot = self
            .player()?
            .bank
            .iter()
            .find(|slot| slot.stack.as_ref().is_some_and(|stack| stack.item == item))
            .map(|slot| slot.index)
            .with_context(|| format!("No legitimately banked {item}"))?;
        self.input(Action::BankWithdraw(game::BankWithdraw {
            banker: BANK.into(),
            bank_slot,
            quantity,
            noted: false,
        }))
        .await?;
        Ok(())
    }

    pub(super) async fn lumbridge(&mut self) -> Result<()> {
        self.label("lumbridge.source_copper_with_legitimate_pickaxe")?;
        self.equip("item.pickaxe.bronze").await?;
        self.gather("spawn.rock.copper.3228.3144.p0.t10.r2", "Mine", "copper")
            .await?;
        self.evidence.passed("lumbridge_copper")?;

        self.label("lumbridge.inventory_equipment_bank_shop")?;
        let original = self.player()?.inventory.clone();
        let from = self.slot("item.ore.copper")?;
        let to = if from == 27 { 26 } else { 27 };
        self.input(Action::MoveInventory(game::MoveInventory { from, to }))
            .await?;
        self.evidence.check(
            "actual_inventory_move",
            json!("item.ore.copper"),
            json!(
                self.player()?
                    .inventory
                    .iter()
                    .find(|slot| slot.index == to)
                    .and_then(|slot| slot.stack.as_ref())
                    .map(|stack| stack.item.as_str())
            ),
        )?;
        self.input(Action::MoveInventory(game::MoveInventory {
            from: to,
            to: from,
        }))
        .await?;
        self.evidence.check(
            "inventory_move_roundtrip",
            json!(
                original
                    .iter()
                    .map(|slot| (slot.index, slot.stack.as_ref().map(evidence::stack_json)))
                    .collect::<BTreeMap<_, _>>()
            ),
            json!(
                self.player()?
                    .inventory
                    .iter()
                    .map(|slot| (slot.index, slot.stack.as_ref().map(evidence::stack_json)))
                    .collect::<BTreeMap<_, _>>()
            ),
        )?;
        self.open_mainland_bank().await?;
        self.check_bank(25)?;
        let copper = self.count("item.ore.copper")?;
        self.deposit("item.ore.copper", 1).await?;
        self.evidence.check(
            "bank_deposit_inventory_debit",
            json!(copper - 1),
            json!(self.count("item.ore.copper")?),
        )?;
        self.withdraw("item.ore.copper", 1).await?;
        self.evidence.check(
            "bank_withdraw_owned_copper",
            json!(copper),
            json!(self.count("item.ore.copper")?),
        )?;
        self.deposit("item.ore.copper", 1).await?;
        self.withdraw("item.coins", 10).await?;
        self.check_bank(15)?;
        self.leave_bank().await?;

        let buckets = self.count("item.bucket")?;
        let coins = self.count("item.coins")?;
        self.interact(SHOPKEEPER, "Trade").await?;
        let shop = self
            .snapshot
            .shop
            .as_ref()
            .context("Actual guarded shop view is missing")?;
        ensure!(
            shop.shop == SHOP,
            "Opened shop identity differs from the source target"
        );
        let index = shop
            .lines
            .iter()
            .find(|line| line.item == "item.bucket")
            .context("Source bucket is absent from the guarded current shop rows")?
            .index;
        let buy = game::ShopBuy {
            shop: SHOP.into(),
            item_index: index,
            quantity: 1,
        };
        let quote = self
            .quote(game::quote_request::Request::ShopBuy(buy.clone()))
            .await?;
        let Some(game::quote::Result::Shop(quote)) = quote.result else {
            bail!("Guarded source purchase did not return a shop quote");
        };
        self.evidence.check(
            "quoted_source_bucket_identity",
            json!("item.bucket"),
            json!(quote.item),
        )?;
        self.evidence.check(
            "quoted_source_bucket_quantity",
            json!(1),
            json!(quote.quantity),
        )?;
        self.evidence.check(
            "quoted_source_bucket_total",
            json!(2),
            json!(quote.total_price),
        )?;
        self.input(Action::ShopBuy(buy)).await?;
        self.evidence.check(
            "source_shop_bucket_purchase",
            json!(buckets + 1),
            json!(self.count("item.bucket")?),
        )?;
        self.evidence.check(
            "source_shop_bucket_price",
            json!(coins - 2),
            json!(self.count("item.coins")?),
        )?;
        let sell = game::ShopSell {
            shop: SHOP.into(),
            inventory_slot: self.slot("item.bucket")?,
            quantity: 1,
        };
        let quote = self
            .quote(game::quote_request::Request::ShopSell(sell.clone()))
            .await?;
        let Some(game::quote::Result::Shop(quote)) = quote.result else {
            bail!("Guarded source sale did not return a shop quote");
        };
        self.evidence.check(
            "quoted_source_bucket_sale_quantity",
            json!(1),
            json!(quote.quantity),
        )?;
        self.evidence.check(
            "quoted_source_bucket_sale_total",
            json!(0),
            json!(quote.total_price),
        )?;
        self.input(Action::ShopSell(sell)).await?;
        self.evidence.check(
            "source_shop_bucket_sale",
            json!(buckets),
            json!(self.count("item.bucket")?),
        )?;
        self.evidence.check(
            "source_zero_coin_bucket_sale",
            json!(coins - 2),
            json!(self.count("item.coins")?),
        )?;
        self.input(Action::CloseInterface(game::Empty {})).await?;
        self.evidence.passed("inventory_equipment_bank_shop")?;

        self.label("lumbridge.actual_goblin_combat")?;
        self.equip("item.sword.bronze").await?;
        self.equip("item.shield.wooden").await?;
        self.style("style.sword.bronze.stab.accurate").await?;
        self.fight(GOBLIN, false, false).await?;
        let ground = self
            .snapshot
            .ground_items
            .iter()
            .find(|ground| {
                ground
                    .stack
                    .as_ref()
                    .is_some_and(|stack| stack.item == "item.bones")
            })
            .context("Credited source goblin did not produce its guaranteed bones")?
            .clone();
        self.take_ground(&ground).await?;
        self.evidence.passed("goblin_combat")?;
        self.source_death().await
    }

    async fn take_ground(&mut self, ground: &game::GroundItem) -> Result<()> {
        ensure!(
            ground.permissions_evaluated && ground.can_take,
            "Ground-item permission is unavailable or denied"
        );
        let tile = ground
            .tile
            .as_ref()
            .map(Tile::from)
            .context("Missing ground-item tile")?;
        let stack = ground
            .stack
            .as_ref()
            .context("Missing real ground item")?
            .clone();
        self.walk(tile).await?;
        let before = self.count(&stack.item)?;
        self.input(Action::TakeGroundItem(game::TakeGroundItem {
            ground_item_id: ground.id.clone(),
        }))
        .await?;
        self.evidence.check(
            "actual_ground_transfer",
            json!(before + u64::from(stack.quantity)),
            json!(self.count(&stack.item)?),
        )
    }

    async fn take_spawn(&mut self, spawn: &str, item: &str) -> Result<()> {
        let tile = self.source.spawn_tile(spawn)?;
        self.walk(tile).await?;
        self.wait_for("source_ground_spawn_respawn", 120, |runner| {
            Ok(runner.snapshot.ground_items.iter().any(|ground| {
                ground.tile.as_ref().map(Tile::from) == Some(tile)
                    && ground
                        .stack
                        .as_ref()
                        .is_some_and(|stack| stack.item == item)
            }))
        })
        .await?;
        let candidates: Vec<_> = self
            .snapshot
            .ground_items
            .iter()
            .filter(|ground| {
                ground.tile.as_ref().map(Tile::from) == Some(tile)
                    && ground
                        .stack
                        .as_ref()
                        .is_some_and(|stack| stack.item == item)
            })
            .cloned()
            .collect();
        ensure!(
            candidates.len() == 1,
            "Source item-spawn selector is ambiguous; ground IDs cannot be guessed"
        );
        self.evidence.append("source_item_spawn", json!({
            "source_identity": self.source.source_identity(spawn)?, "actual_ground_id": candidates[0].id
        }))?;
        self.take_ground(&candidates[0]).await
    }

    async fn source_death(&mut self) -> Result<()> {
        self.label("lumbridge.source_item_losing_death")?;
        self.setting(game::SettingKind::DeathSupplyPiles, false)
            .await?;
        self.setting(game::SettingKind::AutoRetaliate, false)
            .await?;
        self.input(Action::Unequip(game::Unequip {
            slot: "slot.shield".into(),
        }))
        .await?;
        let inventory_before = self.owned_counts()?;
        ensure!(
            inventory_before.len() > 3,
            "Required first item-losing death needs legitimately held possessions"
        );
        self.approach(GOBLIN, "Attack").await?;
        self.wait_for("goblin_respawn_for_required_death", 120, |runner| {
            Ok(runner
                .entities
                .get(GOBLIN)
                .is_some_and(|npc| npc.available && npc.hitpoints > 0))
        })
        .await?;
        self.interact_here(GOBLIN, "Attack").await?;
        self.input(Action::CancelActivity(game::Empty {})).await?;
        let death_tile = self.tile()?;
        let before = evidence::stable_player(self.player()?);
        let hp = self.player()?.hitpoints;
        self.wait_for(
            "actual_goblin_retaliation_to_deaths_office",
            800,
            |runner| {
                Ok(runner.player()?.active_death.is_some()
                    && runner.player()?.region == "region.osrs.12633"
                    && runner.player()?.instance.is_some())
            },
        )
        .await?;
        self.evidence.append(
            "source_death_observed",
            json!({
                "cause": GOBLIN, "hitpoints_before": hp, "death_tile": death_tile,
                "before": before, "actual_office": evidence::public_snapshot(&self.snapshot)?,
                "hp_setter_used": false
            }),
        )?;
        let death = self
            .player()?
            .active_death
            .clone()
            .context("Missing authoritative death identity")?;
        self.evidence.check(
            "death_preserves_xp",
            before["skills"].clone(),
            evidence::stable_player(self.player()?)["skills"].clone(),
        )?;
        self.evidence.check(
            "death_preserves_quests",
            before["quests"].clone(),
            evidence::stable_player(self.player()?)["quests"].clone(),
        )?;
        self.speak("spawn.death", "first_item_loss_intro").await?;
        for topic in ["fees", "timer", "kept_items"] {
            self.choose("spawn.death", topic).await?;
        }
        self.choose("spawn.death", "done").await?;
        self.interact("spawn.death.portal.3169.5726.p0.t10.r3", "Use")
            .await?;
        self.wait_for("legitimate_death_portal_exit", 80, |runner| {
            Ok(runner.player()?.instance.is_none()
                && runner.player()?.region.starts_with("region.osrs.128"))
        })
        .await?;
        self.walk(death_tile).await?;
        self.input(evidence::generated_action(
            "open_grave",
            json!({"death": death}),
        )?)
        .await?;
        let panel = evidence::message_json_with_defaults(
            "clubscape.game.v1.WorldSnapshot",
            &self.snapshot,
        )?;
        let entries = recovery_entries(&panel, &death)?;
        ensure!(
            !entries.is_empty(),
            "Item-losing death has no authorized grave entries"
        );
        let fee: u64 = entries.iter().map(|(_, fee)| fee).sum();
        self.evidence
            .check("source_low_value_grave_fee", json!(0), json!(fee))?;
        self.input(Action::Reclaim(game::Reclaim {
            death,
            storage: game::RecoveryStorage::Grave as i32,
            items: entries.into_iter().map(|(id, _)| id).collect(),
        }))
        .await?;
        self.evidence.check(
            "source_grave_conserves_legitimate_possessions",
            json!(inventory_before),
            json!(self.owned_counts()?),
        )?;
        self.evidence.check(
            "source_grave_preserves_xp",
            before["skills"].clone(),
            evidence::stable_player(self.player()?)["skills"].clone(),
        )?;
        self.evidence.check(
            "source_grave_restores_equipment",
            before["equipment"].clone(),
            evidence::stable_player(self.player()?)["equipment"].clone(),
        )?;
        self.setting(game::SettingKind::AutoRetaliate, true).await?;
        self.evidence.passed("source_death_office_grave_recovery")
    }

    pub(super) async fn cooks_assistant(&mut self) -> Result<()> {
        self.label("cooks.prepare_without_precollected_ingredient_substitution")?;
        self.open_mainland_bank().await?;
        for _ in 0..28 {
            let Some(slot) = self.player()?.inventory.first().cloned() else {
                break;
            };
            let stack = slot.stack.context("Missing inventory stack")?;
            self.input(Action::BankDeposit(game::BankDeposit {
                banker: BANK.into(),
                inventory_slot: slot.index,
                quantity: stack.quantity,
            }))
            .await?;
        }
        self.evidence.check(
            "empty_inventory_before_actual_quest_acquisition",
            json!(0),
            json!(self.player()?.inventory.len()),
        )?;
        self.leave_bank().await?;
        self.speak(COOK, "transition.cooks.accept").await?;
        self.evidence.check(
            "cooks_accepted",
            json!("stage.cooks.delivered.none"),
            json!(self.quest("quest.cooks_assistant")?),
        )?;
        let initial_xp = self.xp("skill.cooking")?;
        let initial_qp = self.player()?.quest_points;
        let initial_coins = self.count("item.coins")?;

        self.label("cooks.actual_pot_cellar_bucket_dairy_milk")?;
        self.take_spawn("spawn.pot.3209.3214.p0", "item.pot")
            .await?;
        self.travel(
            "spawn.lumbridge.kitchen_trapdoor.3209.3216.p0.t22.r0",
            "Climb-down",
        )
        .await?;
        self.take_spawn("spawn.bucket.3216.9625.p0", "item.bucket")
            .await?;
        self.travel(
            "spawn.lumbridge.cellar_ladder.3209.9616.p0.t10.r3",
            "Climb-up",
        )
        .await?;
        let milk = self.count("item.milk.bucket")?;
        self.interact("spawn.dairy_cow.3172.3317.p0.t10.r2", "Milk")
            .await?;
        self.wait_for("actual_dairy_milking", 80, |runner| {
            Ok(runner.count("item.milk.bucket")? == milk + 1)
        })
        .await?;
        self.evidence.check(
            "milk_consumes_acquired_bucket",
            json!(0),
            json!(self.count("item.bucket")?),
        )?;
        self.speak(COOK, "transition.cooks.deliver.none_to_milk")
            .await?;
        self.check_partial("stage.cooks.delivered.milk", initial_xp, initial_qp)?;

        self.label("cooks.actual_west_coop_egg_partial_delivery")?;
        self.take_spawn("spawn.egg.3172.3301.p0", "item.egg")
            .await?;
        self.speak(COOK, "transition.cooks.deliver.milk_to_milk_egg")
            .await?;
        self.check_partial("stage.cooks.delivered.milk_egg", initial_xp, initial_qp)?;

        self.label("cooks.actual_wheat_three_floor_mill")?;
        self.make_flour().await?;
        self.speak(COOK, "transition.cooks.deliver.milk_egg_to_milk_flour_egg")
            .await?;
        self.check_partial(
            "stage.cooks.delivered.milk_flour_egg",
            initial_xp,
            initial_qp,
        )?;
        self.evidence
            .passed("cooks_legitimate_acquisition_partial_delivery")?;

        self.label("cooks.actual_once_only_full_reward")?;
        self.reward_receipt = Some(self.speak(COOK, "transition.cooks.complete").await?);
        self.evidence.check(
            "cooks_reward_completed",
            json!("stage.cooks.completed"),
            json!(self.quest("quest.cooks_assistant")?),
        )?;
        self.evidence.check(
            "cooks_source_reward_300_cooking_xp",
            json!(initial_xp + 3000),
            json!(self.xp("skill.cooking")?),
        )?;
        self.evidence.check(
            "cooks_source_reward_one_qp",
            json!(initial_qp + 1),
            json!(self.player()?.quest_points),
        )?;
        self.evidence.check(
            "cooks_source_reward_no_coins",
            json!(initial_coins),
            json!(self.count("item.coins")?),
        )?;
        self.evidence.check(
            "cooks_source_reward_energy_restore",
            json!(10000),
            json!(self.player()?.run_energy),
        )?;
        self.evidence.report["quest_completed_checkpoint"] =
            evidence::public_snapshot(&self.snapshot)?;
        self.evidence.flush()?;
        let reward = self
            .reward_receipt
            .clone()
            .context("Missing Cook reward receipt")?;
        self.verify_duplicate(&reward, "cooks_reward_before_recovery")
            .await?;
        self.prove_range_access().await?;
        self.evidence.passed("cooks_reward_and_range")?;
        self.open_mainland_bank().await?;
        Ok(())
    }

    fn check_partial(&mut self, stage: &str, xp: u64, qp: u32) -> Result<()> {
        self.evidence.check(
            "cooks_source_partial_stage",
            json!(stage),
            json!(self.quest("quest.cooks_assistant")?),
        )?;
        self.evidence.check(
            "no_cooks_xp_before_thanks",
            json!(xp),
            json!(self.xp("skill.cooking")?),
        )?;
        self.evidence.check(
            "no_cooks_qp_before_thanks",
            json!(qp),
            json!(self.player()?.quest_points),
        )?;
        for item in ["item.milk.bucket", "item.egg", "item.flour.pot"] {
            self.evidence.check(
                "delivered_ingredients_consumed",
                json!(0),
                json!(self.count(item)?),
            )?;
        }
        Ok(())
    }

    async fn make_flour(&mut self) -> Result<()> {
        let grain = self.count("item.grain")?;
        let flour = self.count("item.flour.pot")?;
        let pots = self.count("item.pot")?;
        ensure!(
            pots > 0,
            "Flour collection requires a legitimately acquired pot"
        );
        self.interact("spawn.wheat.3160.3296.p0.t10.r3", "Pick")
            .await?;
        self.wait_for("source_wheat_pick", 40, |runner| {
            Ok(runner.count("item.grain")? == grain + 1)
        })
        .await?;
        self.travel(MILL_LOWER, "Climb-up").await?;
        self.travel(MILL_MIDDLE, "Climb-up").await?;
        self.interact("spawn.mill.hopper.3166.3307.p2.t10.r0", "Fill")
            .await?;
        self.evidence.check(
            "real_hopper_consumes_one_grain",
            json!(grain),
            json!(self.count("item.grain")?),
        )?;
        self.interact("spawn.mill.controls.3166.3305.p2.t10.r3", "Operate")
            .await?;
        self.travel(MILL_UPPER, "Climb-down").await?;
        self.travel(MILL_MIDDLE, "Climb-down").await?;
        self.interact("spawn.mill.flour_bin.3166.3306.p0.t10.r0", "Empty")
            .await?;
        self.wait_for("source_flour_bin_collection", 80, |runner| {
            Ok(runner.count("item.flour.pot")? == flour + 1)
        })
        .await?;
        self.evidence.check(
            "real_flour_bin_consumes_one_pot",
            json!(pots - 1),
            json!(self.count("item.pot")?),
        )?;
        self.evidence.append(
            "mill_public_evidence",
            json!({
                "grain_consumed": 1, "flour_produced": 1, "pot_consumed": 1,
                "floors": [0,1,2,1,0], "private_hopper_counter_read": false,
                "controls_operation_acknowledged": true, "grain_plus_pot_shortcut_used": false
            }),
        )
    }

    async fn prove_range_access(&mut self) -> Result<()> {
        self.label("cooks.reward_range_actual_recipe")?;
        self.take_spawn("spawn.pot.3209.3214.p0", "item.pot")
            .await?;
        self.make_flour().await?;
        self.travel(
            "spawn.lumbridge.kitchen_trapdoor.3209.3216.p0.t22.r0",
            "Climb-down",
        )
        .await?;
        self.take_spawn("spawn.bucket.3216.9625.p0", "item.bucket")
            .await?;
        self.travel(
            "spawn.lumbridge.cellar_ladder.3209.9616.p0.t10.r3",
            "Climb-up",
        )
        .await?;
        // The source sink is not a grain/pot shortcut or a grant; unsupported item-use is a blocker.
        let sink = self
            .source
            .spawn_tile("spawn.water_source.3205.3215.p0.t10.r0")?;
        self.walk(Tile::new(sink.x + 1, sink.y, sink.plane)).await?;
        self.input(Action::UseItem(game::UseItem {
            inventory_slot: self.slot("item.bucket")?,
            target: Some(game::use_item::Target::WorldSpawn(
                "spawn.water_source.3205.3215.p0.t10.r0".into(),
            )),
        }))
        .await?;
        self.wait_for("actual_source_sink_bucket_fill", 80, |runner| {
            Ok(runner.count("item.water.bucket")? == 1)
        })
        .await?;
        self.dough().await?;
        self.cook(
            "recipe.cooking.bread.lumbridge_range",
            Some("spawn.range.lumbridge.3212.3215.p0.t10.r2"),
            "item.bread.dough",
            "item.bread",
            "item.bread.burnt",
            400,
        )
        .await
    }
}

fn recovery_entries(snapshot: &Value, death: &str) -> Result<Vec<(String, u64)>> {
    let mut pending = vec![snapshot];
    let mut found = None;
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                if object.get("death").is_some_and(|id| id == death)
                    && object
                        .get("storage")
                        .and_then(Value::as_str)
                        .is_some_and(|storage| storage == "GRAVE")
                    && let Some(entries) = object.get("entries").and_then(Value::as_array)
                {
                    ensure!(
                        found.is_none(),
                        "Ambiguous guarded recovery views for the active death"
                    );
                    found = Some(entries);
                }
                pending.extend(object.values());
            }
            Value::Array(values) => pending.extend(values),
            _ => {}
        }
    }
    let entries = found.context("Generated WorldSnapshot has no guarded GRAVE recovery view with death/entries/id/full_entry_fee; the runner will not manufacture recovery item IDs")?;
    entries
        .iter()
        .map(|entry| {
            let id = entry["id"]
                .as_str()
                .context("Authorized recovery entry has no stable ID")?;
            let fee = entry
                .get("fullEntryFee")
                .or_else(|| entry.get("full_entry_fee"))
                .context("Authorized recovery view omits the full-entry fee")?;
            let fee = fee
                .as_u64()
                .or_else(|| fee.as_str().and_then(|value| value.parse().ok()))
                .context("Invalid recovery fee")?;
            Ok((id.to_owned(), fee))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn explicit_plan_covers_every_source_state_without_cycle_shortcuts() {
        let source = Source::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        validate_source_path(&source).unwrap();
        assert_eq!(TUTORIAL_STAGES.windows(2).count(), 70);
        assert_eq!(source.tutorial["transitions"].as_array().unwrap().len(), 73);
    }

    #[test]
    fn recovery_does_not_guess_private_item_ids_or_fees() {
        assert!(
            recovery_entries(
                &json!({"player":{"activeDeath":"death.real"}}),
                "death.real"
            )
            .is_err()
        );
        assert!(
            recovery_entries(
                &json!({"recovery":{
                    "death":"death.real","storage":"GRAVE","entries":[{"id":"recovery.real"}]
                }}),
                "death.real"
            )
            .is_err()
        );
        assert_eq!(recovery_entries(&json!({"recovery":{
            "death":"death.real","storage":"GRAVE","entries":[{"id":"recovery.real","fullEntryFee":"0"}]
        }}), "death.real").unwrap(), vec![("recovery.real".into(),0)]);
    }

    #[test]
    fn every_literal_source_spawn_in_the_executable_plan_exists() {
        let source = Source::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let plan = include_str!("plan.rs");
        let mut checked = BTreeSet::new();
        for id in plan.split('"').filter(|text| {
            text.starts_with("spawn.") && text.len() > 6 && !text.contains(char::is_whitespace)
        }) {
            if checked.insert(id) {
                source.spawn(id).unwrap_or_else(|error| panic!("{error:#}"));
            }
        }
        assert!(
            checked.len() > 35,
            "Literal source-selector coverage unexpectedly shrank"
        );
    }
}
