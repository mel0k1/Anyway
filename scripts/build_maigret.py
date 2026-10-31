#!/usr/bin/env python3
"""Convert the Maigret database into Anyway's flat SiteDef format.

Expands engine templates (Discourse, XenForo, phpBB, ...), drops disabled or
non-HTTP entries, deduplicates against the built-in and WhatsMyName databases
and emits a compact JSON array compatible with sites.rs SiteDef.
"""

import json
import os
import sys
from urllib.parse import urlsplit

SRC = sys.argv[1] if len(sys.argv) > 1 else "/tmp/maigret-data.json"
HERE = os.path.dirname(os.path.abspath(__file__))
MODULES = os.path.join(HERE, "..", "native", "crates", "anyway-core", "src", "modules")
SITES_JSON = os.path.join(MODULES, "sites.json")
WMN_JSON = os.path.join(MODULES, "wmn-data.json")
OUT = os.path.join(MODULES, "maigret-data.json")

SKIP_TAGS = {"erotic"}
TAG_MAP = {"erotic": "adult"}


def host_of(url: str) -> str:
    try:
        parts = urlsplit(url)
    except ValueError:
        return ""
    host = (parts.hostname or "").lower()
    return host[4:] if host.startswith("www.") else host


def probe_key(probe: str, name: str) -> str:
    p = probe.replace("{username}", "{}").replace("{account}", "{}")
    p = p.strip().lower().rstrip("/")
    return f"{host_of(p)}\x1f{p}\x1f{name.strip().lower()}"


def load_existing_keys() -> set:
    keys = set()

    with open(SITES_JSON, encoding="utf-8") as f:
        for site in json.load(f):
            for field in ("probe", "url", "body"):
                val = site.get(field) or ""
                if "{}" in val:
                    keys.add(probe_key(val, site.get("name", "")))
                    break

    with open(WMN_JSON, encoding="utf-8") as f:
        for site in json.load(f).get("sites", []):
            for field in ("uri_check", "uri_pretty"):
                val = site.get(field) or ""
                if "{account}" in val:
                    keys.add(probe_key(val, site.get("name", "")))
                    break

    return keys


def expand_site(entry: dict, engines: dict) -> dict:
    engine_name = entry.get("engine")
    if engine_name and engine_name in engines:
        eng = engines[engine_name]
        base = dict(eng.get("site", {}))
        for field in ("absenceStrs", "presenseStrs"):
            merged = list(base.get(field) or [])
            for extra in eng.get(field) or []:
                if extra not in merged:
                    merged.append(extra)
            base[field] = merged
        for key, val in entry.items():
            if key in ("absenceStrs", "presenseStrs") and isinstance(val, list):
                merged = list(base.get(key) or [])
                for extra in val:
                    if extra not in merged:
                        merged.append(extra)
                base[key] = merged
            else:
                base[key] = val
        entry = base

    url = entry.get("url") or ""
    main = (entry.get("urlMain") or "").rstrip("/")
    subpath = (entry.get("urlSubpath") or "").rstrip("/")
    url = url.replace("{urlMain}", main).replace("{urlSubpath}", subpath)
    probe = (entry.get("urlProbe") or "").replace("{urlMain}", main).replace(
        "{urlSubpath}", subpath
    )
    return {**entry, "url": url, "urlProbe": probe}


def convert(name: str, entry: dict, engines: dict) -> dict | None:
    if entry.get("disabled") or entry.get("protocol") or entry.get("type"):
        return None

    entry = expand_site(entry, engines)

    url = (entry.get("url") or "").replace("{username}", "{}")
    probe = (entry.get("urlProbe") or "").replace("{username}", "{}")
    main = entry.get("urlMain") or ""
    if "{}" not in url or not host_of(url) or not host_of(main):
        return None
    if not url.lower().startswith(("http://", "https://")):
        return None
    if probe and "{}" not in probe:
        probe = ""

    check = entry.get("checkType")
    tags = [t for t in entry.get("tags") or [] if t not in SKIP_TAGS]
    category = TAG_MAP.get(tags[0] if tags else "", tags[0] if tags else "") or "misc"

    out = {
        "name": name,
        "cat": category,
        "url": url,
        "probe": probe,
        "method": "",
        "body": "",
        "hdr": entry.get("headers") or {},
        "type": "",
        "codes": [],
        "msgs": [],
        "has": [],
        "mark": "",
        "rx": entry.get("regexCheck") or "",
        "unk": [],
        "strip": "",
        "source": "maigret",
    }

    payload = entry.get("requestPayload")
    if payload is not None:
        body = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        if "{username}" in body:
            body = body.replace("{username}", "{}")
        if "{}" not in body:
            return None
        out["method"] = "POST"
        out["body"] = body
    elif (entry.get("requestMethod") or "").upper() == "POST":
        return None

    if check == "status_code":
        out["type"] = "status"
        out["codes"] = [404]
    elif check == "message":
        absent = [s for s in (entry.get("absenceStrs") or []) if s]
        present = [s for s in (entry.get("presenseStrs") or []) if s]
        if not absent and not present:
            return None
        out["type"] = "message"
        out["msgs"] = absent[:6]
        out["has"] = present[:6]
    elif check == "response_url":
        marker = entry.get("errorUrl") or ""
        if not marker:
            return None
        out["type"] = "redirect"
        out["mark"] = marker
    else:
        return None
    return out


def rank(defn: dict) -> int:
    if defn["type"] == "message" and defn["has"]:
        return 3
    if defn["type"] in ("message", "redirect"):
        return 2
    return 1


def main() -> int:
    with open(SRC, encoding="utf-8") as f:
        data = json.load(f)
    engines = data.get("engines", {})
    existing = load_existing_keys()

    best: dict = {}
    for name, entry in data.get("sites", {}).items():
        defn = convert(name, entry, engines)
        if defn is None:
            continue
        key = probe_key(defn["probe"] or defn["url"], defn["name"])
        if key in existing:
            continue
        current = best.get(key)
        if current is None or rank(defn) > rank(current):
            best[key] = defn

    out = sorted(best.values(), key=lambda d: d["name"].lower())
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, separators=(",", ":"))
    print(f"wrote {len(out)} maigret sites ({OUT})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
