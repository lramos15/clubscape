//! Synthetic rooms, actors and content only. This is not assembled M1 product content.
#![allow(dead_code)]

pub mod v2;

use std::{collections::BTreeMap, sync::Arc};

use clubscape_game_types::*;
use clubscape_simulation::inventory;
use clubscape_world_engine::{ActorEvent, RandomSource, WorldEngine};

pub fn item(name: &str) -> ItemId {
    ItemId::new(format!("item.synthetic.{name}")).unwrap()
}
pub fn spawn(name: &str) -> SpawnId {
    SpawnId::new(format!("spawn.synthetic.{name}")).unwrap()
}
pub fn object(name: &str) -> ObjectId {
    ObjectId::new(format!("object.synthetic.{name}")).unwrap()
}
pub fn recipe(name: &str) -> RecipeId {
    RecipeId::new(format!("recipe.synthetic.{name}")).unwrap()
}
pub fn stage(name: &str) -> StageId {
    StageId::new(format!("stage.synthetic.{name}")).unwrap()
}
pub fn slot(name: &str) -> SlotId {
    SlotId::new(format!("slot.synthetic.{name}")).unwrap()
}
pub fn actor() -> ActorId {
    ActorId::new("actor.synthetic.one").unwrap()
}
pub fn actor_two() -> ActorId {
    ActorId::new("actor.synthetic.two").unwrap()
}
pub fn skill() -> SkillId {
    SkillId::new("skill.synthetic.practice").unwrap()
}
pub fn hp_skill() -> SkillId {
    SkillId::new("skill.hitpoints").unwrap()
}
pub fn quest() -> QuestId {
    QuestId::new("quest.synthetic.supplies").unwrap()
}
pub fn dialogue() -> DialogueId {
    DialogueId::new("dialogue.synthetic.cook").unwrap()
}
pub fn shop() -> ShopId {
    ShopId::new("shop.synthetic.store").unwrap()
}
pub fn interface() -> InterfaceId {
    InterfaceId::new("interface.synthetic.tab").unwrap()
}
pub fn tile(x: u16, y: u16, plane: u8) -> Tile {
    Tile::new(x, y, plane).unwrap()
}
pub fn quantity(value: u32) -> Quantity {
    Quantity::new(value).unwrap()
}
pub fn stack(name: &str, count: u32) -> ItemStack {
    ItemStack {
        item: item(name),
        quantity: quantity(count),
        instance: None,
    }
}
pub fn certain() -> ChanceRule {
    ChanceRule {
        numerator_at_level_1: 1,
        numerator_at_level_99: 1,
        denominator: 1,
        domain: ChanceDomain::Constant,
    }
}
pub fn never() -> ChanceRule {
    ChanceRule {
        numerator_at_level_1: 0,
        numerator_at_level_99: 0,
        denominator: 1,
        domain: ChanceDomain::Constant,
    }
}
pub fn source() -> Vec<SourceRecord> {
    vec![SourceRecord {
        reference: "synthetic test fixture".into(),
        revision: "synthetic-v1".into(),
        status: EvidenceStatus::TestFixture,
        notes: "No real M1 journey, source-map or presentation claim.".into(),
    }]
}

