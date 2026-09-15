//! Deliberately synthetic, small content. Values are not an OSRS XP table or gamepack.

use std::collections::BTreeMap;

use clubscape_game_types::*;

pub fn item(name: &str) -> ItemId {
    ItemId::new(format!("item.synthetic.{name}")).unwrap()
}

pub fn skill(name: &str) -> SkillId {
    SkillId::new(format!("skill.synthetic.{name}")).unwrap()
}

pub fn slot(name: &str) -> SlotId {
    SlotId::new(format!("slot.synthetic.{name}")).unwrap()
}

pub fn quantity(value: u32) -> Quantity {
    Quantity::new(value).unwrap()
}

pub fn stack(name: &str, amount: u32) -> ItemStack {
    ItemStack {
        item: item(name),
        quantity: quantity(amount),
        instance: None,
    }
}

pub fn tile(x: u16, y: u16, plane: u8) -> Tile {
    Tile::new(x, y, plane).unwrap()
}

pub fn source() -> Vec<SourceRecord> {
    vec![SourceRecord {
        reference: "synthetic unit-test fixture; no source claim".into(),
        revision: "synthetic-v1".into(),
        status: EvidenceStatus::TestFixture,
        notes: "Not M1 product content or acceptance evidence.".into(),
    }]
}

pub fn item_definition(name: &str, stackable: bool) -> ItemDefinition {
    ItemDefinition {
        id: item(name),
        name: format!("Synthetic {name}"),
        source_id: None,
        stackable: stackable.into(),
        tradable: true,
        base_value: 0,
        equipment: None,
        noted_variant: None,
        unnoted_variant: None,
        healing: None,
        weight: None,
        charges: None,
        asset: None,
        source: source(),
    }
}

pub fn equipment_definition(primary: &str, occupied: &[&str]) -> EquipmentDefinition {
    EquipmentDefinition {
        slot: slot(primary),
        occupied_slots: occupied.iter().map(|name| slot(name)).collect(),
        requirements: Vec::new(),
        bonuses: CombatBonuses::default(),
        attack_speed_ticks: None,
        attack_styles: Vec::new(),
        weapon: None,
    }
}

pub fn skill_definition() -> SkillDefinition {
    SkillDefinition {
        id: skill("practice"),
        name: "Synthetic practice".into(),
        source_id: 0,
        xp_thresholds_tenths: vec![0, 100, 301, 610, 1000],
        maximum_xp_tenths: 2000,
        source: source(),
    }
}

pub fn region(name: &str, min: Tile, max: Tile) -> RegionDefinition {
    let mut cells = Vec::new();
    for plane in min.plane()..=max.plane() {
        for x in min.x()..=max.x() {
            for y in min.y()..=max.y() {
                cells.push(CollisionCell {
                    tile: tile(x, y, plane),
                    height: 0,
                    walkable: true,
                    blocked_movement: 0,
                    blocked_sight: 0,
                });
            }
        }
    }
    RegionDefinition {
        id: RegionId::new(format!("region.synthetic.{name}")).unwrap(),
        name: format!("Synthetic {name}"),
        min,
        max,
        cells,
        source_map_squares: Vec::new(),
        scene_asset: None,
        source: source(),
    }
}

pub fn cell_mut(region: &mut RegionDefinition, tile: Tile) -> &mut CollisionCell {
    region
        .cells
        .iter_mut()
        .find(|cell| cell.tile == tile)
        .unwrap()
}

