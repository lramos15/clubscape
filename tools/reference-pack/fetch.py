#!/usr/bin/env python3
"""Bounded, account-free acquisition of exact public MediaWiki evidence."""

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

from PIL import Image
from media import mp4_facts


ROOT = Path(__file__).resolve().parents[2]
SCRATCH = ROOT / ".local/reference-pack"
EVIDENCE = ROOT / "research/reference-pack/v1"
MEDIA = ROOT / "assets/reference/wiki"
API = "https://oldschool.runescape.wiki/api.php"
AGENT = "ClubScape-ReferencePack/1.0 (bounded public source research; no account)"
MAX_RESPONSE = 24 * 1024 * 1024


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + "\n")


def get(url, limit=MAX_RESPONSE):
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https" or parsed.hostname != "oldschool.runescape.wiki":
        raise ValueError(f"Unexpected acquisition host: {url}")
    for attempt in range(3):
        try:
            with urllib.request.urlopen(
                urllib.request.Request(url, headers={"User-Agent": AGENT}),
                timeout=90,
            ) as response:
                final = urllib.parse.urlparse(response.url)
                if final.scheme != "https" or final.hostname != parsed.hostname:
                    raise ValueError(f"Unexpected redirect: {response.url}")
                data = response.read(limit + 1)
                if len(data) > limit:
                    raise ValueError(f"Response exceeds {limit} bytes: {url}")
                return data
        except urllib.error.HTTPError as error:
            if error.code not in (429, 500, 502, 503, 504) or attempt == 2:
                raise
            print(f"Retrying HTTP {error.code}: {url}", flush=True)
            time.sleep(2 ** (attempt + 1))
    raise RuntimeError("HTTP retries exhausted")


def query(params):
    params = {"action": "query", "format": "json", "formatversion": 2, **params}
    key = sha256(json.dumps(params, sort_keys=True).encode())
    path = SCRATCH / "api" / (key + ".json")
    if path.exists():
        record = json.loads(path.read_text())
    else:
        url = API + "?" + urllib.parse.urlencode(params)
        data = json.loads(get(url))
        if "error" in data:
            raise RuntimeError(json.dumps(data["error"]))
        record = {
            "url": url,
            "retrieved_at": datetime.now(timezone.utc).isoformat(),
            "response": data,
        }
        write_json(path, record)
        time.sleep(0.15)
    return record


def slug(title):
    return re.sub(r"[^a-z0-9]+", "-", title.removeprefix("File:").lower()).strip("-")


def revision_url(title, revision):
    title = urllib.parse.quote(title.replace(" ", "_"), safe=":/")
    return f"https://oldschool.runescape.wiki/w/{title}?oldid={revision}"


def save_snapshot(page, record):
    revision = page["revisions"][0]
    value = {"page": page}
    data = gzip.compress(
        (json.dumps(value, sort_keys=True, ensure_ascii=True) + "\n").encode(),
        mtime=0,
    )
    path = EVIDENCE / "sources" / f"{page['pageid']}-{revision['revid']}.json.gz"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return {
        "path": path.relative_to(ROOT).as_posix(),
        "sha256": sha256(data),
        "size_bytes": len(data),
    }


