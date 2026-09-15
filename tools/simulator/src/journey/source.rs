use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::Read,
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use clubscape_protocol::game;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("Read {}", path.display()))?;
    ensure!(
        bytes.len() as u64 <= MAX_SOURCE_BYTES,
        "Source input too large"
    );
    let bytes = if path.extension().is_some_and(|extension| extension == "gz") {
        let mut decoded = Vec::new();
        GzDecoder::new(bytes.as_slice())
            .take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut decoded)?;
        ensure!(
            decoded.len() as u64 <= MAX_SOURCE_BYTES,
            "Expanded source input too large"
        );
        decoded
    } else {
        bytes
    };
    serde_json::from_slice(&bytes).with_context(|| format!("Decode {}", path.display()))
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub plane: u32,
}

impl Tile {
    pub const fn new(x: u32, y: u32, plane: u32) -> Self {
        Self { x, y, plane }
    }

    pub fn wire(self) -> game::Tile {
        game::Tile {
            x: self.x,
            y: self.y,
            plane: self.plane,
        }
    }

    pub fn distance(self, other: Self) -> u32 {
        if self.plane != other.plane {
            u32::MAX
        } else {
            self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
        }
    }

    fn offset(self, x: i32, y: i32) -> Option<Self> {
        Some(Self {
            x: self.x.checked_add_signed(x)?,
            y: self.y.checked_add_signed(y)?,
            ..self
        })
    }
}

