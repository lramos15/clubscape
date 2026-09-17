#!/usr/bin/env python3
"""Check source reward/XP boundaries and the unchanged numeric-only native jingle transport."""

import bisect
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True


def record(path):
    data = path.read_bytes()
    return {"path": str(path), "sha256": hashlib.sha256(data).hexdigest(), "size_bytes": len(data)}


def main():
    root = Path(__file__).resolve().parents[3]
    if root != Path.cwd().resolve():
        raise ValueError("Run from the assigned worktree root.")
    tool = Path("tools/browser-audio-tests/native")
    work = tool / ".run/cook-boundary"
    for part in ["classes", "home", "work"]:
        (work / part).mkdir(parents=True, exist_ok=True)
    spec = importlib.util.spec_from_file_location("audio_policy_tools", tool / "probe.py")
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    jars = native.artifact_paths(Path("../m1-audio-bindings/.local/audio-import"))
    java = [
        "-XX:-UsePerfData", "-XX:ActiveProcessorCount=2", "-Xmx512m", "-Djava.awt.headless=true",
        f"-Duser.home={(work/'home').resolve()}", f"-Djava.io.tmpdir={(work/'work').resolve()}",
    ]
    sources = [
        Path("tools/audio-import/SourceAudio.java"), Path("tools/audio-import/BindingCache.java"),
        Path("tools/audio-import/BindingExtract.java"), Path("tools/source-capture/FixturePreferenceWrites.java"),
        *sorted(tool.glob("*.java")),
    ]
    subprocess.run(["javac", *["-J"+option for option in java], "--release", "17", "-proc:none",
                    "-cp", os.pathsep.join(map(str,jars)), "-d", str(work/"classes"), *map(str,sources)],
                   check=True, timeout=120, capture_output=True)
    output = work / "native.json"
    subprocess.run(["java", *java, "-cp", os.pathsep.join(map(str,[work/"classes",*jars])),
                    "CookRewardBoundary", "../m1-audio-bindings/.local/audio-import/inputs", str(output)],
                   check=True, timeout=60, capture_output=True)

    content_path = Path("content/m1/game-content.json.gz")
    content = json.loads(gzip.decompress(content_path.read_bytes()))
    quest_path = Path("research/journey-rules/cooks-assistant.json")
    contract = json.loads(quest_path.read_text())
    reward = next(row for row in contract["rewards"] if row["quest_ref"] == "quest.cooks_assistant")
    award = next(row["xp_tenths"] for row in reward["xp_awards"] if row["skill_ref"] == "skill.cooking")
    thresholds = content["skills"]["skill.cooking"]["xp_thresholds_tenths"]
    shrimp = content["recipes"]["recipe.cooking.shrimps.fire"]["xp"][0]["amount_tenths"]
    bread = content["recipes"]["recipe.cooking.bread.range"]["xp"][0]["amount_tenths"]
    assert (award, shrimp, bread) == (3000, 300, 400)
    boundaries = []
    for name, extra_shrimps in [("observed-level4-compatible",0),("pretrained-level5",1),("pretrained-no-level",171)]:
        before = bread + shrimp * (1 + extra_shrimps)
        after = before + award
        boundaries.append({
            "case": name, "skillId": "skill.cooking",
            "successful_source_cooking": {"bread":1,"shrimps":1+extra_shrimps},
            "before": {"xpTenths":str(before),"baseLevel":bisect.bisect_right(thresholds,before)},
            "after": {"xpTenths":str(after),"baseLevel":bisect.bisect_right(thresholds,after)},
            "levelChanged": bisect.bisect_right(thresholds,before) != bisect.bisect_right(thresholds,after),
        })
    assert [(b["before"]["baseLevel"], b["after"]["baseLevel"]) for b in boundaries] == [(1,4),(2,5),(21,21)]
    observations_path = Path("research/audio-source/selector-observations.json")
    observation = json.loads(observations_path.read_text())["cooks_assistant_observation"]
    result = {
        "schema_version": 1, "result": "passed_source_boundary_and_native_transport",
        "scope": "Source-derived committed-projection regression inputs, not a played/seeded quest or server journey.",
        "source_reward_xp_tenths_unchanged": award,
        "source_reward_atomicity": contract["reward_atomicity"],
        "boundaries": boundaries, "native_transport": json.loads(output.read_text()),
        "selector_qualification": {
            "observed_cook_jingle": observation["quest_jingle"],
            "observed_reward_level": observation["levelup_after_scroll_dismissal"]["level"],
            "observed_reward_jingle": observation["levelup_after_scroll_dismissal"]["jingle"],
            "later_level5_jingle_not_a_quest_reward": observation["later_actual_cooking_levelup"]["jingle"],
            "new_level_to_jingle_mapping": None,
            "policy": "Preserve the source-selected group in the committed event; 33/level4 is one dated case, never a universal Cook condition.",
        },
        "inputs": [record(p) for p in [content_path, quest_path, observations_path]],
        "native_artifacts": [record(p) for p in jars],
        "probe_sources": [record(p) for p in [*sources,Path(__file__)]],
        "gameplay_xp_changed": False, "quest_events_created_by_audio": False,
        "source_pack_modified": False, "m1_acceptance": False,
    }
    target = Path("research/browser-audio-policy/cook-reward-boundary.json")
    target.write_text(json.dumps(result,indent=2)+"\n")
    print(json.dumps({"source_boundaries":[[b["case"],b["before"]["baseLevel"],b["after"]["baseLevel"]] for b in boundaries],
                      "native_jingle_cases":len(result["native_transport"]["cases"]),"xp_or_quest_mutations":0}))


if __name__ == "__main__":
    main()