def pages(titles):
    """Fetch only named pages; derive image names locally, not transclusion fan-out."""
    results = []
    for offset in range(0, len(titles), 20):
        record = query({
            "titles": "|".join(titles[offset:offset + 20]),
            "redirects": 1,
            "prop": "revisions",
            "rvprop": "ids|timestamp|content",
            "rvslots": "main",
        })
        for page in record["response"]["query"]["pages"]:
            if page.get("missing"):
                results.append({"title": page["title"], "missing": True})
                continue
            snapshot = save_snapshot(page, record)
            rev = page["revisions"][0]
            content = rev["slots"]["main"]["content"]
            images = set(re.findall(r"\[\[(?:File|Image):([^|\]\n]+)", content))
            images.update(re.findall(
                r"(?m)^\s*(?:File:)?([^|\n{}<>\[\]]+\.(?:png|gif|jpg|ogg|webm))\s*\|",
                content,
            ))
            results.append({
                "title": page["title"],
                "page_id": page["pageid"],
                "revision": rev["revid"],
                "revision_timestamp": rev["timestamp"],
                "url": revision_url(page["title"], rev["revid"]),
                "snapshot": snapshot,
                "images": sorted(images),
            })
    old_path = EVIDENCE / "pages.json"
    previous = json.loads(old_path.read_text()) if old_path.exists() else []
    merged = {p["title"]: p for p in previous}
    merged.update({p["title"]: p for p in results if not p.get("missing")})
    write_json(old_path, sorted(merged.values(), key=lambda p: p["title"]))
    return results


def search(term, namespace, offset=0):
    record = query({
        "list": "search", "srnamespace": namespace, "srsearch": term,
        "srlimit": 30, "srprop": "size|timestamp", "sroffset": offset,
    })
    return {
        "term": term,
        "total_hits": record["response"]["query"]["searchinfo"]["totalhits"],
        "matches": record["response"]["query"]["search"],
        "continuation": record["response"].get("continue"),
    }


def image_facts(data):
    with Image.open(io.BytesIO(data)) as image:
        image.verify()
    durations = []
    pixel_hashes = []
    with Image.open(io.BytesIO(data)) as image:
        dimensions = [image.width, image.height]
        for index in range(getattr(image, "n_frames", 1)):
            image.seek(index)
            image.load()
            durations.append(image.info.get("duration"))
            pixel_hashes.append(sha256(image.convert("RGBA").tobytes()))
        return {
            "format": image.format,
            "dimensions": dimensions,
            "frame_count": len(pixel_hashes),
            "frame_duration_ms": durations,
            "frame_rgba_sha256": pixel_hashes,
        }


