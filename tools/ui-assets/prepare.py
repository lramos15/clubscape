#!/usr/bin/env python3
"""Prepare individual original UI assets and resolved native widget templates."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/ui-assets"
WORK = TOOL / ".cache"
OUT = ROOT / "assets/compiled/ui"
SOURCE = ROOT / "assets/source/osrs/cache2695"
PACK_SHA = "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read(path):
    raw = path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix == ".gz" else raw)


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, separators=(",", ":"), ensure_ascii=False) + "\n")


def verified(path, record):
    if sha(path) != record["sha256"] or path.stat().st_size != record["size_bytes"]:
        raise ValueError(f"Source input does not match publication: {path}")
    return path


def collections(kind):
    result = {}
    for prefix in (SOURCE, SOURCE / "content-v2", SOURCE / "potions"):
        path = prefix / f"collections/{kind}.json.gz"
        if path.exists():
            result.update({int(value["id"]): value for value in read(path).values()})
    return result


def native(items, source, tooling):
    lock = read(ROOT / "tools/cache-import/dependencies.json")
    selection = read(ROOT / "research/current-source/selection.json")
    capture_lock = read(ROOT / "tools/source-capture/dependencies.json")
    libraries = []
    for record in [lock["decoder"], *lock["libraries"], *selection["runtime"]["artifacts"],
                   *capture_lock["additional_artifacts"]]:
        path = verified(tooling / record["name"], record)
        if path not in libraries:
            libraries.append(path)
    for record in read(ROOT / "research/current-source/cache-files.json"):
        verified(source / record["name"], record)
    for name in ("classes", "java-home", "java-work", "native"):
        (WORK / name).mkdir(parents=True, exist_ok=True)
    requests = []
    for item in items.values():
        quantities = {1}
        quantities.update(q for q in item.get("countCo") or [] if q > 1)
        for quantity in sorted(quantities):
            requests.append([item["id"], quantity])
    write(WORK / "items.json", requests)
    original = ROOT / "tools/source-capture"
    instrumented = (original / "HudCapture.java").read_text()
    begin = instrumented.index('state.add(OriginalCapture.map("id", widget.getId()')
    end = instrumented.index("));", begin) + 3
    instrumented = instrumented[:begin] + "state.add(UiAssetExport.widget(widget));" + instrumented[end:]
    instrumented = instrumented.replace(
        "!widget.isHidden() && widget.getWidth() > 0 && widget.getHeight() > 0",
        "!widget.isHidden() && ((widget.getWidth() > 0 && widget.getHeight() > 0) || widget.getType() == 4 || widget.getType() == 9)")
    needle = '), 10000);\n    }\n\n    private void bank()'
    replacement = '), 10000);\n        UiAssetExport.frame(capture, this, name, state);\n    }\n\n    private void bank()'
    if instrumented.count(needle) != 1:
        raise ValueError("Pinned native frame instrumentation anchor changed")
    instrumented = instrumented.replace(needle, replacement)
    instrumented = instrumented.replace(
        "        qi.ck.az(1920, 1080, widgets, 1, 2, -293044276);\n        List<Object> state",
        "        if (activeGroup == 679 || activeGroup == 84 || activeGroup == 4)\n"
        "            UiAssetExport.rendererPreviewBoundary(capture, this, activeGroup);\n"
        "        qi.ck.az(1920, 1080, widgets, 1, 2, -293044276);\n        List<Object> state")
    instrumented = instrumented.replace("        unlockFamilies();",
        "        unlockFamilies();\n        extraUiPanels();\n        UiAssetExport.extras(capture);")
    extra = """
    private void extraUiPanels() throws Exception
    {
        for (var node : new ArrayList<>(sideNodes.values())) capture.game.closeInterface(node, true);
        sideNodes.clear();
        enabledTabs.clear();
        for (int tab = 0; tab < 14; tab++)
        {
            sideNodes.put(tab, capture.game.openInterface(161 << 16 | (76 + tab), TAB_GROUPS[tab], 1));
            enabledTabs.add(tab);
        }
        fixtureFamily = "source-additional-component";
        String[] names = {"grouping", "friends", "account", "logout", "settings", "emotes", "music"};
        for (int tab = 7; tab < 14; tab++)
        {
            selectTab(tab);
            frame("native-" + names[tab - 7], TAB_GROUPS[tab]);
        }
        selectTab(3);
        int[] groups = {119, 153, 929, 679, 84, 4, 602, 669, 312, 270, 134};
        String[] panels = {"journal", "reward", "experience", "appearance", "equipment-stats",
            "kept-items", "grave", "recovery", "smithing", "production", "all-settings"};
        for (int i = 0; i < groups.length; i++)
        {
            var main = capture.game.openInterface(161 << 16 | 16, groups[i], 0);
            capture.game.runScript(907, 161 << 16, 1130);
            UiAssetExport.rendererPreviewBoundary(capture, this, groups[i]);
            frame("native-" + panels[i], groups[i]);
            capture.game.closeInterface(main, true);
            capture.game.runScript(907, 161 << 16, 1130);
        }
        capture.game.setVarbit(net.runelite.api.Prayer.THICK_SKIN.getVarbit(), 1);
        load(541, true);
        selectTab(5);
        frame("native-prayer-active", 541);
        capture.game.setVarbit(net.runelite.api.Prayer.THICK_SKIN.getVarbit(), 0);
        load(541, true);
        bh.aq(93, 9, -1, 0);
        bh.aq(93, 10, -1, 0);
        load(218, true);
        selectTab(6);
        frame("native-magic-missing-runes", 218);
    }
