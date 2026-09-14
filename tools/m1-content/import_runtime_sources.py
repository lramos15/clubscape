#!/usr/bin/env python3
"""Replay only the pinned source pages needed to close concrete v2 runtime bindings."""

import json
import re
import urllib.parse
import urllib.request

from common import BINDINGS, Inputs, load, sha, write


PAGES = ("Fire", "Shop", "Bottomless milk bucket", "General store")


def main():
    inputs = Inputs()
    directory = BINDINGS / ".local/runtime-sources"
    directory.mkdir(parents=True, exist_ok=True)
    previous = BINDINGS / "runtime-source-facts.json"
    known = dict(inputs.by_page)
    if previous.exists():
        known.update({entry["source"]["page"]: entry["source"] for entry in load(previous)["sources"]})
    if "General store" not in known:
        url = "https://oldschool.runescape.wiki/api.php?" + urllib.parse.urlencode({
            "action": "query", "format": "json", "formatversion": 2, "prop": "revisions",
            "rvprop": "ids|timestamp|content", "rvslots": "main", "titles": "General store",
        })
        with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "ClubScapeM1Content/2"}), timeout=90) as response:
            payload = json.load(response)
        if "error" in payload or "continue" in payload:
            raise ValueError("Incomplete general-store source response")
        revision = payload["query"]["pages"][0]["revisions"][0]
        data = revision["slots"]["main"]["content"].encode()
        known["General store"] = {
            "id": "source.wiki.runtime.general_store", "kind": "wiki_revision", "page": "General store",
            "revision": revision["revid"], "revision_timestamp": revision["timestamp"],
            "url": f"https://oldschool.runescape.wiki/w/General_store?oldid={revision['revid']}",
            "sha256_utf8_wikitext": sha(data), "bytes_utf8_wikitext": len(data),
        }
        (directory / f"{revision['revid']}.wikitext").write_bytes(data)
    records = [known[name] for name in PAGES]
    missing = [record for record in records if not (directory / f"{record['revision']}.wikitext").exists()]
    if missing:
        url = "https://oldschool.runescape.wiki/api.php?" + urllib.parse.urlencode({
            "action": "query", "format": "json", "formatversion": 2, "prop": "revisions",
            "rvprop": "ids|timestamp|content", "rvslots": "main",
            "revids": "|".join(str(record["revision"]) for record in missing),
        })
        request = urllib.request.Request(url, headers={"User-Agent": "ClubScapeM1Content/2 (pinned source binding)"})
        with urllib.request.urlopen(request, timeout=90) as response:
            payload = json.load(response)
        if "error" in payload or "continue" in payload:
            raise ValueError("Incomplete pinned runtime source response")
        for page in payload["query"]["pages"]:
            revision = page["revisions"][0]
            (directory / f"{revision['revid']}.wikitext").write_text(revision["slots"]["main"]["content"])
    facts = []
    for record in records:
        data = (directory / f"{record['revision']}.wikitext").read_bytes()
        if sha(data) != record["sha256_utf8_wikitext"]:
            raise ValueError(f"Source hash mismatch: {record['page']}")
        text = data.decode()
        fields = {key.strip(): value.strip() for key, value in re.findall(
            r"^\|\s*(id\d*|name\d*|stackable|value|weight)\s*=\s*([^\n]+)", text, re.M)}
        facts.append({"source": record, "infobox_fields": fields})
        excerpts = [line for line in text.splitlines() if re.search(
            r"\b40\b|restock|10,?000|[Ss]tack|^\|\s*id\d*\s*=|2732|26185", line)]
        print(record["page"], json.dumps(fields))
        print("\n".join(excerpts[:12])[:3500])
    write(BINDINGS / "runtime-source-facts.json", {"sources": facts}, pretty=True)


if __name__ == "__main__":
    main()