def media(titles, download):
    results = []
    index = EVIDENCE / "public-media.json"
    previous = json.loads(index.read_text()) if index.exists() else []
    known = {entry["id"]: entry for entry in previous}
    for offset in range(0, len(titles), 20):
        names = [t if t.startswith("File:") else "File:" + t
                 for t in titles[offset:offset + 20]]
        record = query({
            "titles": "|".join(names), "redirects": 1,
            "prop": "imageinfo|revisions",
            "rvprop": "ids|timestamp|content", "rvslots": "main",
            "iiprop": "timestamp|url|size|sha1|mime|extmetadata",
            "iilimit": 1,
        })
        continuation = record["response"].get("continue", {})
        if set(continuation) - {"iistart", "continue"}:
            raise RuntimeError("Incomplete page results; narrow titles")
        for page in record["response"]["query"]["pages"]:
            if not page.get("imageinfo"):
                results.append({"title": page["title"], "missing": True})
                continue
            rev = page["revisions"][0]
            info = page["imageinfo"][0]
            entry = {
                "id": "wiki." + slug(page["title"]),
                "title": page["title"],
                "file_page_id": page["pageid"],
                "file_page_revision": rev["revid"],
                "file_page_revision_timestamp": rev["timestamp"],
                "file_page_url": revision_url(page["title"], rev["revid"]),
                "history_url": "https://oldschool.runescape.wiki/w/"
                    + urllib.parse.quote(page["title"].replace(" ", "_"), safe=":")
                    + "?action=history",
                "original_url": info["url"],
                "upload_timestamp": info["timestamp"],
                "capture_timestamp": None,
                "capture_build": None,
                "imageinfo": info,
                "retrieved_at": record["retrieved_at"],
                "source_snapshot": save_snapshot(page, record),
                "imageinfo_api_url": record["url"],
                "history_scope": "Latest upload only; older image-history continuation "
                    "is intentionally not consumed. File page history URL is retained.",
            }
            if download:
                ext = Path(urllib.parse.unquote(
                    urllib.parse.urlparse(info["url"]).path)).suffix.lower()
                path = MEDIA / (slug(page["title"]) + ext)
                attempts = []
                delivery_url = None
                reused = path.exists()
                if reused:
                    data = path.read_bytes()
                else:
                    for attempt in range(3):
                        # Polish can change cached PNG bytes, including ?download=1.
                        # A fresh public cache key is still checked against imageinfo.
                        delivery_url = info["url"] + (
                            "&" if "?" in info["url"] else "?"
                        ) + f"download=1&reference_original={info['sha1']}-{time.time_ns()}"
                        data = get(delivery_url, min(MAX_RESPONSE, info["size"] + 1))
                        actual_sha1 = hashlib.sha1(data).hexdigest()
                        attempts.append({
                            "url": delivery_url, "size_bytes": len(data),
                            "sha1": actual_sha1,
                            "matches_original": actual_sha1 == info["sha1"],
                        })
                        if actual_sha1 == info["sha1"] and len(data) == info["size"]:
                            break
                        print(
                            f"Rejected transformed delivery for {page['title']} "
                            f"(attempt {attempt + 1})", file=sys.stderr, flush=True,
                        )
                if len(data) != info["size"]:
                    raise ValueError(f"imageinfo byte length mismatch: {page['title']}")
                if hashlib.sha1(data).hexdigest() != info["sha1"]:
                    raise ValueError(f"imageinfo SHA-1 mismatch: {page['title']}")
                facts = (mp4_facts(data) if info["mime"] == "video/mp4"
                         else image_facts(data))
                if facts["dimensions"] != [info["width"], info["height"]]:
                    raise ValueError(f"imageinfo dimension mismatch: {page['title']}")
                path.parent.mkdir(parents=True, exist_ok=True)
                if not reused:
                    path.write_bytes(data)
                prior = known.get(entry["id"])
                if reused and prior and prior["sha256"] == sha256(data):
                    delivery_url = prior["retrieval_url"]
                    attempts = prior["delivery_attempts"]
                verified_at = datetime.now(timezone.utc).isoformat()
                entry.update({
                    "path": path.relative_to(ROOT).as_posix(),
                    "sha256": sha256(data), "size_bytes": len(data),
                    "decoded": facts,
                    "retrieval_url": delivery_url,
                    "delivery_attempts": attempts,
                    "bytes_retrieved_at": (
                        prior["bytes_retrieved_at"] if reused and prior
                        else None if reused else verified_at
                    ),
                    "bytes_verified_at": verified_at,
                    "delivery_context": "Reused exact local original; absent prior transfer details remain null."
                        if reused else "Exact original acquired through the recorded public delivery URL.",
                    "mediawiki_original_sha1_verified": True,
                })
                known[entry["id"]] = entry
                write_json(index, sorted(known.values(), key=lambda value: value["id"]))
            results.append(entry)
    if download:
        write_json(index, sorted(known.values(), key=lambda value: value["id"]))
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("pages", "media-info", "retrieve"):
        commands.add_parser(name).add_argument("titles", nargs="+")
    search_parser = commands.add_parser("search")
    search_parser.add_argument("term")
    search_parser.add_argument("--namespace", type=int, default=6)
    search_parser.add_argument("--offset", type=int, default=0)
    args = parser.parse_args()
    if args.command == "search":
        result = search(args.term, args.namespace, args.offset)
    elif args.command == "pages":
        result = pages(args.titles)
    else:
        entries = media(args.titles, args.command == "retrieve")
        result = [
            {k: v for k, v in p.items()
             if k in ("id", "title", "missing", "upload_timestamp", "original_url",
                      "path", "size_bytes", "file_page_revision")}
            | ({"dimensions": [p["imageinfo"]["width"], p["imageinfo"]["height"]],
                "mime": p["imageinfo"]["mime"]}
               if "imageinfo" in p else {})
            for p in entries
        ]
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