pub fn content() -> GameContent {
    let mut items = BTreeMap::new();
    for (name, stackable) in [
        ("coins", true),
        ("pick", false),
        ("hammer", false),
        ("ore", false),
        ("tin", false),
        ("bar", false),
        ("dagger", false),
        ("raw", false),
        ("cooked", false),
        ("burnt", false),
        ("flour", false),
        ("water", false),
        ("dough", false),
        ("pot", false),
        ("bucket", false),
        ("pebble", false),
        ("milk", false),
        ("egg", false),
        ("arrow", true),
        ("rune", true),
        ("ore_note", true),
    ] {
        let definition = ItemDefinition {
            id: item(name),
            name: format!("Synthetic {name}"),
            source_id: None,
            stackable: stackable.into(),
            tradable: true,
            base_value: 1,
            equipment: None,
            noted_variant: None,
            unnoted_variant: None,
            healing: None,
            weight: None,
            charges: None,
            asset: None,
            source: source(),
        };
        items.insert(definition.id.clone(), definition);
    }
    for name in ["pick", "dagger"] {
        items.get_mut(&item(name)).unwrap().equipment = Some(EquipmentDefinition {
            slot: slot("weapon"),
            occupied_slots: vec![],
            requirements: vec![],
            bonuses: CombatBonuses::default(),
            attack_speed_ticks: Some(4),
            attack_styles: vec!["accurate".into()],
            weapon: None,
        });
    }
    items.get_mut(&item("arrow")).unwrap().equipment = Some(EquipmentDefinition {
        slot: slot("ammo"),
        occupied_slots: vec![],
        requirements: vec![],
        bonuses: CombatBonuses::default(),
        attack_speed_ticks: None,
        attack_styles: vec![],
        weapon: None,
    });
    items.get_mut(&item("cooked")).unwrap().healing = Some(3);
    items.get_mut(&item("ore")).unwrap().noted_variant = Some(item("ore_note"));
    items.get_mut(&item("ore_note")).unwrap().unnoted_variant = Some(item("ore"));
    let practice = SkillDefinition {
        id: skill(),
        name: "Synthetic practice using selected numeric vectors".into(),
        source_id: 1000,
        xp_thresholds_tenths: vec![0, 830, 1740, 2760, 3880],
        maximum_xp_tenths: 100_000,
        source: source(),
    };
    let hp = SkillDefinition {
        id: hp_skill(),
        name: "Synthetic HP fixture".into(),
        source_id: 3,
        xp_thresholds_tenths: (0..10).map(|level| level * 100).collect(),
        maximum_xp_tenths: 10_000,
        source: source(),
    };
    let mut regions = BTreeMap::new();
    for plane in 0..=1 {
        let id = RegionId::new(format!("region.synthetic.floor_{plane}")).unwrap();
        let mut cells = Vec::new();
        for x in 10..=24 {
            for y in 10..=24 {
                cells.push(CollisionCell {
                    tile: tile(x, y, plane),
                    height: 0,
                    walkable: true,
                    blocked_movement: 0,
                    blocked_sight: 0,
                });
            }
        }
        regions.insert(
            id.clone(),
            RegionDefinition {
                id,
                name: "Synthetic room".into(),
                min: tile(10, 10, plane),
                max: tile(24, 24, plane),
                cells,
                source_map_squares: vec![],
                scene_asset: None,
                source: source(),
            },
        );
    }
    let tutorial = ["start", "next", "last"]
        .into_iter()
        .map(|name| {
            let stage = TutorialStageDefinition {
                id: stage(name),
                instruction: "Synthetic stage".into(),
                allowed_actions: vec!["*".into()],
                xp_caps_tenths: BTreeMap::new(),
                xp_stop_levels: BTreeMap::new(),
                nonfatal_combat: false,
                transitions: vec![],
                source: source(),
            };
            (stage.id.clone(), stage)
        })
        .collect();
    let mut content = GameContent {
        schema_version: CONTENT_SCHEMA_VERSION,
        revision: "synthetic-v1".into(),
        baseline: "synthetic fixture only".into(),
        items,
        skills: BTreeMap::from([(practice.id.clone(), practice), (hp.id.clone(), hp)]),
        regions,
        spawns: BTreeMap::new(),
        objects: BTreeMap::new(),
        npcs: BTreeMap::new(),
        recipes: BTreeMap::new(),
        dialogues: BTreeMap::new(),
        tutorial,
        quests: BTreeMap::new(),
        shops: BTreeMap::new(),
        interfaces: BTreeMap::from([(
            interface(),
            InterfaceDefinition {
                id: interface(),
                name: "Synthetic tab".into(),
                access: InterfaceAccess::Tab,
                source_ids: vec![],
                source: source(),
            },
        )]),
        equipment_slots: vec![slot("weapon"), slot("ammo")],
        initial_state: InitialStateDefinition {
            region: RegionId::new("region.synthetic.floor_0").unwrap(),
            tile: tile(10, 10, 0),
            inventory: Inventory::default(),
            equipment: BTreeMap::new(),
            bank: Bank {
                capacity: 8,
                slots: vec![],
            },
            skills: BTreeMap::from([
                (
                    skill(),
                    SkillState {
                        xp_tenths: 0,
                        current_level: 1,
                    },
                ),
                (
                    hp_skill(),
                    SkillState {
                        xp_tenths: 900,
                        current_level: 10,
                    },
                ),
            ]),
            hitpoints: 10,
            prayer_points: 1,
            run_energy: 10_000,
            tutorial_stage: stage("start"),
            quest_points: 0,
            quests: BTreeMap::new(),
            flags: BTreeMap::new(),
            interfaces: vec![interface()],
            runtime: InitialRuntimeDefinition::default(),
            source: source(),
        },
        mechanics: MechanicsDefinition::default(),
    };
    give_initial(&mut content, &[stack("pick", 1), stack("hammer", 1)]);
    add_object(
        &mut content,
        "rock",
        InteractionAction::Gather {
            rule: Box::new(GatherRule {
                skill: skill(),
                required_level: 1,
                tools: vec![item("pick")],
                output: stack("ore", 1),
                xp_tenths: 175,
                attempt_ticks: Some(8),
                // The source formula's low/high are 100/350; endpoints already include +1.
                success: ChanceRule {
                    numerator_at_level_1: 101,
                    numerator_at_level_99: 351,
                    denominator: 256,
                    domain: ChanceDomain::Skill {
                        levels: LevelDomain {
                            minimum: 1,
                            maximum: 99,
                            basis: SkillLevelBasis::Current,
                        },
                    },
                },
                depletion: certain(),
                respawn_ticks: Some(4),
                mechanics: None,
                animation: None,
                sound: None,
            }),
        },
    );
    add_object(
        &mut content,
        "furnace",
        InteractionAction::Production {
            recipes: vec![recipe("bronze")],
        },
    );
    add_object(
        &mut content,
        "anvil",
        InteractionAction::Production {
            recipes: vec![recipe("dagger")],
        },
    );
    add_object(
        &mut content,
        "range",
        InteractionAction::Production {
            recipes: vec![recipe("cook")],
        },
    );
    add_object(&mut content, "bank", InteractionAction::Bank);
    add_object(
        &mut content,
        "store",
        InteractionAction::Shop { shop: shop() },
    );
    for (name, inputs, outputs, failed, tools, ticks, xp, target) in [
        (
            "bronze",
            vec![stack("ore", 1), stack("tin", 1)],
            vec![stack("bar", 1)],
            vec![],
            vec![],
            6,
            62,
            vec![object("furnace")],
        ),
        (
            "dagger",
            vec![stack("bar", 1)],
            vec![stack("dagger", 1)],
            vec![],
            vec![item("hammer")],
            5,
            125,
            vec![object("anvil")],
        ),
        (
            "cook",
            vec![stack("raw", 1)],
            vec![stack("cooked", 1)],
            vec![stack("burnt", 1)],
            vec![],
            3,
            300,
            vec![object("range")],
        ),
        (
            "dough",
            vec![stack("flour", 1), stack("water", 1)],
            vec![stack("dough", 1), stack("pot", 1), stack("bucket", 1)],
            vec![],
            vec![],
            1,
            0,
            vec![],
        ),
    ] {
        content.recipes.insert(
            recipe(name),
            RecipeDefinition {
                id: recipe(name),
                name: format!("Synthetic {name}"),
                inputs,
                outputs,
                failed_outputs: failed,
                tools,
                requirements: vec![SkillRequirement {
                    skill: skill(),
                    level: 1,
                    basis: SkillLevelBasis::Current,
                }],
                xp: if xp == 0 {
                    vec![]
                } else {
                    vec![XpReward {
                        skill: skill(),
                        amount_tenths: xp,
                    }]
                },
                ticks: Some(ticks),
                success: certain(),
                target_objects: target,
                mechanics: None,
                source: source(),
            },
        );
    }
    content.shops.insert(
        shop(),
        ShopDefinition {
            id: shop(),
            name: "Synthetic fixed-price shop".into(),
            currency: item("coins"),
            stock: vec![
                ShopItem {
                    item: item("pot"),
                    base_stock: 5,
                    restock_ticks: 2,
                    buy_price: 2,
                    sell_price: 1,
                    mechanics: None,
                },
                ShopItem {
                    item: item("arrow"),
                    base_stock: 20,
                    restock_ticks: 4,
                    buy_price: 1,
                    sell_price: 0,
                    mechanics: None,
                },
            ],
            accepts_general_items: true,
            unstocked: None,
            source: source(),
        },
    );
    content
}

