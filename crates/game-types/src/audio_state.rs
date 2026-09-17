use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAuthorityDefinition {
    pub version: u32,
    pub profile: String,
    pub areas: BTreeMap<String, MusicAreaDefinition>,
    pub tracks: BTreeMap<u32, MusicTrackDefinition>,
    pub varps: BTreeMap<u32, NativeVarpDefinition>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicAreaDefinition {
    pub plane: u8,
    /// Original half-tile coordinates multiplied by two, avoiding floating boundary decisions.
    pub polygons: Vec<Vec<[i32; 2]>>,
    pub source: Vec<SourceRecord>,
}

impl MusicAreaDefinition {
    pub fn contains(&self, tile: Tile) -> bool {
        if tile.plane() != self.plane {
            return false;
        }
        let x = i64::from(tile.x()) * 2;
        let y = i64::from(tile.y()) * 2;
        self.polygons.iter().any(|polygon| {
            let mut inside = false;
            for (a, b) in polygon
                .iter()
                .zip(polygon.iter().cycle().skip(1))
                .take(polygon.len())
            {
                let [ax, ay] = a.map(i64::from);
                let [bx, by] = b.map(i64::from);
                let cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
                if cross == 0
                    && x >= ax.min(bx)
                    && x <= ax.max(bx)
                    && y >= ay.min(by)
                    && y <= ay.max(by)
                {
                    return true;
                }
                if (ay > y) != (by > y) && (cross < 0) == (by > ay) {
                    inside = !inside;
                }
            }
            inside
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicTrackDefinition {
    pub group: u32,
    pub name: String,
    pub automatic: bool,
    pub area: Option<String>,
    pub conserved: Option<CounterId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVarpDefinition {
    pub binding: String,
    pub fields: Vec<NativeVarpField>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVarpField {
    pub lsb: u8,
    pub width: u8,
    pub cases: Vec<NativeVarpCase>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeVarpCase {
    pub guard: Guard,
    pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAuthorityRuntime {
    pub version: u32,
    pub profile: String,
    pub history: MusicHistoryStatus,
    pub tracked_from_tick: u64,
    pub revision: u64,
    pub unlocks: BTreeMap<u32, MusicConfirmation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicConfirmation {
    pub tick: u64,
    pub rule: String,
}

impl AudioAuthorityRuntime {
    pub fn validate_successor(&self, next: &Self) -> GameResult<()> {
        next.validate_shape()?;
        if self.version != next.version
            || self.profile != next.profile
            || self.history != next.history
            || self.tracked_from_tick != next.tracked_from_tick
            || next.revision < self.revision
            || self
                .unlocks
                .iter()
                .any(|(group, entry)| next.unlocks.get(group) != Some(entry))
            || (self.unlocks == next.unlocks && self.revision != next.revision)
            || (self.unlocks != next.unlocks && self.revision >= next.revision)
        {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Confirmed source audio history cannot be removed, rewritten or reset.",
            ));
        }
        Ok(())
    }

    pub fn validate_shape(&self) -> GameResult<()> {
        if self.version != AUDIO_AUTHORITY_VERSION
            || self.profile.is_empty()
            || self.profile.len() > 160
            || self.tracked_from_tick > i64::MAX as u64
            || self.unlocks.len() > 128
            || self.revision > self.unlocks.len() as u64
            || (self.revision == 0) != self.unlocks.is_empty()
            || self.unlocks.iter().any(|(group, confirmation)| {
                *group == 0
                    || *group > 65534
                    || confirmation.tick < self.tracked_from_tick
                    || confirmation.tick > i64::MAX as u64
                    || confirmation.rule.is_empty()
                    || confirmation.rule.len() > 192
            })
        {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Invalid source audio history.",
            ));
        }
        Ok(())
    }

    pub fn validate_definition(&self, definition: &AudioAuthorityDefinition) -> GameResult<()> {
        self.validate_shape()?;
        if self.profile != definition.profile {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Source audio history needs its explicitly migrated profile.",
            ));
        }
        for (group, confirmation) in &self.unlocks {
            let track = definition.tracks.get(group).ok_or_else(|| {
                GameError::new(
                    GameErrorCode::InvalidInput,
                    "Confirmed music no longer exists in the source profile.",
                )
            })?;
            let valid = track.automatic && confirmation.rule == format!("music.{group}.automatic")
                || track
                    .area
                    .as_ref()
                    .is_some_and(|area| confirmation.rule == format!("music.{group}.area.{area}"))
                || track.conserved.is_some()
                    && confirmation.rule == format!("music.{group}.conserved");
            if !valid {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Confirmed music lost its bound source rule.",
                ));
            }
        }
        Ok(())
    }
}