"""
    instrumented = instrumented.rstrip()[:-1] + extra + "}\n"
    (WORK / "HudCapture.java").write_text(instrumented)
    cp = os.pathsep.join(map(str, libraries))
    env = dict(os.environ, TMPDIR=str(WORK / "java-work"), TMP=str(WORK / "java-work"),
               TEMP=str(WORK / "java-work"))
    java_sources = [p for p in original.glob("*.java") if p.name != "HudCapture.java"]
    command = ["javac", "--release", "17", "-cp", cp, "-d", str(WORK / "classes"),
               *map(str, java_sources), str(WORK / "HudCapture.java"), str(TOOL / "UiAssetExport.java")]
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    (WORK / "compile.log").write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-6000:])
    command = ["java", "-ea", "-Xmx3g", "-Djava.awt.headless=true",
               "--add-opens=java.base/java.lang=ALL-UNNAMED", "-Dclubscape.capture.seed=0",
               f"-Duser.home={WORK / 'java-home'}", f"-Djava.io.tmpdir={WORK / 'java-work'}",
               f"-Dclubscape.ui.items={WORK / 'items.json'}",
               "-cp", str(WORK / "classes") + os.pathsep + cp,
               "OriginalCapture", str(source), str(WORK), str(WORK / "native"), "hud"]
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=600)
    (WORK / "native.log").write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-6000:])
    for record in read(ROOT / "research/current-source/cache-files.json"):
        verified(source / record["name"], record)
    return [{"path": str(p.relative_to(ROOT.parent)), "sha256": sha(p)} for p in libraries]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", action="store_true", help="Re-run pinned original runtime, no accounts or network")
    parser.add_argument("--source", type=Path, default=ROOT.parent / "m1-runtime-inputs/.local/current-source/cache-2695")
    parser.add_argument("--tooling", type=Path, default=ROOT.parent / "m1-source-captures/.local/source-capture/tooling")
    args = parser.parse_args()
    pack_path = ROOT / "research/reference-pack/v1/manifest.json"
    approval = read(ROOT / "milestones/approvals/m1-reference-pack-v1.3.0.json")
    if sha(pack_path) != PACK_SHA or approval["decision"] != "approved" or approval["reference_pack_sha256"] != PACK_SHA:
        raise ValueError("Exact owner-approved source pack is required")
    pack = read(pack_path)
    items = collections("item")
    for capture in read(ROOT / "assets/reference/osrs240/native-hud/captures.json")["captures"]:
        for widget in capture["source"]["visible_widgets"]:
            if widget["item"] >= 0 and widget["item"] not in items:
                items[widget["item"]] = {"id": widget["item"], "name": "", "examine": "",
                    "stackable": 1 if widget["item_quantity"] > 1 else 0, "interfaceOptions": [], "countCo": None}
    WORK.mkdir(parents=True, exist_ok=True)
    artifacts = native(items, args.source.resolve(), args.tooling.resolve()) if args.native else (
        read(OUT / "provenance.json").get("originalRuntimeArtifacts", []) if (OUT / "provenance.json").exists() else [])
    native_dir = WORK / "native"
    if not (native_dir / "captures.json").exists():
        raise ValueError("Run prepare.py --native once; resolved native styles cannot be guessed")
    # Compare the instrumented host to the already-approved originals, not to a new baseline.
    native_source = ROOT / "assets/reference/osrs240/native-hud"
    matches = []
    for capture in read(native_source / "captures.json")["captures"]:
        path = native_dir / capture["path"]
        if sha(path) != capture["sha256"]:
            raise ValueError(f"Readback instrumentation changed approved source pixels: {capture['path']}")
        matches.append(capture["path"])
    OUT.mkdir(parents=True, exist_ok=True)
    for name in ("sprites", "fonts", "items", "portraits", "minimaps"):
        destination = OUT / name
        destination.mkdir(exist_ok=True)
        for path in (native_dir / name).glob("*"):
            if path.suffix == ".png":
                shutil.copyfile(path, destination / path.name)
    shutil.copyfile(native_dir / "title-background.png", OUT / "title-background.png")
    sprites = {p.stem: read(p) | {"asset": f"ui/sprites/{p.stem}.png"} for p in (native_dir / "sprites").glob("*.json")}
    fonts = {p.stem: read(p) for p in (native_dir / "fonts").glob("*.json")}
    definitions = {}
    for prefix in (SOURCE, SOURCE / "content-v2"):
        for p in (prefix / "interfaces").glob("*.json.gz"):
            definitions.update({str(w["id"]): w for w in read(p)})
    scenes = {p.stem: read(p) for p in (native_dir / "styles").glob("*.json")}
    for widgets in scenes.values():
        for widget in widgets:
            definition = definitions.get(str(widget["id"]), {})
            widget["lineWidth"] = definition.get("lineWidth", 1)
            widget["lineDirection"] = definition.get("lineDirection", False)
    item_catalog = {}
    native_items = read(native_dir / "item-metadata.json")
    for item in items.values():
        item_catalog[str(item["id"])] = {
            key: item.get(key) for key in ("name", "examine", "stackable", "interfaceOptions", "shiftClickDropIndex",
                                         "notedID", "notedTemplate", "countCo", "countObj", "category")
        }
        quantities = sorted({1, *(q for q in item.get("countCo") or [] if q > 1)})
        if not item_catalog[str(item["id"])]["name"]:
            item_catalog[str(item["id"])].update(native_items[str(item["id"])])
        item_catalog[str(item["id"])]["icons"] = [
            {"minimum": q, "asset": f"ui/items/{item['id']}-{q}-1.png",
             "selectedAsset": f"ui/items/{item['id']}-{q}-2.png"} for q in quantities]
    factoring = pack["evidence_factorization"]
    content = read(ROOT / "content/m1/game-content.json.gz")
    manifest = {
        "version": 1, "sourcePackSha256": PACK_SHA, "sourceCache": 2695, "nativeCanvas": [1920, 1080],
        "sprites": sprites, "fonts": fonts, "items": item_catalog,
        "titleBackground": "ui/title-background.png", "templates": scenes,
        "namedSprites": read(native_dir / "named-sprites.json"),
        "minimaps": read(native_dir / "minimaps.json"),
        "combatCategories": read(native_dir / "combat-categories.json"),
        "questTable": read(native_dir / "hud-input-contract.json")["quest_counter_fixture"],
        "definitions": definitions,
        "proposals": {p["id"].removeprefix("proposal."): {
            "content": p["content"], "frame": p["titlebox_rectangle"], "controls": p["controls"]
        } for p in pack["proposal_inputs"]},
        "tutorialStates": factoring["state_bindings"], "hudSignatures": factoring["hud_signatures"],
        "portraits": read(native_dir / "portraits.json"),
        "npcs": {str(n["id"]): {"name": n["name"], "examine": n.get("examine")} for n in collections("npc").values()},
        "presentation": {
            "interfaces": {key: {"name": v["name"], "sourceIds": v["source_ids"]}
                           for key, v in content["interfaces"].items()},
            "skills": {key: {"name": v["name"], "sourceId": v["source_id"], "thresholds": v["xp_thresholds_tenths"]}
                       for key, v in content["skills"].items()},
            "styleIds": list(content["mechanics"]["combat_styles"]),
            "experiences": [{"id": v["id"], "name": v["name"]} for v in content["mechanics"]["experiences"].values()],
            "appearance": content["mechanics"]["appearance"]["choices"],
            "sourceItems": {key: v["source_id"] for key, v in content["items"].items()},
            "weapons": {str(v["source_id"]): v["equipment"]["weapon"]["styles"]
                        for v in content["items"].values() if v.get("equipment") and v["equipment"].get("weapon")},
            "weaponCategories": {"1265": 10, "1351": 1, "1205": 17, "1277": 17, "841": 3, "1237": 14},
            "equipment": {str(v["source_id"]): v["equipment"]["bonuses"]
                          for v in content["items"].values() if v.get("equipment")},
            "runEnergyScale": 100,
            "runEnergyUnitBasis": "Server/content centipercent representation; normal full energy is initial_state.run_energy=10000.",
        },
    }
    write(OUT / "manifest.json", manifest)
    write(OUT / "provenance.json", {
        "sourcePackSha256": PACK_SHA, "originalRuntimeArtifacts": artifacts,
        "readbackInstrumentationSource": {"path": "tools/source-capture/HudCapture.java",
                                          "sha256": sha(ROOT / "tools/source-capture/HudCapture.java")},
        "nativeFixturesUnchanged": matches,
        "transformations": [
            "Original cache sprite frames repacked without scaling, palette changes or invented pixels.",
            "Original CP1252 257-byte font metrics and original glyph masks.",
            "Original Client.createItemSprite, native widget shadow 0x333333, quantity_mode=0; quantities are painted from AppState.",
            "Original JPEG decoder, unscaled source title background, no title-screen capture.",
            "NPC model-only native widget painting on transparency; no finished dialogue-panel crop.",
            "Native widget readbacks; fixture text/items are templates, never authoritative game state."
        ],
        "additionalPreviewBoundary": read(native_dir / "renderer-previews.json"),
        "assets": [{"path": "ui/" + str(p.relative_to(OUT)), "sha256": sha(p), "bytes": p.stat().st_size}
                   for p in sorted(OUT.rglob("*")) if p.is_file() and p.name != "provenance.json"],
        "finalAcceptance": False, "gameplayEvidence": False,
    })
    print(json.dumps({"sprites": len(sprites), "fonts": len(fonts), "items": len(items),
                      "nativeTemplates": len(scenes), "sourceFixtureReadbacksExact": len(matches)}))


if __name__ == "__main__":
    main()
