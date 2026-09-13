use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    ActorId, DialogueId, GameError, GameErrorCode, GameResult, INVENTORY_SLOTS, InterfaceId,
    ItemId, MAX_STACK_QUANTITY, QuestId, RecipeId, RegionId, ShopId, SkillId, SlotId, SpawnId,
    StageId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "TileCoordinates", into = "TileCoordinates")]
pub struct Tile {
    x: u16,
    y: u16,
    plane: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileCoordinates {
    pub x: u16,
    pub y: u16,
    pub plane: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    East,
    South,
    West,
    NorthEast,
    SouthEast,
    SouthWest,
    NorthWest,
}

impl Direction {
    pub const ALL: [Self; 8] = [
        Self::North,
        Self::East,
        Self::South,
        Self::West,
        Self::NorthEast,
        Self::SouthEast,
        Self::SouthWest,
        Self::NorthWest,
    ];

    pub fn offset(self) -> (i16, i16) {
        match self {
            Self::North => (0, 1),
            Self::East => (1, 0),
            Self::South => (0, -1),
            Self::West => (-1, 0),
            Self::NorthEast => (1, 1),
            Self::SouthEast => (1, -1),
            Self::SouthWest => (-1, -1),
            Self::NorthWest => (-1, 1),
        }
    }

    pub fn mask(self) -> u8 {
        match self {
            Self::North => 1,
            Self::East => 2,
            Self::South => 4,
            Self::West => 8,
            Self::NorthEast => 16,
            Self::SouthEast => 32,
            Self::SouthWest => 64,
            Self::NorthWest => 128,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::NorthEast => Self::SouthWest,
            Self::SouthEast => Self::NorthWest,
            Self::SouthWest => Self::NorthEast,
            Self::NorthWest => Self::SouthEast,
        }
    }
}

impl Tile {
    pub fn new(x: u16, y: u16, plane: u8) -> GameResult<Self> {
        if x >= 16_384 || y >= 16_384 || plane > 3 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "World coordinates require tiles below 16384 and a plane from 0 through 3.",
            ));
        }
        Ok(Self { x, y, plane })
    }

    pub fn x(self) -> u16 {
        self.x
    }
    pub fn y(self) -> u16 {
        self.y
    }
    pub fn plane(self) -> u8 {
        self.plane
    }

    pub fn distance(self, other: Self) -> Option<u16> {
        (self.plane == other.plane).then(|| self.x.abs_diff(other.x).max(self.y.abs_diff(other.y)))
    }

    pub fn offset(self, dx: i16, dy: i16) -> Option<Self> {
        Self::new(
            self.x.checked_add_signed(dx)?,
            self.y.checked_add_signed(dy)?,
            self.plane,
        )
        .ok()
    }
}

impl TryFrom<TileCoordinates> for Tile {
    type Error = GameError;

    fn try_from(value: TileCoordinates) -> Result<Self, Self::Error> {
        Self::new(value.x, value.y, value.plane)
    }
}

impl From<Tile> for TileCoordinates {
    fn from(value: Tile) -> Self {
        Self {
            x: value.x,
            y: value.y,
            plane: value.plane,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Quantity(u32);

impl Quantity {
    pub fn new(value: u32) -> GameResult<Self> {
        if value == 0 || value > MAX_STACK_QUANTITY {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Item quantity is out of range.",
            ));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for Quantity {
    type Error = GameError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Quantity> for u32 {
    fn from(value: Quantity) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemId,
    pub quantity: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: [Option<ItemStack>; INVENTORY_SLOTS],
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bank {
    pub capacity: u16,
    pub slots: Vec<Option<ItemStack>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillState {
    pub xp_tenths: u64,
    pub current_level: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestState {
    pub stage: StageId,
    pub flags: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Activity {
    Idle,
    Walking {
        path: Vec<Tile>,
        running: bool,
    },
    Gathering {
        target: SpawnId,
        next_tick: u64,
    },
    Producing {
        recipe: RecipeId,
        target: Option<SpawnId>,
        remaining: u32,
        next_tick: u64,
    },
    Fighting {
        target: SpawnId,
        style: String,
        next_tick: u64,
    },
    Casting {
        spell: String,
        target: Option<SpawnId>,
        completes_at: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenDialogue {
    pub id: DialogueId,
    pub speaker: SpawnId,
    pub node: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterState {
    pub schema_version: u32,
    pub actor_id: ActorId,
    pub display_name: String,
    pub appearance: BTreeMap<String, u32>,
    pub region: RegionId,
    pub tile: Tile,
    pub inventory: Inventory,
    pub equipment: BTreeMap<SlotId, ItemStack>,
    pub bank: Bank,
    pub skills: BTreeMap<SkillId, SkillState>,
    pub hitpoints: u16,
    pub prayer_points: u16,
    pub run_energy: u16,
    pub quest_points: u32,
    pub tutorial_stage: StageId,
    pub quests: BTreeMap<QuestId, QuestState>,
    pub flags: BTreeMap<String, i64>,
    pub interfaces: Vec<InterfaceId>,
    pub activity: Activity,
    pub dialogue: Option<OpenDialogue>,
    pub last_action_tick: u64,
    pub last_command_sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundItem {
    pub id: String,
    pub tile: Tile,
    pub stack: ItemStack,
    pub owner: Option<ActorId>,
    pub public_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityState {
    pub tile: Tile,
    pub hitpoints: u16,
    pub available_at_tick: u64,
    pub flags: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopState {
    pub stock: BTreeMap<ItemId, u32>,
    pub next_restock_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldState {
    pub schema_version: u32,
    pub content_revision: String,
    pub tick: u64,
    pub revision: u64,
    pub characters: BTreeMap<ActorId, CharacterState>,
    pub entities: BTreeMap<SpawnId, EntityState>,
    pub shops: BTreeMap<ShopId, ShopState>,
    pub ground_items: Vec<GroundItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_check_edges_planes_and_diagonal_distance() {
        let tile = Tile::new(3200, 3200, 0).unwrap();
        assert_eq!(tile.distance(Tile::new(3203, 3204, 0).unwrap()), Some(4));
        assert_eq!(tile.distance(Tile::new(3200, 3200, 1).unwrap()), None);
        assert!(Tile::new(16384, 0, 0).is_err());
        assert!(Tile::new(0, 0, 4).is_err());
        assert!(Tile::new(0, 0, 0).unwrap().offset(-1, 0).is_none());
        assert!(serde_json::from_str::<Tile>(r#"{"x":0,"y":0,"plane":4}"#).is_err());
    }

    #[test]
    fn quantities_and_inventory_shape_reject_invalid_input() {
        assert!(Quantity::new(0).is_err());
        assert!(Quantity::new(MAX_STACK_QUANTITY + 1).is_err());
        assert_eq!(
            Quantity::new(MAX_STACK_QUANTITY).unwrap().get(),
            MAX_STACK_QUANTITY
        );
        assert!(serde_json::from_str::<Quantity>("0").is_err());
        assert!(serde_json::from_str::<Inventory>(r#"{"slots":[]}"#).is_err());
        let inventory = Inventory::default();
        assert_eq!(inventory.slots.len(), 28);
        assert_eq!(
            serde_json::from_str::<Inventory>(&serde_json::to_string(&inventory).unwrap()).unwrap(),
            inventory
        );
    }
}