pub fn add_object(content: &mut GameContent, name: &str, action: InteractionAction) {
    content.objects.insert(
        object(name),
        ObjectDefinition {
            id: object(name),
            name: format!("Synthetic {name}"),
            source_id: 0,
            size_x: 1,
            size_y: 1,
            clip: None,
            morph: None,
            asset: None,
            source: source(),
        },
    );
    content.spawns.insert(
        spawn(name),
        SpawnDefinition {
            id: spawn(name),
            region: content.initial_state.region.clone(),
            tile: tile(11, 10, 0),
            facing: 0,
            placement: None,
            kind: SpawnKind::Object {
                object: object(name),
            },
            interactions: vec![InteractionDefinition {
                name: "use".into(),
                reach: 1,
                guard: Guard::Always,
                action,
            }],
            source: source(),
        },
    );
}

pub fn add_item_spawn(content: &mut GameContent) {
    content.spawns.insert(
        spawn("egg"),
        SpawnDefinition {
            id: spawn("egg"),
            region: content.initial_state.region.clone(),
            tile: tile(10, 10, 0),
            facing: 0,
            placement: None,
            kind: SpawnKind::Item {
                stack: stack("egg", 1),
                respawn_ticks: 3,
            },
            interactions: vec![],
            source: source(),
        },
    );
}

