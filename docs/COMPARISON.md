# Anyway vs the OSINT landscape

A factual self-audit: what Anyway does today, how the well-known tools compare, and what is genuinely missing. Checked against the upstream projects in November 2026.

## Positioning

Anyway is a desktop OSINT workstation with an Android arm64 build, not a CLI or a SaaS panel. Its differentiators: a Rust core that fans every module out over Tokio, a GUI built on Material 3 Expressive, an entity relationship graph generated from every scan and persisted across runs in a diffable entity store, per-finding confidence tagging with anti-false-positive nonce calibration, multi-target scans, a re-scan planner that reports only new findings, and local case storage with Markdown/JSON/HTML/PDF export. No telemetry and no accounts; the only API keys are the user's own optional HIBP and VirusTotal keys, stored locally.

## Tool-by-tool comparison

| | Anyway | Sherlock | Maigret | Blackbird | holehe | theHarvester | SpiderFoot | Maltego |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Type | Desktop GUI + Rust core | CLI | CLI | CLI | CLI | CLI | Web UI / CLI automation | Commercial link-analysis suite |
| Username → accounts | 1,123 sites (built-in + WhatsMyName) | 400+ sites | 3,000+ sites (top 500 default) | 600+ sites (WhatsMyName) | – | – | via modules | via transforms |
| Email → accounts | holehe-style probes across 20 services + Gravatar/provider intel | – | partial | yes, same 600+ list | 120+ via password-recovery | – | via modules | via transforms |
| Email/domain/IP passive recon | yes (MX, RDAP, DoH DNS, crt.sh, ip-api, Wayback CDX history, VirusTotal passive DNS with key) | – | – | – | – | 38+ passive sources | 200+ modules | 100+ connectors |
| Relationship graph | built from every scan, interactive, persistent entity store with run-to-run diff | no | no | no | no | no | built-in for some modules | the whole product |
| Confidence tagging | per finding + per node, nonce-calibrated | no | partial (tags) | low-FP list | exists/rate-limit flags | no | per-module | manual analyst work |
| False-positive defenses | re-probe with random nonce, WAF detection, marker/status/redirect heuristics | basic heuristics | per-site check + disabled-site list | WhatsMyName vetted list | recovery-page parsing | n/a | source-dependent | analyst must verify |
| Local breach-dump index | streaming import of colon/semicolon/comma dumps into a searchable SQLite store | – | – | – | – | – | via modules | via transforms |
| Export | Markdown, JSON, HTML, PDF, cases on disk | txt/json | txt, CSV, PDF, HTML, xmind | JSON, CSV, PDF | json | csv/json/xml | CSV, web report | graph exports, reports |
| Proxy / Tor | http(s), socks5, socks5h, hard-fail on bad proxy; embedded Tor (arti) when no daemon runs | socks4/5, tor | proxy support | proxy support | proxy support | proxy | proxy | via system |
| Automation / scheduling | built-in re-scan planner, per-entry cadence, new-findings diff | no | no | no | no | no | yes (scheduled scans) | machines |
| Extensibility | module registry in Rust, site JSON + bundled WhatsMyName snapshot | site YAML | site JSON, tags | site list | python modules | python sources | 200+ python modules | transform hub / SDK |
| Price | free, GPL-3.0 | free MIT | free MIT | free MIT | free GPL | free GPL | free / commercial cloud | CE free (24 results/transform), paid tiers €3k+/yr |

