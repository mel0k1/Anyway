# Anyway vs the OSINT landscape

A factual self-audit: what Anyway does today, how the well-known tools compare, and what is genuinely missing. Checked against the upstream projects in November 2026.

## Positioning

Anyway is a desktop OSINT workstation, not a CLI or a SaaS panel. Its differentiators: a Rust core that fans every module out over Tokio, a GUI built on Material 3 Expressive, an entity relationship graph generated from every scan, per-finding confidence tagging with anti-false-positive nonce calibration, multi-target scans, and local case storage with Markdown/JSON export. No API keys, no telemetry, no installers.

## Tool-by-tool comparison

| | Anyway | Sherlock | Maigret | Blackbird | holehe | theHarvester | SpiderFoot | Maltego |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Type | Desktop GUI + Rust core | CLI | CLI | CLI | CLI | CLI | Web UI / CLI automation | Commercial link-analysis suite |
| Username → accounts | 1,123 sites (built-in + WhatsMyName) | 400+ sites | 3,000+ sites (top 500 default) | 600+ sites (WhatsMyName) | – | – | via modules | via transforms |
| Email → accounts | Gravatar + provider intel only | – | partial | yes, same 600+ list | 120+ via password-recovery | – | via modules | via transforms |
| Email/domain/IP passive recon | yes (MX, RDAP, DoH DNS, crt.sh, ip-api) | – | – | – | – | 38+ passive sources | 200+ modules | 100+ connectors |
| Relationship graph | built from every scan, interactive | no | no | no | no | no | built-in for some modules | the whole product |
| Confidence tagging | per finding + per node, nonce-calibrated | no | partial (tags) | low-FP list | exists/rate-limit flags | no | per-module | manual analyst work |
| False-positive defenses | re-probe with random nonce, WAF detection, marker/status/redirect heuristics | basic heuristics | per-site check + disabled-site list | WhatsMyName vetted list | recovery-page parsing | n/a | source-dependent | analyst must verify |
| Export | Markdown, JSON, cases on disk | txt/json | txt, CSV, PDF, HTML, xmind | JSON, CSV, PDF | json | csv/json/xml | CSV, web report | graph exports, reports |
| Proxy / Tor | http(s), socks5, socks5h, hard-fail on bad proxy | socks4/5, tor | proxy support | proxy support | proxy support | proxy | proxy | via system |
| Automation / scheduling | no | no | no | no | no | no | yes (scheduled scans) | machines |
| Extensibility | module registry in Rust, site JSON + bundled WhatsMyName snapshot | site YAML | site JSON, tags | site list | python modules | python sources | 200+ python modules | transform hub / SDK |
| Price | free, GPL-3.0 | free MIT | free MIT | free MIT | free GPL | free GPL | free / commercial cloud | CE free (24 results/transform), paid tiers €3k+/yr |

Sources: [Sherlock](https://github.com/sherlock-project/sherlock), [Maigret](https://github.com/soxoj/maigret), [Blackbird](https://github.com/p1ngul1n0/blackbird), [holehe](https://github.com/megadose/holehe), [theHarvester](https://github.com/laramies/theHarvester), [SpiderFoot](https://github.com/smicallef/spiderfoot), [Maltego CE limits](https://osint-s.com/tools/maltego).

## Where Anyway already leads

1. **Graph + confidence in one surface.** Sherlock, Maigret, Blackbird and holehe all stop at flat result lists. Maltego has graphs but no automatic confidence model — links look authoritative even when they are weak. Anyway derives typed, confidence-scored edges from what the modules actually proved and lets the analyst filter by Confirmed / Questionable / Unverified.
2. **Anti-false-positive calibration.** The nonce re-probe (detecting platforms that report random names as taken) and WAF-aware downgrading are rare even among commercial tools; Blackbird leans on a curated list instead of probing logic.
3. **Speed and packaging.** One Rust binary core with a shared Tokio client, a signed release pipeline (tar.gz / zip + SHA-256 from GitHub Actions) and no Python environment to babysit.
4. **Multi-target scans** across mixed target kinds in one pass.

## Gaps, honestly

P0 — directly competes on core value:

- **Site database depth.** Done in November 2026: the WhatsMyName JSON is imported wholesale — the merged database now spans 1,123 sites with category tags, marker-level detection from WMN entries and host-level deduplication that keeps the strongest heuristic per platform. Remaining gap: Maigret's 3,000+ long tail, reachable through a Maigret-DB importer.
- **Email → registered accounts.** Done in November 2026: the `email.accounts` module ports the holehe technique — registration, login and password-recovery probes across 20 popular services (GitHub, Discord, Spotify, Twitter/X, Proton, Mail.ru, Firefox, Nike, Pinterest, Internet Archive and more) — with per-service response verdicts, evidence strings and confidence tagging that feed the graph as platform nodes.
- **Breach data.** Done in November 2026: the `email.breaches` module queries the Have I Been Pwned v3 API with a user-supplied free key (stored locally in Settings) and reports every breach with dates, exposed record counts and data classes as Confirmed findings. Remaining gap: self-hostable DeHashed-style integrations and feeding breach names into the graph as dedicated nodes.
- **Report formats.** Done in November 2026: results export as Markdown, JSON or a self-contained dark-themed HTML report with confidence badges, the entity/edge tables of the relationship graph and the scan log. Remaining gap: PDF rendering and Maigret-style per-platform detail pages.

P1 — differentiating but not blocking:

- **Recursive pivoting.** Maigret extracts usernames/identifiers from found profile pages and rescans automatically. Anyway stops at one hop; the graph's lead nodes should be clickable into a new scan.
- **Headless browser checks.** JS-only sites fail plain HTTP heuristics; an optional WebDriver/CDP pass would raise coverage where marker heuristics cannot.
- **Site categories and bulk toggles** (Maigret `--tags`): filter scans by category or country instead of per-module switches only.
- **Social graph enrichment**: followers/friends of found profiles as graph nodes (GitHub API already available in enrich).
- **Reverse image search** (Yandex/Lens/TinEye deep links exist in dorks only) and more image metadata sources beyond EXIF.
- **macOS / Android targets** — the core is portable, the runners are not built yet.
- **CLI mode** reusing the Rust core for scripting and CI use.

P2 — power users and scale:

- **Scheduling / monitoring** (SpiderFoot territory): re-scan a case nightly, alert on new findings.
- **Optional premium APIs** (Shodan, Censys, VirusTotal — real queries, not just dork links).
- **Onion-space checks** via Tor (Ahmia, onion site liveness).
- **Plugin packs**: hot-loadable site YAML packs and community module registry.
- **Localization** of the UI (currently English-only).

## Bottom line

Anyway's core loop (scan → calibrate → graph → confidence → case) is already stronger than every free CLI in this table. The distance to the leaders is concentrated in four concrete areas: database breadth (P0), email→accounts vector (P0), breach intel (P0) and richer reports (P0). Everything else is roadmap, not foundation.