pub fn add_dialogue(content: &mut GameContent) {
    add_object(
        content,
        "cook",
        InteractionAction::Dialogue {
            dialogue: dialogue(),
        },
    );
    content.dialogues.insert(
        dialogue(),
        DialogueDefinition {
            id: dialogue(),
            nodes: vec![
                DialogueNode {
                    id: "entry".into(),
                    text: "Synthetic greeting".into(),
                    guard: Guard::Always,
                    choices: vec![DialogueChoice {
                        id: "continue".into(),
                        text: "Synthetic continue".into(),
                        guard: Guard::Always,
                        effects: vec![Effect::SetFlag {
                            name: "visited".into(),
                            value: 1,
                        }],
                        next_node: Some("last".into()),
                    }],
                },
                DialogueNode {
                    id: "last".into(),
                    text: "Synthetic final node".into(),
                    guard: Guard::Always,
                    choices: vec![DialogueChoice {
                        id: "finish".into(),
                        text: "Synthetic finish".into(),
                        guard: Guard::Always,
                        effects: vec![Effect::SetFlag {
                            name: "finished".into(),
                            value: 1,
                        }],
                        next_node: None,
                    }],
                },
            ],
            entry_nodes: vec!["entry".into()],
            source: source(),
        },
    );
}

pub fn add_quest(content: &mut GameContent) {
    add_object(
        content,
        "cook",
        InteractionAction::Dialogue {
            dialogue: dialogue(),
        },
    );
    let mut journal = BTreeMap::from([
        (stage("quest_not_started"), "Synthetic not started".into()),
        (stage("quest_completed"), "Synthetic completed".into()),
    ]);
    let mut nodes = vec![DialogueNode {
        id: "offer".into(),
        text: "Synthetic offer".into(),
        guard: quest_guard("quest_not_started"),
        choices: vec![DialogueChoice {
            id: "accept".into(),
            text: "Accept".into(),
            guard: Guard::Always,
            effects: vec![quest_effect("quest_0")],
            next_node: None,
        }],
    }];
    for mask in 0..8_u8 {
        let name = format!("quest_{mask}");
        journal.insert(stage(&name), format!("Synthetic delivered mask {mask}"));
        let mut choices = vec![];
        for (index, ingredient) in ["milk", "flour", "egg"].iter().enumerate() {
            if mask & (1 << index) == 0 {
                choices.push(DialogueChoice {
                    id: format!("deliver_{ingredient}"),
                    text: "Synthetic partial delivery".into(),
                    guard: Guard::HasItems {
                        items: vec![stack(ingredient, 1)],
                    },
                    effects: vec![
                        Effect::TakeItems {
                            items: vec![stack(ingredient, 1)],
                        },
                        quest_effect(&format!("quest_{}", mask | (1 << index))),
                    ],
                    next_node: None,
                });
            }
        }
        if mask == 7 {
            choices.push(DialogueChoice {
                id: "thanks".into(),
                text: "Synthetic reward".into(),
                guard: Guard::Always,
                effects: vec![
                    quest_effect("quest_completed"),
                    Effect::AddQuestPoints { amount: 1 },
                    Effect::AwardXp {
                        rewards: vec![XpReward {
                            skill: skill(),
                            amount_tenths: 3000,
                        }],
                    },
                ],
                next_node: None,
            });
        }
        nodes.push(DialogueNode {
            id: name.clone(),
            text: name.clone(),
            guard: quest_guard(&name),
            choices,
        });
    }
    nodes.push(DialogueNode {
        id: "complete".into(),
        text: "Synthetic complete".into(),
        guard: quest_guard("quest_completed"),
        choices: vec![],
    });
    let entry_nodes = nodes.iter().map(|node| node.id.clone()).collect();
    content.dialogues.insert(
        dialogue(),
        DialogueDefinition {
            id: dialogue(),
            nodes,
            entry_nodes,
            source: source(),
        },
    );
    content.quests.insert(
        quest(),
        QuestDefinition {
            id: quest(),
            name: "Synthetic supplies (not Cook's Assistant assembly)".into(),
            initial_stage: stage("quest_not_started"),
            completed_stage: stage("quest_completed"),
            journal,
            transitions: vec![],
            source: source(),
        },
    );
    content.initial_state.quests.insert(
        quest(),
        QuestState {
            stage: stage("quest_not_started"),
            flags: BTreeMap::new(),
        },
    );
}