impl From<&game::Tile> for Tile {
    fn from(tile: &game::Tile) -> Self {
        Self::new(tile.x, tile.y, tile.plane)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct Cell {
    tile: Tile,
    walkable: bool,
    blocked_movement: u8,
}

#[derive(Clone)]
pub struct Navigation {
    cells: BTreeMap<Tile, Cell>,
}

impl Navigation {
    pub fn from_content(content: &Value) -> Result<Self> {
        let mut cells = BTreeMap::new();
        for region in content["regions"]
            .as_object()
            .context("Missing source regions")?
            .values()
        {
            for value in region["cells"]
                .as_array()
                .context("Missing source collision cells")?
            {
                let cell: Cell = serde_json::from_value(value.clone())?;
                ensure!(
                    cells.insert(cell.tile, cell).is_none(),
                    "Duplicate source navigation tile"
                );
            }
        }
        ensure!(
            !cells.is_empty() && cells.len() <= 1_000_000,
            "Unbounded/empty source navigation"
        );
        Ok(Self { cells })
    }

    pub fn walkable(&self, tile: Tile) -> bool {
        self.cells.get(&tile).is_some_and(|cell| cell.walkable)
    }

    pub fn replace(&mut self, collision: &Value) -> Result<()> {
        for value in collision
            .as_array()
            .context("Missing explicit transform collision")?
        {
            let cell: Cell = serde_json::from_value(value.clone())?;
            ensure!(
                self.cells.contains_key(&cell.tile),
                "Transform invented a navigation tile"
            );
            self.cells.insert(cell.tile, cell);
        }
        Ok(())
    }

    pub fn step(&self, from: Tile, to: Tile) -> bool {
        if !self.walkable(from) || !self.walkable(to) || from.plane != to.plane {
            return false;
        }
        self.contact_clear(from, to)
    }

    fn contact_clear(&self, from: Tile, to: Tile) -> bool {
        if from.plane != to.plane
            || !self.cells.contains_key(&from)
            || !self.cells.contains_key(&to)
        {
            return false;
        }
        let direction = match (
            i64::from(to.x) - i64::from(from.x),
            i64::from(to.y) - i64::from(from.y),
        ) {
            (0, 1) => (1, 4),
            (1, 0) => (2, 8),
            (0, -1) => (4, 1),
            (-1, 0) => (8, 2),
            _ => return false,
        };
        self.cells[&from].blocked_movement & direction.0 == 0
            && self.cells[&to].blocked_movement & direction.1 == 0
    }

    // Cardinal routes are conservative input plans, not engine path/permission assertions.
    pub fn route(&self, start: Tile, goals: &BTreeSet<Tile>) -> Option<Vec<Tile>> {
        if !self.walkable(start) || goals.is_empty() {
            return None;
        }
        let mut seen = BTreeMap::from([(start, start)]);
        let mut queue = VecDeque::from([start]);
        while let Some(tile) = queue.pop_front() {
            if goals.contains(&tile) {
                let mut route = vec![tile];
                while route.last() != Some(&start) {
                    route.push(seen[route.last()?]);
                }
                route.reverse();
                return Some(route);
            }
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if let Some(next) = tile.offset(dx, dy)
                    && !seen.contains_key(&next)
                    && self.step(tile, next)
                {
                    seen.insert(next, tile);
                    queue.push_back(next);
                }
            }
        }
        None
    }
}

pub struct Source {
    pub content: Value,
    pub oracle: Value,
    pub tutorial: Value,
    pub initial: Value,
    pub activities: Value,
    pub travels: Value,
    pub identity: Value,
    pub navigation: Navigation,
}

impl Source {
    pub fn load(root: &Path) -> Result<Self> {
        let oracle = read_json(&root.join("tests/scenarios/m1_fresh_account.json"))?;
        ensure!(
            oracle["name"] == "m1_fresh_account",
            "Wrong named scenario oracle"
        );
        let mut hashes = BTreeMap::new();
        for name in oracle["source_oracles"]
            .as_array()
            .context("Missing source oracle list")?
        {
            let name = name.as_str().context("Invalid source oracle path")?;
            ensure!(
                !name.contains("..") && !Path::new(name).is_absolute(),
                "Unsafe source path"
            );
            hashes.insert(name.to_owned(), hash(&fs::read(root.join(name))?));
        }
        for name in [
            "content/m1/manifest.json",
            "content/m1/game-content.json.gz",
            "content/m1/game-content.csc.gz",
            "tests/scenarios/m1_fresh_account.json",
            "crates/protocol/proto/account.proto",
            "crates/protocol/proto/game.proto",
        ] {
            hashes.insert(name.to_owned(), hash(&fs::read(root.join(name))?));
        }
        let content = read_json(&root.join("content/m1/game-content.json.gz"))?;
        let manifest = read_json(&root.join("content/m1/manifest.json"))?;
        ensure!(
            content["revision"] == manifest["revision"],
            "Content/manifest revision mismatch"
        );
        let tutorial = read_json(&root.join("research/journey-rules/tutorial.json"))?;
        let initial = read_json(&root.join("research/journey-rules/initial-state.json"))?;
        let activities = read_json(&root.join("research/journey-rules/activities.json"))?;
        let travels = read_json(&root.join("research/m1-bindings/travel-bindings.json"))?;
        ensure!(
            tutorial["states"]
                .as_array()
                .context("Missing tutorial states")?
                .len()
                == 71
                && tutorial["transitions"]
                    .as_array()
                    .context("Missing tutorial edges")?
                    .len()
                    == 73,
            "Source graph changed; review the explicit 71-state/73-edge plan"
        );
        let identity = json!({
            "file_sha256": hashes,
            "content_revision": content["revision"],
            "content_schema_version": content["schema_version"],
            "content_artifact": manifest["compiled_artifact"],
            "baseline": manifest["baseline"],
            "protocol_version": clubscape_protocol::PROTOCOL_VERSION,
            "generated_descriptor_sha256": hash(clubscape_protocol::FILE_DESCRIPTOR_SET),
            "source_oracle_policy": "Expected values come from independently authored source contracts/scenario literals, never engine execution or seeded state.",
            "source_classification_limits": "Retains source-contract assumptions/inferences; this run cannot promote them to owner approval."
        });
        let navigation = Navigation::from_content(&content)?;
        Ok(Self {
            content,
            oracle,
            tutorial,
            initial,
            activities,
            travels,
            identity,
            navigation,
        })
    }

    pub fn revision(&self) -> Result<&str> {
        self.content["revision"]
            .as_str()
            .context("Missing content revision")
    }

    pub fn number(&self, pointer: &str) -> Result<u64> {
        self.oracle
            .pointer(pointer)
            .and_then(Value::as_u64)
            .with_context(|| format!("Missing independent numeric oracle {pointer}"))
    }

