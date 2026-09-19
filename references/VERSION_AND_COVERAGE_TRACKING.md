# Version and Coverage Tracking

This is the only file that should contain repeated operational pinning instructions. Other reference files describe roles and snapshot observations without restating this procedure.

## Version vocabulary

| Value | Meaning |
|---|---|
| OSRS protocol revision | Client protocol integer, such as 240 |
| Cache snapshot | Dated and hashed archive of models, configs, maps, scripts, interfaces, and related data |
| RuneLite application version | RuneLite release/build version, such as `1.12.39` |
| Injected client artifact | Underlying client code/artifact loaded by the RuneLite build |
| Repository head | Commit on a named branch at a stated time |
| Dependency target | Revision/module declared by another project's build/configuration |
| Club Penguin era | Named original client/media family and date range |
| Wand composition | Parent commit plus every submodule commit |
| Custom content build | ClubScape-generated cache/assets layered on a selected baseline |

Never use the word `latest`, a branch name, floating alpha dependency, or mutable archive URL as the only identity.

## Current research snapshot

Verified through 2026-09-17 and frozen in [FROZEN_BASELINE.md](FROZEN_BASELINE.md):

| Component | Observed value | Confidence/remaining work |
|---|---|---|
| OSRS protocol | Revision 240 | Frozen; validate the handshake against retained fixtures |
| OpenRS2 cache | Live cache/archive id 2710, build 240, timestamp 2026-09-16 10:30:13 | Frozen; record downloaded artifact hashes |
| Weekly cache mirror | `2026-09-16-rev240`; commit `ec6c640cad14b78b05b74c25e07c2603b085fd14` | Frozen; retain release-asset hashes |
| RuneLite | Stable `1.12.39`; release commit `67d51a4a4a945e6e3f60c75cbc7dd859d441ff04` | Frozen source reference; resolve exact injected client |
| RuneLite GameVals | `2026-09-16-rev240`; commit `0edc8a6bc4a0755d2c1e44de04b88adc8fea10c4` | Frozen data update |
| CS2 scripts | `2026-09-16-rev240`; commit `baf8a24853e230d024b627f2513cd475bfbd58ba` | Frozen script reference; record consumed-file hashes |
| RSProt | Revision modules through 240; alpha artifact dated 2026-09-12 | Record exact dependency and validate fixtures |
| RSProx | Protocol modules through 240 | Record exact head and capture configuration |
| OpenRune | Config 240.2 and RSProt 240 | Version-compatible candidate, not content-complete |
| Houdini | `45021fa838ede1e1526858ea079bd13d1b01d5b9` | Source implementation, not original CP truth |
| Wand | `fdeec704748ca6c75509eab3661c2c5fdb2615ad` | Resolve all five recorded submodule SHAs |
| Snowflake | `089411b895dd3cd9b19b1771dc8c046bcd7748e3` | Card-Jitsu Snow implementation claims need tests |
| Yukon | `2f47b90137d168d484ce934b58543cadf0f94920` | Pair with Yukon Server |
| Yukon Server | `fead5f7849e97a866e07d39213ff41b970497f4e` | Pair with Yukon client |
| Ruffle | `9525d0fb8773a0d1868940918385e66360927a07` | Observation runtime only |
| JPEXS | `b2ef2d3d3f44291a4bdfa899c986bc39a89fa6aa` | Inspection tool only |

The protocol revision can remain constant while cache content changes weekly. August 5, 12, 19, 26, September 2, 8, and 16 were all labeled revision 240. ClubScape is frozen to the September 16 content identity, not to revision 240 generically.

## Baseline manifest

Maintain one machine-readable record with at least:

```text
snapshot_id
verified_at_utc
protocol_revision
client_artifact + sha256
runelite_version + repository + branch + full_commit_sha
injected_client_artifact + sha256
base_cache_source + archive_or_release_id + acquisition_date + sha256
cs2_source + label + full_commit_sha + artifact_hashes
rsprot_module + exact_dependency_version + repository_sha
rsprox_repository_sha + capture_configuration
custom_cache_build_id + sha256
custom_web_asset_build_id + sha256
osrs_scope = persistent_live_game
included_persistent_account_modes[]
excluded_content_classes[] = seasonal_modes, temporary_events, removed_or_historical, inaccessible, debug, unused
access_model = no_membership_gate
risk_behavior_source = frozen_baseline
blender_version
exporter_repository + full_commit_sha
club_penguin_era
wand_parent_sha + every_submodule_sha
consumed_media_files[] + sha256
```

The manifest is the operational authority; prose tables are human-readable summaries.

## Reference manifest

For every repository actually used by an implementation or specification:

```text
repository_url
evidence_class
branch_or_tag
full_commit_sha
commit_date
stars_at_inspection
target_revision_or_era
subsystem_used
files_or_symbols_consulted[]
conclusion_ids[]
confidence_labels[]
```

Stars are retained only for research prioritization. Activity means substantive default-branch work, not GitHub `updated_at`, a dependency bump, merge commit, generated report, or submodule fast-forward.

## Source record for a conclusion

Every externally researched mechanic, formula, asset interpretation, or behavior should be traceable:

```text
conclusion_id
subject
chosen_rule_or_value
confidence_label
source_records[]
observations[]
conflicts[]
clubscape_difference
test_fixture_ids[]
last_verified_snapshot_id
```

Allowed confidence labels are defined in [README.md](README.md).

## Coverage ledgers

“Revision 240 supported” does not mean revision-240 gameplay exists. Maintain machine-readable ledgers for:

| Ledger | Minimum coverage |
|---|---|
| Items | IDs/variants, configs, actions, rules, sources, tests, penguin visual status |
| Equipment | Slot, bonuses, requirements/effects, worn models, sockets, hidden parts, animations |
| Skills | Inputs/outputs, requirements, timing, XP, boosts, interrupts, interfaces |
| Combat | Formulas, styles, weapons/ammo, spells, prayers, effects, NPC/item exceptions |
| Quests | Requirements, varps/varbits, dialogue, NPCs, objects, items, scenes, rewards, Quest Points, explicit prerequisites, and affected Quest Point consumers |
| World | Regions, collision, transport, doors, instances, spawns, shops, drops |
| Activities | Bosses, raids, minigames, diaries, clues, random, social, and group systems |
| Interfaces | IDs/scripts, state, input validation, RuneLite status, WASM status |
| Club Penguin items | Category, sources/sinks, trade/death/reclaim, economy, visual and client status |
| Club Penguin minigames | Original evidence, adaptation, state machine, OSRS links, rewards, tests |
| Penguin missions | Source era, shared quest identity, requirements, states, unified Quest Points, rewards, pins/stamps, and affected Quest Point consumers |
| Pins and player cards | Award source, binding, collection state, displayed pin/background, Collection Log links, POH display, privacy, and client status |
| Assets | Semantic IDs, evidence, Blender source, metadata, outputs, validation |

### Orthogonal status fields

Do not combine progress, verification, and fidelity into one status. Use:

```text
implementation: not_started | in_progress | implemented | blocked | deferred
verification: unverified | partially_verified | verified
fidelity: exact | presentation_adaptation | behavior_adaptation | original_content
```

Definition presence belongs in an evidence/source field and does not make `implementation` complete. A feature becomes `verified` only when semantic tests pass the written specification and required client visuals pass in the applicable RuneLite and WASM support targets.

### Item completion record

At minimum:

```text
item_id
variant_ids[]
semantic_status
name_actions_stackability
slot_requirements_bonuses
effects_and_special_attack
charges_degradation_transformations
creation_sources_shops_drops
quest_and_set_relationships
trade_ge_alchemy_death_reclaim_ironman
inventory_and_ground_model_status
penguin_worn_model_status
runelite_status
wasm_status
evidence_ids[]
test_ids[]
```

An item definition does not implement its special attack, charges, degradation, creation, drops, or quest relationships.

### Penguin equipment compatibility report

Regenerate after every cache/content update:

```text
item_id
slot
new_or_changed_since_previous_snapshot
equipment_family
explicit_exception
model_and_recolor_changes
socket_and_hidden_part_mapping
animation_coverage
runelite_capture
wasm_capture
review_status
```

## Evidence reconciliation

1. Start with the selected cache for IDs and client-visible encoded data.
2. Use OSRS DB to enumerate and enrich; retain its package/cache number and source fields.
3. Use OSRS Wiki and official updates for player-facing rules and history.
4. Compare RuneLite, RSProt/RSProx, current server projects, and permitted observation for runtime behavior.
5. Use Void, Lost City, and other historical projects for patterns only after labeling their eras.
6. Store disagreements explicitly and make the resolution testable.

## Upgrade diff

For each new OSRS baseline, compare:

- protocol packets and state updates;
- cache indices, archives, checksums, and XTEAs;
- configs, enums, structs, varps, and varbits;
- items, NPCs, objects, models, animations, textures, and maps;
- CS2 scripts, interfaces, and widget IDs;
- RuneLite APIs/events and injected-client mappings;
- OSRS DB generated records;
- official updates and relevant Wiki revisions;
- known behavior fixtures and captures;
- ClubScape custom-ID collisions;
- every new or changed wearable against the penguin equipment report.

Retain the prior baseline and its test/capture results for comparison and rollback.

## Refresh checklist

1. Inspect substantive default-branch changes and releases for primary repositories.
2. Resolve RuneLite's application build and exact injected-client artifact.
3. Check RSProt and RSProx revision modules and exact artifacts.
4. Cross-check OpenRS2 and `abextm/osrs-cache` date, revision, archive/release identity, and hashes.
5. Check OSRS DB's embedded cache number, package version, generated-data diff, and repository SHA.
6. Check CS2 label, full commit, and input artifact handling.
7. Review official updates and relevant OSRS Wiki page revisions.
8. Recheck OpenRune/RS Mod for modern compatibility and substantive content changes.
9. Recheck Void and paired Lost City engine/content branches for reusable system/tooling improvements without changing their evidence class.
10. Recheck Creator's Kit, Blender add-ons, Blender compatibility, and open format bugs.
11. Resolve Wand's parent and all submodule SHAs, including legacy and vanilla media.
12. Hash every cache, media, generated asset, and observation input actually consumed.
13. Run upgrade diffs, semantic regression tests, RuneLite captures, WASM captures, and the penguin compatibility report.
14. Publish the new snapshot manifest only after validation; retain the previous snapshot.

## Revision-monitor scope

The scheduled reference monitor should watch at least:

- RuneLite and its injected-client dependency;
- RuneLite CS2 scripts;
- RSProt and RSProx;
- OpenRS2 archives;
- `abextm/osrs-cache` releases;
- OSRS DB cache/package version;
- OpenRune and RS Mod revision targets;
- Void releases/substantive tooling changes;
- Creator's Kit and Blender add-ons;
- Wand plus Houdini, Dash, Snowflake, legacy media, and vanilla media;
- official OSRS update announcements.

Its output should report detected changes and affected subsystems rather than automatically declaring a new production baseline.