pub fn content() -> GameContent {
    let mut items = BTreeMap::new();
    for (name, stackable) in [
        ("tokens", true),
        ("shard", false),
        ("shard_note", true),
        ("pebble", false),
        ("blade", false),
        ("shield", false),
        ("staff", false),
        ("ammo", true),
    ] {
        let definition = item_definition(name, stackable);
        items.insert(definition.id.clone(), definition);
    }
    items.get_mut(&item("shard")).unwrap().noted_variant = Some(item("shard_note"));
    items.get_mut(&item("shard_note")).unwrap().unnoted_variant = Some(item("shard"));
    let mut blade = equipment_definition("weapon", &[]);
    blade.requirements.push(SkillRequirement {
        skill: skill("practice"),
        level: 2,
        basis: SkillLevelBasis::Base,
    });
    items.get_mut(&item("blade")).unwrap().equipment = Some(blade);
    items.get_mut(&item("shield")).unwrap().equipment = Some(equipment_definition("offhand", &[]));
    items.get_mut(&item("staff")).unwrap().equipment =
        Some(equipment_definition("weapon", &["weapon", "offhand"]));
    items.get_mut(&item("ammo")).unwrap().equipment = Some(equipment_definition("ammo", &[]));
    let equipment_slots: Vec<_> = [
        "head",
        "neck",
        "cape",
        "torso",
        "legs",
        "hands",
        "feet",
        "ring",
        "ammo",
        "weapon",
        "offhand",
        "extra_functional_slot",
    ]
    .into_iter()
    .map(slot)
    .collect();
    let definition = skill_definition();
    let skills = BTreeMap::from([(definition.id.clone(), definition)]);
    let skill_states = BTreeMap::from([(
        skill("practice"),
        SkillState {
            xp_tenths: 610,
            current_level: 4,
        },
    )]);
    let stage = TutorialStageDefinition {
        id: StageId::new("stage.synthetic.start").unwrap(),
        instruction: "Synthetic test stage, not a tutorial implementation.".into(),
        allowed_actions: Vec::new(),
        xp_caps_tenths: BTreeMap::new(),
        xp_stop_levels: BTreeMap::new(),
        nonfatal_combat: false,
        transitions: Vec::new(),
        source: source(),
    };
    let region = region("room", tile(10, 10, 0), tile(16, 16, 0));
    GameContent {
        schema_version: CONTENT_SCHEMA_VERSION,
        ui: None,
        revision: "synthetic-v1".into(),
        baseline: "synthetic only; not a gamepack".into(),
        interfaces: BTreeMap::new(),
        items,
        skills,
        regions: BTreeMap::from([(region.id.clone(), region.clone())]),
        spawns: BTreeMap::new(),
        objects: BTreeMap::new(),
        npcs: BTreeMap::new(),
        recipes: BTreeMap::new(),
        dialogues: BTreeMap::new(),
        tutorial: BTreeMap::from([(stage.id.clone(), stage.clone())]),
        quests: BTreeMap::new(),
        shops: BTreeMap::new(),
        equipment_slots,
        initial_state: InitialStateDefinition {
            region: region.id,
            tile: region.min,
            inventory: Inventory::default(),
            equipment: BTreeMap::new(),
            bank: Bank {
                capacity: 8,
                slots: Vec::new(),
            },
            skills: skill_states,
            hitpoints: 3,
            prayer_points: 2,
            run_energy: 11,
            tutorial_stage: stage.id,
            quest_points: 7,
            quests: BTreeMap::new(),
            flags: BTreeMap::from([("synthetic_progress".into(), 41)]),
            interfaces: Vec::new(),
            runtime: InitialRuntimeDefinition::default(),
            source: source(),
        },
        mechanics: MechanicsDefinition::default(),
    }
}

pub fn character(content: &GameContent) -> CharacterState {
    let initial = &content.initial_state;
    CharacterState {
        schema_version: GAME_SCHEMA_VERSION,
        actor_id: ActorId::new("actor.synthetic.tester").unwrap(),
        display_name: "Synthetic tester".into(),
        appearance: BTreeMap::from([("synthetic_choice".into(), 0)]),
        region: initial.region.clone(),
        tile: initial.tile,
        inventory: initial.inventory.clone(),
        equipment: initial.equipment.clone(),
        bank: initial.bank.clone(),
        skills: initial.skills.clone(),
        hitpoints: initial.hitpoints,
        prayer_points: initial.prayer_points,
        run_energy: initial.run_energy,
        quest_points: initial.quest_points,
        tutorial_stage: initial.tutorial_stage.clone(),
        quests: initial.quests.clone(),
        flags: initial.flags.clone(),
        interfaces: initial.interfaces.clone(),
        activity: Activity::Idle,
        dialogue: None,
        last_action_tick: 12,
        last_command_sequence: 7,
        runtime: CharacterRuntime::from_initial(content),
    }
}

pub fn put(inventory: &mut Inventory, slot: usize, stack: ItemStack) {
    *inventory.slots.get_mut(slot).unwrap() = Some(stack);
}

pub fn filled(name: &str) -> Inventory {
    Inventory {
        slots: std::array::from_fn(|_| Some(stack(name, 1))),
    }
}

pub fn assert_error<T: std::fmt::Debug>(result: GameResult<T>, code: GameErrorCode) {
    assert_eq!(result.unwrap_err().code, code);
}

pub fn totals<'a>(stacks: impl IntoIterator<Item = &'a ItemStack>) -> BTreeMap<ItemId, u64> {
    let mut totals = BTreeMap::new();
    for stack in stacks {
        *totals.entry(stack.item.clone()).or_default() += u64::from(stack.quantity.get());
    }
    totals
}

pub fn normalized_totals(
    character: &CharacterState,
    content: &GameContent,
) -> BTreeMap<ItemId, u64> {
    let mut result = BTreeMap::new();
    for stack in character
        .inventory
        .slots
        .iter()
        .flatten()
        .chain(character.equipment.values())
        .chain(character.bank.slots.iter().flatten())
    {
        let definition = content.items.get(&stack.item).unwrap();
        let base = definition.unnoted_variant.as_ref().unwrap_or(&stack.item);
        *result.entry(base.clone()).or_default() += u64::from(stack.quantity.get());
    }
    result
}