    pub fn source_rule(&self, id: &str) -> Result<&Value> {
        self.activities["rules"]
            .as_array()
            .context("Missing source rules")?
            .iter()
            .find(|rule| rule["id"] == id)
            .with_context(|| format!("Unknown source rule {id}"))
    }

    pub fn tutorial_edge(&self, from: &str, to: &str) -> Result<&Value> {
        self.tutorial["transitions"]
            .as_array()
            .context("Missing source graph")?
            .iter()
            .find(|edge| edge["from_ref"] == from && edge["to_ref"] == to)
            .with_context(|| format!("Not a source tutorial edge: {from} -> {to}"))
    }

    pub fn spawn(&self, id: &str) -> Result<&Value> {
        self.content["spawns"]
            .get(id)
            .with_context(|| format!("Missing exact source spawn {id}"))
    }

    pub fn spawn_tile(&self, id: &str) -> Result<Tile> {
        serde_json::from_value(self.spawn(id)?["tile"].clone()).context("Invalid source spawn tile")
    }

    pub fn travel_destination(&self, spawn: &str, action: &str) -> Result<Tile> {
        for link in self.travels["links"]
            .as_array()
            .context("Missing source travel links")?
        {
            if let Some(index) = link["spawn_endpoints"]
                .as_array()
                .context("Missing source endpoints")?
                .iter()
                .position(|endpoint| endpoint == spawn)
            {
                ensure!(index < 2, "Unsupported source travel endpoint count");
                if link["actions"][index] != action {
                    continue;
                }
                let point = link["landing_candidates"][1 - index]
                    .as_array()
                    .context("Missing source landing candidate")?;
                ensure!(point.len() == 3, "Invalid source travel coordinate");
                return Ok(Tile::new(
                    point[0].as_u64().context("Invalid landing X")? as u32,
                    point[1].as_u64().context("Invalid landing Y")? as u32,
                    point[2].as_u64().context("Invalid landing plane")? as u32,
                ));
            }
        }
        bail!("No independent source travel binding for {spawn}")
    }

    pub fn source_identity(&self, id: &str) -> Result<Value> {
        let value = self.spawn(id)?;
        let (family, definition) = match value["kind"]["kind"].as_str() {
            Some("npc") => ("npcs", value["kind"]["npc"].as_str()),
            Some("object") => ("objects", value["kind"]["object"].as_str()),
            Some("item") => ("items", value["kind"]["stack"]["item"].as_str()),
            _ => bail!("Unknown source spawn kind"),
        };
        let definition = definition.context("Missing source definition ID")?;
        let data = &self.content[family][definition];
        Ok(json!({
            "spawn": id, "definition": definition, "source_id": data["source_id"],
            "asset": data["asset"], "tile": value["tile"], "placement": value["placement"]
        }))
    }

    pub fn interaction(&self, id: &str, action: &str) -> Result<&Value> {
        self.spawn(id)?["interactions"]
            .as_array()
            .context("Missing source interactions")?
            .iter()
            .find(|entry| entry["name"] == action)
            .with_context(|| format!("No source action {action} for {id}"))
    }

