#!/usr/bin/env python3
"""Close missing starter item IDs using existing original bytes, without fetching a cache."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import zlib


ROOT = Path(__file__).resolve().parents[2]
WORK = ROOT / "tools/m1-content/.local/definition-import"
OUTPUT = ROOT / "research/m1-bindings/definitions.json.gz"
NAMES = (
    "Burnt shrimp", "Ashes", "Bucket of water", "Bread dough", "Burnt bread", "Hammer",
    "Wooden shield", "Water rune", "Earth rune", "Body rune", "Bones",
    "Bottomless milk bucket", "Bottomless milk bucket (empty)", "Bronze sq shield", "Bronze spear", "Bronze bolts",
    "Goblin book", "Goblin mail", "Chef's hat", "Beer", "Brass necklace", "Air talisman",
    "Energy potion(1)", "Energy potion(2)", "Energy potion(4)", "Ensouled goblin head",
    "Goblin champion scroll", "Jug", "Empty jug pack", "Shears", "Knife",
    "Empty bucket pack", "Bowl", "Cake tin", "Chisel", "Spade", "Newcomer map", "Security book",
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def disk_archive(directory, index, group):
    """Read checked JS5 sectors in rb mode; no cache storage implementation can mutate them."""
    with (directory / f"main_file_cache.idx{index}").open("rb") as source:
        source.seek(group * 6)
        entry = source.read(6)
    if len(entry) != 6:
        raise ValueError(f"Missing index entry {index}/{group}")
    length = int.from_bytes(entry[:3], "big")
    sector = int.from_bytes(entry[3:], "big")
    if not 0 < length <= 16 * 1024 * 1024:
        raise ValueError(f"Invalid archive length {length}")
    data, visited, chunk = bytearray(), set(), 0
    with (directory / "main_file_cache.dat2").open("rb") as source:
        while len(data) < length:
            if not sector or sector in visited:
                raise ValueError("Truncated/cyclic cache sector chain")
            visited.add(sector)
            source.seek(sector * 520)
            header_size = 8 if group <= 65535 else 10
            header = source.read(header_size)
            if len(header) != header_size:
                raise ValueError("Truncated cache sector header")
            width = 2 if group <= 65535 else 4
            actual_group = int.from_bytes(header[:width], "big")
            actual_chunk = int.from_bytes(header[width:width + 2], "big")
            following = int.from_bytes(header[width + 2:width + 5], "big")
            if (actual_group, actual_chunk, header[-1]) != (group, chunk, index):
                raise ValueError("Wrong source archive/chunk/index in cache sector")
            count = min(520 - header_size, length - len(data))
            part = source.read(count)
            if len(part) != count:
                raise ValueError("Truncated cache sector data")
            data.extend(part)
            sector, chunk = following, chunk + 1
    if sector:
        raise ValueError("Unexpected trailing cache sector")
    return bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-dir", type=Path, required=True)
    parser.add_argument("--tooling-dir", type=Path, required=True)
    parser.add_argument("--npc-catalogue", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    args = parser.parse_args()
    WORK.mkdir(parents=True, exist_ok=True)
    lock = json.loads((ROOT / "tools/cache-import/dependencies.json").read_text())
    jars = []
    for dependency in [lock["decoder"], *lock["libraries"]]:
        path = args.tooling_dir / dependency["name"]
        if sha(path.read_bytes()) != dependency["sha256"]:
            raise ValueError(f"Decoder dependency SHA mismatch: {path.name}")
        jars.append(str(path.resolve()))
    bundle = json.loads(gzip.decompress(
        (ROOT / "assets/manifests/osrs/cache2695-full-bundle.json.gz").read_bytes()))
    container = disk_archive(args.cache_dir, 2, 10)
    expected = bundle["groups"]["2/10"]
    if sha(container) != expected["container_sha256"]:
        raise ValueError("Original item archive differs from selected full-bundle identity")
    if (int.from_bytes(container[-2:], "big") != expected["revision"] & 65535
            or zlib.crc32(container[:-2]) != expected["crc32"]):
        raise ValueError("Item archive revision trailer/CRC mismatch")
    index = disk_archive(args.cache_dir, 255, 2)
    (WORK / "index2.bin").write_bytes(index)
    (WORK / "items.bin").write_bytes(container)
    (WORK / "request.json").write_text(json.dumps({"names": NAMES, "ids": [2530, 33089, 33091]}))
    classpath = ":".join(jars)
    javac = str(args.java_home / "bin/javac") if args.java_home else "javac"
    java = str(args.java_home / "bin/java") if args.java_home else "java"
    options = [f"-Djava.io.tmpdir={WORK}", f"-Duser.home={WORK}", "-Djava.awt.headless=true"]
    subprocess.run([javac, *["-J" + value for value in options], "--release", "17",
                    "-classpath", classpath, "-d", str(WORK),
                    str(ROOT / "tools/m1-content/DefinitionSupplement.java")], check=True)
    subprocess.run([java, *options, "-ea", "-classpath", str(WORK) + ":" + classpath,
                    "DefinitionSupplement", str(WORK)], check=True)
    decoded = json.loads((WORK / "decoded.json").read_text())
    if decoded["archive_revision"] != expected["revision"]:
        raise ValueError("Config index disagrees with pinned archive revision")
    catalogue_bytes = args.npc_catalogue.read_bytes()
    catalogue = json.loads(catalogue_bytes)
    npc_ids = (306, 311, 7941, 9244, 4626, 4628)
    output = {
        "schema_version": 1,
        "source_selection": "research/current-source/selection.json",
        "purpose": "Additional original definitions absent from the source worker's bounded item request; "
                   "no new meshes, substitute assets, NPC spawn observations or cache download.",
        "item_group": expected,
        "index2_container_sha256": sha(index),
        "decoder_lock": "tools/cache-import/dependencies.json",
        "items": decoded["items"],
        "npcs": {str(number): catalogue[str(number)] for number in npc_ids},
        "npc_source": {
            "origin": "existing source-worker dependency-scan/npc-catalogue.json",
            "sha256": sha(catalogue_bytes),
            "group": bundle["groups"]["2/9"],
        },
    }
    data = (json.dumps(output, sort_keys=True, separators=(",", ":")) + "\n").encode()
    OUTPUT.write_bytes(gzip.compress(data, mtime=0))
    print(json.dumps({"items": len(output["items"]), "npcs": len(output["npcs"]),
                      "output": str(OUTPUT.relative_to(ROOT)), "sha256": sha(OUTPUT.read_bytes())}))


if __name__ == "__main__":
    main()