pub fn quest_guard(name: &str) -> Guard {
    Guard::QuestStage {
        quest: quest(),
        stage: stage(name),
    }
}
pub fn quest_effect(name: &str) -> Effect {
    Effect::SetQuestStage {
        quest: quest(),
        stage: stage(name),
    }
}
pub fn give_initial(content: &mut GameContent, stacks: &[ItemStack]) {
    inventory::add_batch(&mut content.initial_state.inventory, &content.items, stacks).unwrap();
}
pub fn set_position(content: &mut GameContent, target: &str, at: Tile) {
    content.spawns.get_mut(&spawn(target)).unwrap().tile = at;
}
pub fn cell(content: &mut GameContent, at: Tile) -> &mut CollisionCell {
    content
        .regions
        .values_mut()
        .flat_map(|region| &mut region.cells)
        .find(|cell| cell.tile == at)
        .unwrap()
}
pub fn interaction<'a>(content: &'a mut GameContent, name: &str) -> &'a mut InteractionDefinition {
    &mut content.spawns.get_mut(&spawn(name)).unwrap().interactions[0]
}
pub fn gather_rule(content: &mut GameContent) -> &mut GatherRule {
    match &mut interaction(content, "rock").action {
        InteractionAction::Gather { rule } => rule,
        _ => unreachable!(),
    }
}
pub fn setup(content: GameContent) -> (WorldEngine, WorldState) {
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor(),
        engine
            .character_from_initial(actor(), "Synthetic actor", BTreeMap::new())
            .unwrap(),
    );
    (engine, world)
}
pub fn state(world: &WorldState) -> &CharacterState {
    &world.characters[&actor()]
}
pub fn state_mut(world: &mut WorldState) -> &mut CharacterState {
    world.characters.get_mut(&actor()).unwrap()
}
pub fn count(engine: &WorldEngine, world: &WorldState, name: &str) -> u32 {
    inventory::count(
        &state(world).inventory,
        &engine.content().items,
        &item(name),
    )
    .unwrap()
}
pub fn locate(world: &WorldState, name: &str) -> u8 {
    state(world)
        .inventory
        .slots
        .iter()
        .position(|stack| stack.as_ref().is_some_and(|stack| stack.item == item(name)))
        .unwrap() as u8
}
pub fn interact(name: &str) -> GameIntent {
    GameIntent::Interact {
        target: spawn(name),
        action: "use".into(),
    }
}
pub fn produce(name: &str, target: Option<&str>, count: u32) -> GameIntent {
    GameIntent::Produce {
        recipe: recipe(name),
        target: target.map(spawn),
        quantity: quantity(count),
    }
}
pub fn select(choice: &str) -> GameIntent {
    GameIntent::SelectDialogue {
        speaker: spawn("cook"),
        choice: choice.into(),
    }
}
pub fn apply(engine: &WorldEngine, world: &mut WorldState, intent: GameIntent) -> Vec<ActorEvent> {
    engine
        .apply_intent(world, &actor(), &intent, &mut NeverDraw)
        .unwrap()
}
pub fn next(engine: &WorldEngine, world: &mut WorldState) -> Vec<ActorEvent> {
    engine.tick(world, &mut NeverDraw).unwrap()
}
pub fn ticks(
    engine: &WorldEngine,
    world: &mut WorldState,
    count: usize,
    rng: &mut impl RandomSource,
) -> Vec<ActorEvent> {
    let mut events = vec![];
    for _ in 0..count {
        events.extend(engine.tick(world, rng).unwrap());
    }
    events
}
pub fn error_unchanged(
    engine: &WorldEngine,
    world: &mut WorldState,
    intent: GameIntent,
    code: GameErrorCode,
) {
    let before = world.clone();
    assert_eq!(
        engine
            .apply_intent(world, &actor(), &intent, &mut NeverDraw)
            .unwrap_err()
            .code,
        code
    );
    assert_eq!(*world, before);
}

pub struct NeverDraw;
impl RandomSource for NeverDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("Unexpected random draw")
    }
}
pub struct Fixed(pub u32);
impl RandomSource for Fixed {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        Ok(self.0)
    }
}
#[derive(Clone)]
pub struct Seeded(pub u64);
impl RandomSource for Seeded {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        // Test-only reproducibility. Not exported by the production crate or a protocol.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        Ok((self.0 % u64::from(upper)) as u32)
    }
}