    pub fn target_goals(
        &self,
        id: &str,
        action: &str,
        live: Option<&game::Entity>,
        navigation: &Navigation,
    ) -> Result<BTreeSet<Tile>> {
        let spawn = self.spawn(id)?;
        let tile = live
            .and_then(|entity| entity.tile.as_ref())
            .map(Tile::from)
            .unwrap_or(self.spawn_tile(id)?);
        let reach = self.interaction(id, action)?["reach"]
            .as_u64()
            .context("Missing source reach")?;
        ensure!((1..=32).contains(&reach), "Unexpected interaction reach");
        let (mut width, mut height, mut blocked) = (1, 1, 0);
        match spawn["kind"]["kind"].as_str() {
            Some("npc") => {
                let definition = &self.content["npcs"]
                    [spawn["kind"]["npc"].as_str().context("Missing NPC ID")?];
                if let Some(access) = definition
                    .pointer("/navigation/anchor/access_tiles")
                    .and_then(Value::as_array)
                {
                    return access
                        .iter()
                        .map(|tile| serde_json::from_value(tile.clone()).map_err(Into::into))
                        .collect();
                }
                width = definition["size"]
                    .as_u64()
                    .context("Missing NPC footprint")? as u32;
                height = width;
            }
            Some("object") => {
                let definition_id = live
                    .map(|entity| entity.definition_id.as_str())
                    .filter(|id| self.content["objects"].get(*id).is_some())
                    .or_else(|| spawn["kind"]["object"].as_str())
                    .context("Missing object ID")?;
                let definition = &self.content["objects"][definition_id];
                width = definition["size_x"]
                    .as_u64()
                    .context("Missing object width")? as u32;
                height = definition["size_y"]
                    .as_u64()
                    .context("Missing object height")? as u32;
                blocked = definition
                    .pointer("/clip/access_blocked_sides")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u8;
                let turns = spawn["placement"]["quarter_turns"].as_u64().unwrap_or(0);
                for _ in 0..turns {
                    blocked = ((blocked << 1) & 15) | ((blocked >> 3) & 1);
                }
                if turns % 2 == 1 {
                    (width, height) = (height, width);
                }
            }
            Some("item") => {}
            _ => bail!("Unsupported source target kind"),
        }
        ensure!(
            (1..=64).contains(&width) && (1..=64).contains(&height),
            "Invalid target footprint"
        );
        let mut goals = BTreeSet::new();
        for dx in -(reach as i32)..(width as i32 + reach as i32) {
            for dy in -(reach as i32)..(height as i32 + reach as i32) {
                let side = if dx < 0 && (0..height as i32).contains(&dy) {
                    8
                } else if dx >= width as i32 && (0..height as i32).contains(&dy) {
                    2
                } else if dy < 0 && (0..width as i32).contains(&dx) {
                    4
                } else if dy >= height as i32 && (0..width as i32).contains(&dx) {
                    1
                } else if (0..width as i32).contains(&dx) && (0..height as i32).contains(&dy) {
                    0
                } else {
                    continue;
                };
                if side & blocked == 0
                    && let Some(candidate) = tile.offset(dx, dy)
                    && navigation.walkable(candidate)
                {
                    if side == 0 && spawn["kind"]["kind"] == "npc" {
                        continue;
                    }
                    if reach == 1 && side != 0 && spawn["kind"]["kind"] == "npc" {
                        let contact = match side {
                            8 => candidate.offset(1, 0),
                            2 => candidate.offset(-1, 0),
                            4 => candidate.offset(0, 1),
                            1 => candidate.offset(0, -1),
                            _ => None,
                        };
                        if contact
                            .is_none_or(|contact| !navigation.contact_clear(candidate, contact))
                        {
                            continue;
                        }
                    }
                    goals.insert(candidate);
                }
            }
        }
        ensure!(!goals.is_empty(), "No source access candidate for {id}");
        Ok(goals)
    }