Sources: [Sherlock](https://github.com/sherlock-project/sherlock), [Maigret](https://github.com/soxoj/maigret), [Blackbird](https://github.com/p1ngul1n0/blackbird), [holehe](https://github.com/megadose/holehe), [theHarvester](https://github.com/laramies/theHarvester), [SpiderFoot](https://github.com/smicallef/spiderfoot), [Maltego CE limits](https://osint-s.com/tools/maltego).

## Where Anyway already leads

1. **Graph + confidence in one surface.** Sherlock, Maigret, Blackbird and holehe all stop at flat result lists. Maltego has graphs but no automatic confidence model — links look authoritative even when they are weak. Anyway derives typed, confidence-scored edges from what the modules actually proved and lets the analyst filter by Confirmed / Questionable / Unverified.
2. **Anti-false-positive calibration.** The nonce re-probe (detecting platforms that report random names as taken) and WAF-aware downgrading are rare even among commercial tools; Blackbird leans on a curated list instead of probing logic.
3. **Speed and packaging.** One Rust binary core with a shared Tokio client, a signed release pipeline (Linux tar.gz, Windows zip, Android arm64 APK + SHA-256 from GitHub Actions) and no Python environment to babysit.
4. **Multi-target scans** across mixed target kinds in one pass.
5. **Local-first data plane.** The entity store, the leak-dump index and the case files are all plain files on disk; SpiderFoot and Maltego organize their data around a server process or a cloud account.

## Gaps, honestly

P0 — directly competes on core value:

- **Site database depth.** Done in November 2026: the WhatsMyName JSON is imported wholesale and the Maigret database ships as an optional long tail with engine templates expanded — the merged database spans 1,100+ core sites and 7,900+ with the long tail enabled, with category tags, marker-level detection from WMN entries and host-level deduplication that keeps the strongest heuristic per platform and prefers built-in entries on ties. Remaining gap: periodic refresh of both snapshots from upstream.
- **Email → registered accounts.** Done in November 2026: the `email.accounts` module ports the holehe technique — registration, login and password-recovery probes across 20 popular services (GitHub, Discord, Spotify, Twitter/X, Proton, Mail.ru, Firefox, Nike, Pinterest, Internet Archive and more) — with per-service response verdicts, evidence strings and confidence tagging that feed the graph as platform nodes.
- **Breach data.** Done in November 2026: the `email.breaches` module queries the Have I Been Pwned v3 API with a user-supplied free key (stored locally in Settings) and reports every breach with dates, exposed record counts and data classes as Confirmed findings. Offline gap is narrowed too: leaked dumps (colon/semicolon/comma separated) stream-import into a local SQLite index with field-filtered search and stats. Remaining gap: self-hostable DeHashed-style integrations and feeding breach names into the graph as dedicated nodes.
- **Report formats.** Done in November 2026: results export as Markdown, JSON, a self-contained dark-themed HTML report with confidence badges, the entity/edge tables of the relationship graph and the scan log — and a PDF rendered by the native core (one page per section, saved next to the other formats). Remaining gap: Maigret-style per-platform detail pages.

P1 — differentiating but not blocking:

- **Recursive pivoting.** Done in November 2026: discovered entities (exposed emails, alt handles, resolved IPs, subdomains, linked domains) are automatically queued as the next scan targets up to a configurable depth and budget, confidence-gated at Questionable+, with role-account, mail-provider and reserved-IP denylists. Every pivot lands in the graph with provenance edges and a depth badge, and any lead node can be re-pivoted manually from the graph. Remaining gap: pivot presets per entity type.
- **Headless browser checks.** Done in November 2026: an optional pass re-probes ambiguous sites (unknown answers, bot walls, JS-rendered pages) through a system Chromium-family browser with --dump-dom style rendering (chrome/edge/chromium/brave auto-detected, capped per scan) and applies the same marker heuristics to the rendered DOM. Remaining gap: full WebDriver/CDP automation for login-walled flows.
- **Site categories and bulk toggles** (Maigret `--tags`). Done in November 2026: every category in the merged database can be set to include-only or excluded from Settings, with live counts; country filters narrow scans the same way. Remaining gap: language tags and scan-level presets.
- **Social graph enrichment**: done for GitHub, Steam and Mastodon in November 2026 — the `social.graph` module pulls public follower/follow lists and links them into the graph as Confirmed username nodes. Remaining gap: federation-wide Mastodon instance enumeration.
- **Reverse image search** (Yandex/Lens/TinEye deep links exist in dorks only) and more image metadata sources beyond EXIF.
- **macOS target** — the core is portable; Android arm64 ships since v0.4.0 (GitHub Actions builds the APK with cargo-ndk), macOS is the remaining runner.
- **CLI mode** reusing the Rust core for scripting and CI use.

P2 — power users and scale:

- **Scheduling / monitoring.** Done in November 2026: a built-in re-scan planner — per-entry cadence with a minimum interval, run-now, a journal that survives restarts, and a run-to-run diff that reports only the new findings, surfaced as a desktop OS notification. Remaining gap: alert routing beyond desktop notifications (webhooks, mail).
- **Optional premium APIs** — VirusTotal passive DNS is wired in with the user's key; Shodan and Censys remain dork links.
- **Onion-space checks**: the Tor transport itself is built in (embedded arti SOCKS5 that prefers an external daemon), but onion-space modules (Ahmia search, onion site liveness) are still missing.
- **Plugin packs**: hot-loadable site YAML packs and community module registry.
- **Localization** of the UI (currently English-only).

## Bottom line

Anyway's core loop (scan → calibrate → graph → confidence → case) is already stronger than every free CLI in this table. The four P0 areas from the previous audit — database breadth, the email→accounts vector, breach intel and richer reports — are closed as of v0.4.0, alongside a local leak-dump index, a persistent entity store with run-to-run diff, a re-scan planner and an embedded Tor transport. What remains is upkeep and scale: periodic snapshot refresh from upstream, per-platform report detail pages, a macOS runner, CLI mode and premium infrastructure APIs.
