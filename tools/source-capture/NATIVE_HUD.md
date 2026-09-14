# Native Resizable-Classic HUD references

The `hud` profile drives the original widget repository, layout, CS2 interpreter,
widget-update traversal, scene/minimap code and widget painter. It does **not**
draw replacement panels, paste reference screenshots, or invoke login/account
handlers. Source timing/gameplay observations and owner approval remain separate.

```bash
python3 tools/source-capture/capture.py --profile hud
python3 tools/source-capture/capture.py --profile hud --verify-only
python3 -m unittest discover -s tools/source-capture -p 'test_*.py' -q
```

HUD output defaults to `assets/reference/osrs240/native-hud/`; the existing
93-image `all` profile and its default directory are preserved. The HUD directory
contains its own `captures.json`, `provenance.json`, source input contract and
16 full-frame PNGs. Original source/runtime inputs are reused under the same
hash-checked, isolated worktree rules documented in `README.md`.

## Actual native paths

* `vv/ly` loads original interface groups from index 3, models 7, sprites 8 and
  font metrics 13. The optional fifth widget archive is index 23, which has zero
  source groups in this cache; its native optional reference remains null.
* Root group **161** is `TOPLEVEL_OSRS_STRETCH`, the stock Resizable-Classic frame.
  `cn.ae` performs original recursive sizing. Root on-load **901** and redraw
  **907** use source enum **1130** and the original CS2 VM, not host layout math.
* `Client.openInterface`/`closeInterface` create native interface-parent links.
  Side-tab selections execute the actual dynamically installed source listener
  **914**, which calls source tab-selection logic.
* `pq.as` performs the real widget-update traversal, including publication of
  absolute render coordinates; `gp.az` paints the complete native tree. Merely
  calling the painter leaves some API canvas positions unset, even when pixels
  look right. The fixture now uses both parts of the native lifecycle.
* Scene content and minimap are original rendering, not image backgrounds.
  Native world/player registries, default identity kits, the original minimap
  scratch setup and `client.bm` map rasterizer feed the actual widget handlers.
* Native source configurations supply enum, inventory, parameter, struct, varbit,
  varc and DB-row/table/index definitions. The DB and font bindings are required
  by current combat, quest, prayer and magic scripts.
* Bank groups **12/15** run their real on-load scripts. Shop group **300** is
  initialized with native script **1074**. Its original side group **301** uses
  the shared native inventory initializer **6007** for the supplied items; this
  is visual source-component reuse, **not verification of sale actions/prices**.
* NPC dialogue uses the real chat-modal anchor **162:567**, interface **231** and
  source chatbox rebuild **216**. An invisible attachment to a different chat
  layer was rejected rather than accepted as a dialogue reference.

The source JAR is unchanged. Version-specific fields are accessed by exact
name/type where obfuscation permits otherwise-ambiguous Java members. The original
unneeded enclosing-class metadata on `yo` is retained; reflection binds its font
archive without rewriting the JAR.

The matching RuneLite resource JAR remains on the classpath. Unlike the earlier
model/title fixtures, HUD scripts can use its stock compatibility overlays.
No plugin, custom panel, HD renderer or host-drawn replacement artwork is enabled.
Required source script hashes and overlay dispositions are retained in
`hud-input-contract.json`; this reference is the pinned **injected runtime**,
not an unsupported assertion about a pristine standalone Jagex gamepack.

## Captured families

| Files under `hud/` | Native content |
| --- | --- |
| `native-inventory`, `native-equipment` | Populated source inventory/worn-item widgets |
| `native-skills`, `native-combat` | Native levels, source weapon name and matching combat category |
| `native-prayer`, `native-magic` | Source sprites, text metrics, locks and spell/prayer layout |
| `native-quest-list` | Actual quest DB rows and native source-derived fixture counters |
| `native-bank`, `native-shop` | Native main/side modal families with explicit fixture contents |
| `native-guide-dialogue` | Original NPC head/name/dialogue/continue layout |
| `family-guide`, `family-survival`, `family-quest-guide` | Increasing native tab-attachment families with instructor dialogue |
| `family-combat`, `family-prayer`, `family-magic` | Additional source tab families and native instructor portraits |

All are 1920×1080 full-frame original renders with the stock map, chatbox, sidebar
and source panel geometry. The all-unlocked set has all **14** original side-tab
controls. Each progression-family capture records the actual native
interface-parent links and enabled slot set; it does not merely hide a large
rectangle in an image.

These families are **declared offline source UI scenarios**, organized by
representative instructor roles. They are not authenticated progression,
an exact 71-stage unlock table, or verified arrival-account observations.
The same explicitly chosen Lumbridge scene/camera is used to isolate UI changes;
the existing original Tutorial Island captures remain separately available.
Dialogue body text is deliberately synthetic fixture text and is not attributed
to the source transcript.

## Controlled state and isolation

The fixture registers an original default-kit player named `Reference`, source
combat level 3, all skills 1 except Hitpoints 10 (1154 XP), declared inventory and
equipment containers, empty social data and explicit source varps/varcs.
These are inputs to rendering, not rewards or claims about a fresh account.
The sword fixture uses source combat category **17**, identified in native
DB row **3959**, so it shows **Stab/Lunge/Slash/Block**, not an inconsistent
unarmed panel labeled as a sword.

Quest counter inputs are derived from source quest table 0: type-0 parent quests
count once (187), while their subquest points remain in the total (347).
Completed/earned values are zero. This is a documented data-derived fixture,
not an independently observed live server counter. Source table field names and
the derivation are recorded beside the capture.

`FixtureVarcs` retains the original schema/defaults and VM operations but disables
only persisted UI-settings reads. `FixturePreferenceWrites` records only the
identified `mw.af` preference-save tasks instead of executing them; unexpected
async operations fail. No personal settings are read or overwritten.

Original UI scripts can construct chat-filter requests. A zero-key **local fixture
ISAAC** enables that native construction, but the packet writer has **no transport**.
The fixture never flushes it, pumps a network connection or calls login handlers.
Transport absence is checked and recorded for every image. This is not a login
or encryption bypass and is not evidence of request delivery.

## Checks and remaining scope

In addition to the original cache/JAR/PNG checks, validation requires:

* Native 161 root, chat/orbs attachments, exact tab-link families and a disconnected
  packet writer.
* Original widget-update canvas positions. At 1920×1080 the source map container
  is `[1709,0,211,207]`, chat `[0,915,519,165]`, sidebar `[1679,745,241,335]` and
  standard side content `[1704,782,190,261]`.
* Per-region pixel hashes, colors and nonblank pixels, so a nonblank world cannot
  conceal a missing panel.
* Actual dynamic item widgets, skill text, consistent native combat labels,
  source quest rows/counters, bank/shop initialization and instructor models.
* No native exceptions or handled CS2 `Client error` reports.

Source rendering is no longer blocked on obtaining a live account. The owner has
directed wiki/online references and permits terms acceptance; no such acceptance
was needed or performed here. Timing, exact server-side unlock mappings,
stock/repricing/sale responses and live state observations remain separate.
Numeric comparison tolerances, owner reference-pack approval and final M1
presentation/gameplay acceptance are **not** granted by this experiment.