    pub fn navigation_with_states(
        &self,
        observed: &BTreeMap<String, String>,
        open_candidates: bool,
    ) -> Result<Navigation> {
        let mut navigation = self.navigation.clone();
        let transforms = self.content["mechanics"]["object_transforms"]
            .as_object()
            .context("Missing door transforms")?;
        let mut selections = BTreeMap::new();
        for (id, transform) in transforms {
            let initial = transform["initial"]
                .as_str()
                .context("Missing transform initial state")?;
            let state = if open_candidates
                && transform["states"].get("object_state.open").is_some()
                && self
                    .interaction(
                        transform["spawn"]
                            .as_str()
                            .context("Missing transform spawn")?,
                        "Open",
                    )
                    .is_ok()
            {
                "object_state.open"
            } else {
                observed.get(id).map(String::as_str).unwrap_or(initial)
            };
            selections.insert(id.clone(), state.to_owned());
        }
        let mut grouped = BTreeSet::new();
        if let Some(groups) = self.content["mechanics"]["collision_groups"].as_object() {
            for group in groups.values() {
                let members = group["transforms"]
                    .as_array()
                    .context("Missing collision group members")?;
                let required: BTreeMap<_, _> = members
                    .iter()
                    .map(|member| {
                        let id = member.as_str().context("Bad collision group member")?;
                        Ok((
                            id.to_owned(),
                            selections
                                .get(id)
                                .context("Missing group transform")?
                                .clone(),
                        ))
                    })
                    .collect::<Result<_>>()?;
                // Version-3 combinations are explicit, never a union of incompatible leaf states.
                let states = group["states"]["value"]
                    .as_array()
                    .context("Unbound collision-group states")?;
                let state = states
                    .iter()
                    .find(|candidate| {
                        serde_json::from_value::<BTreeMap<String, String>>(
                            candidate["selection"].clone(),
                        )
                        .ok()
                        .as_ref()
                            == Some(&required)
                    })
                    .context("No declared collision combination for observed door state")?;
                navigation.replace(&state["collision"])?;
                grouped.extend(required.into_keys());
            }
        }
        for (id, state) in selections {
            if !grouped.contains(&id) {
                navigation.replace(&transforms[&id]["states"][state]["collision"])?;
            }
        }
        Ok(navigation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_navigation() -> Navigation {
        Navigation::from_content(&json!({"regions":{"test":{"cells":[
            {"tile":{"x":1,"y":1,"plane":0},"walkable":true,"blocked_movement":2},
            {"tile":{"x":2,"y":1,"plane":0},"walkable":true,"blocked_movement":8},
            {"tile":{"x":1,"y":2,"plane":0},"walkable":true,"blocked_movement":0}
        ]}}}))
        .unwrap()
    }

    #[test]
    fn source_walls_missing_tiles_and_planes_are_not_walkable_shortcuts() {
        let map = tiny_navigation();
        let start = Tile::new(1, 1, 0);
        assert!(!map.step(start, Tile::new(2, 1, 0)));
        assert!(!map.step(start, Tile::new(1, 2, 1)));
        assert!(!map.step(start, Tile::new(2, 2, 0)));
        assert!(
            map.route(start, &BTreeSet::from([Tile::new(2, 1, 0)]))
                .is_none()
        );
        assert_eq!(
            map.route(start, &BTreeSet::from([Tile::new(1, 2, 0)]))
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn collision_replacements_cannot_create_new_cells() {
        let mut map = tiny_navigation();
        assert!(
            map.replace(&json!([{
                "tile":{"x":500,"y":500,"plane":0},"walkable":true,"blocked_movement":0
            }]))
            .is_err()
        );
    }

    #[test]
    fn numeric_oracles_match_independent_source_rules() {
        let source = Source::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        )
        .unwrap();
        for activity in ["fishing", "woodcutting", "tin", "copper"] {
            let oracle = &source.oracle["activities"][activity];
            let rule = source
                .source_rule(oracle["source_rule"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                rule["xp_awards"][0]["xp_tenths"],
                oracle["xp_tenths_per_item"]
            );
            assert_eq!(rule["xp_awards"][0]["skill_ref"], oracle["skill"]);
        }
        assert_eq!(
            source.source_rule("rule.smelting.bronze").unwrap()["xp_awards"][0]["xp_tenths"],
            62
        );
    }

    #[test]
    fn chosen_source_targets_have_conservative_real_access_candidates() {
        let source = Source::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let map = source
            .navigation_with_states(&BTreeMap::new(), false)
            .unwrap();
        for (id, action) in [
            ("spawn.gielinor_guide", "Talk-to"),
            ("spawn.tutorial.fishing_spot.3099.3090.p0", "Net"),
            ("spawn.tree.tutorial.3105.3093.p0.t10.r3", "Chop down"),
            ("spawn.rock.copper.3228.3144.p0.t10.r2", "Mine"),
            ("spawn.cook", "Talk-to"),
            ("spawn.death", "Talk-to"),
            ("spawn.mill.hopper.3166.3307.p2.t10.r0", "Fill"),
        ] {
            let goals = source
                .target_goals(id, action, None, &map)
                .unwrap_or_else(|error| panic!("{id}: {error:#}"));
            assert!(!goals.is_empty());
            assert!(goals.iter().all(|tile| map.walkable(*tile)), "{id}");
        }
        let guide = source
            .target_goals("spawn.gielinor_guide", "Talk-to", None, &map)
            .unwrap();
        assert!(map.route(Tile::new(3094, 3106, 0), &guide).is_some());
    }
}
