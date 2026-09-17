#!/usr/bin/env python3
"""Reproduce pinned public originals/revisions, never silently follow a new baseline."""

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import sys
import time

from components import ROOT, OUT
from fetch import get, image_facts, query, write_json
from media import mp4_facts


def retrieve_blob(entry, output):
    expected_sha1 = entry["imageinfo"]["sha1"]
    expected_sha256 = entry["sha256"]
    destination = output / entry["path"]
    if destination.exists():
        data = destination.read_bytes()
        if hashlib.sha256(data).hexdigest() != expected_sha256:
            raise ValueError(f"Changed local retrieval; refusing to overwrite: {destination}")
        return {"id": entry["id"], "sha256": expected_sha256, "result": "existing_exact_bytes"}
    info = query({
        "titles": entry["title"], "prop": "imageinfo",
        "iiprop": "timestamp|url|size|sha1|mime",
        "iistart": entry["upload_timestamp"], "iiend": entry["upload_timestamp"], "iilimit": 1,
    })
    pages = info["response"].get("query", {}).get("pages", [])
    matching = [image for page in pages for image in page.get("imageinfo", [])
                if image["sha1"] == expected_sha1 and image["timestamp"] == entry["upload_timestamp"]]
    if len(matching) != 1:
        raise ValueError(f"Exact pinned upload is not publicly retrievable: {entry['title']}")
    original_url = matching[0]["url"]
    attempts = []
    for attempt in range(3):
        url = original_url + ("&" if "?" in original_url else "?")
        url += f"download=1&reference_original={expected_sha1}-{time.time_ns()}"
        data = get(url, entry["size_bytes"] + 1)
        actual_sha1 = hashlib.sha1(data).hexdigest()
        actual_sha256 = hashlib.sha256(data).hexdigest()
        attempts.append({"url": url, "sha1": actual_sha1, "sha256": actual_sha256, "size_bytes": len(data)})
        if actual_sha256 == expected_sha256 and actual_sha1 == expected_sha1 and len(data) == entry["size_bytes"]:
            break
        print(f"Rejected changed/CDN-transformed bytes: {entry['title']} attempt{attempt + 1}", file=sys.stderr)
    else:
        raise ValueError(f"No exact original bytes after three bounded deliveries: {entry['title']}")
    decoded = mp4_facts(data) if entry["decoded"]["format"] == "MP4" else image_facts(data)
    if decoded != entry["decoded"]:
        raise ValueError(f"Decoded original differs: {entry['title']}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(data)
    return {"id": entry["id"], "result": "retrieved_exact_original", "sha256": expected_sha256,
            "original_or_archived_url": original_url, "attempts": attempts}


def retrieve_pages(pages, output):
    results = []
    for offset in range(0, len(pages), 20):
        selected = pages[offset:offset + 20]
        record = query({"revids": "|".join(str(page["revision"]) for page in selected),
                        "prop": "revisions", "rvprop": "ids|timestamp|content", "rvslots": "main"})
        if "continue" in record["response"]:
            raise ValueError("Incomplete pinned page response")
        by_id = {revision["revid"]: revision
                 for page in record["response"]["query"]["pages"] for revision in page.get("revisions", [])}
        for page in selected:
            snapshot = json.loads(gzip.decompress((ROOT / page["snapshot"]["path"]).read_bytes()))
            expected = snapshot["page"]["revisions"][0]["slots"]["main"]["content"].encode("utf-8")
            revision = by_id[page["revision"]]
            actual = revision["slots"]["main"]["content"].encode("utf-8")
            if actual != expected or revision["timestamp"] != page["revision_timestamp"]:
                raise ValueError(f"Wrong pinned source prose/revision: {page['title']}")
            destination = output / "pages" / f"{page['page_id']}-{page['revision']}.wikitext"
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(actual)
            results.append({"title": page["title"], "revision": page["revision"],
                            "sha256_utf8_wikitext": hashlib.sha256(actual).hexdigest(),
                            "result": "exact_pinned_revision"})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    output = (ROOT / args.output).resolve()
    allowed = (ROOT / ".local/reference-pack").resolve()
    if output == allowed or not output.is_relative_to(allowed):
        raise ValueError("Retrieval output must be a named child of this worktree's .local/reference-pack/")
    if args.limit is not None and args.limit <= 0:
        raise ValueError("--limit must be positive")
    media = json.loads((OUT / "public-media.json").read_text())
    pages = json.loads((OUT / "pages.json").read_text())
    if args.limit:
        media, pages = media[:args.limit], pages[:args.limit]
    blobs = [retrieve_blob(entry, output) for entry in media]
    revisions = retrieve_pages(pages, output)
    report = {"schema_version": 1, "observed_at": datetime.now(timezone.utc).isoformat(),
              "media": blobs, "pages": revisions, "source_identity_changed": False,
              "account_login": False, "owner_approval": False}
    write_json(output / "retrieval-report.json", report)
    print(json.dumps({"exact_originals": len(blobs), "exact_page_revisions": len(revisions),
                      "report": (output / "retrieval-report.json").relative_to(ROOT).as_posix()}))


if __name__ == "__main__":
    main()
