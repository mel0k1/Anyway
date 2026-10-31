# Anyway

<p align="center">
  <img src="assets/brand/banner.png" alt="Anyway banner" width="840"/>
</p>

<h3 align="center">One query. Every source. Rust speed.</h3>

<p align="center">
  <a href="https://github.com/mel0k1/Anyway/actions/workflows/ci.yml"><img src="https://github.com/mel0k1/Anyway/actions/workflows/ci.yml/badge.svg" alt="CI"/></a>
  <a href="https://github.com/mel0k1/Anyway/releases/latest"><img src="https://img.shields.io/github/v/release/mel0k1/Anyway?logo=github&label=release" alt="Release"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue.svg" alt="License"/></a>
  <img src="https://img.shields.io/badge/Flutter-3.35%2B-02569B?logo=flutter&logoColor=white" alt="Flutter"/>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-DEA584?logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/platforms-linux%20%7C%20windows-lightgrey" alt="Platforms"/>
  <img src="https://img.shields.io/badge/UI-Material%203%20Expressive-7C6AF0" alt="M3 Expressive"/>
</p>

**Anyway** is a cross-platform OSINT workstation: a Flutter interface on top of a Rust investigation core. Point it at a username, email, IP address, domain, phone number or an image file — it detects the target type, runs the matching reconnaissance modules concurrently and keeps everything in a case you can export as Markdown, JSON or a self-contained HTML report.

## Download

Grab a prebuilt bundle from the [Releases](https://github.com/mel0k1/Anyway/releases) page:

- **Linux** — unpack `anyway-linux-x64.tar.gz` and run `bundle/anyway`
- **Windows** — unpack `anyway-windows-x64.zip` and run `anyway.exe`

Every release is built straight from a version tag by CI, ships a `checksums.txt` and contains no installers or telemetry. Building from source is the same one-liner it always was:

```sh
flutter run -d linux   # or: flutter run -d windows
```

- **1,100+ sites** checked per username — the merged database combines the curated built-in list (Sherlock and Maigret heritage) with the community-vetted [WhatsMyName](https://github.com/WebBreacher/WhatsMyName) set across social, gaming, coding, dating, forum and art platforms, all with status, message, presence and redirect heuristics plus WAF awareness
- **Email → linked accounts** — registration, login and password-recovery probes across 20 popular services (GitHub, Discord, Spotify, Twitter/X, Proton, Mail.ru, Firefox, Nike, Pinterest and more), ported from the holehe technique, reveal which accounts hang off an address
- **Breach intel** — Have I Been Pwned lookup lists every breach that exposed the address with dates, record counts and data classes (bring your own free API key, stored locally in Settings)
- **Relationship graph** — every scan builds an interactive entity graph: platforms, emails, IPs, domains, providers, organizations, locations and device fingerprints as nodes, linked by what the modules actually proved. Pan, zoom and tap any node to trace its evidence chain
- **Confidence tagging** — each finding and graph node carries a trust level: **Confirmed** (explicit marker or authoritative API), **Questionable** (status or redirect heuristics) and **Unverified** (detection that failed calibration)
- **Anti-false-positive calibration** — before trusting a heuristic hit, Anyway re-probes the same platform with a random nonce username; sites that report random names as taken (or serve identical bodies for everyone) are automatically downgraded to Unverified, and WAF-challenged answers never produce findings
- **Multiple targets in one scan** — separate them with commas: `user1, user2, example.org`
- **Proxy & Tor** — route every request through `http`, `https`, `socks5` or `socks5h` (one click for a local Tor daemon), with a hard fail instead of a silent fallback if the proxy is invalid
- **Search dorks** — every scan ends with one-click deep links for Google, Bing, DuckDuckGo, Yandex, GitHub, Pastebin, Shodan, Censys, VirusTotal, Wayback and more, preloaded with the target

## Modules

| Module | Target | What it does |
| --- | --- | --- |
| `username.enumerate` | Username | Probes **1,123 platforms** (built-in + WhatsMyName) with status / message / presence / redirect heuristics, per-site regex validation, rate-limit and WAF awareness |
| `username.enrich` | Username | Official API lookups: GitHub, Reddit, Hacker News, Dev.to, Keybase, Bluesky, Mastodon, Telegram channels, Steam profiles |
| `search.dorks` | Username, email, IP, domain, phone | Prebuilt deep links: search engines, code hosts, paste sites and intel portals (Shodan, Censys, VirusTotal, crt.sh, Wayback, Truecaller…) |
| `email.recon` | Email | Syntax + MX validation over DoH, disposable and provider detection, Gravatar avatar & profile resolution |
| `email.accounts` | Email | Holehe-style registration / login / password-recovery probes across **20 services** that confirm which accounts are linked to the address |
| `email.breaches` | Email | Have I Been Pwned breach lookup: dates, exposed record counts and data classes per breach (free HIBP API key required) |
| `ip.intel` | IP | Geo / ASN / ISP flags via ip-api, RDAP registration data with abuse contacts, reverse DNS |
| `domain.intel` | Domain | RDAP whois, A / AAAA / MX / NS / TXT over DoH, subdomain harvest from crt.sh |
| `phone.analysis` | Phone | libphonenumber parsing: country, line type, validity, E.164 / international formats |
| `file.exif` | File | EXIF extraction: device, software, timestamps, GPS with a map link |

## Screenshots

| Dashboard | Scan in progress |
| --- | --- |
| ![Dashboard](docs/screenshots/01_dashboard.png) | ![Scan](docs/screenshots/02_scan.png) |
| **Results with confidence tagging** | **Relationship graph** |
| ![Results](docs/screenshots/03_results.png) | ![Graph](docs/screenshots/04_graph.png) |
| **Modules** | **Settings and proxy** |
| ![Modules](docs/screenshots/06_modules.png) | ![Settings](docs/screenshots/07_settings.png) |

Dark-first Material 3 Expressive: a top navigation shell with soft rounded corners, wavy progress indicators per module, spring-driven motion and tonal surfaces. The screenshots are rendered deterministically from demo data by `flutter test test/screenshots_test.dart --update-goldens`.

## Architecture

```
┌─────────────────────────────┐
│ Flutter (Material 3E)       │  UI, graph view, cases, reports
└────────────┬────────────────┘
             │ dart:ffi · JSON over C ABI
┌────────────┴────────────────┐
│ anyway-ffi                  │  scan manager, polling API
├─────────────────────────────┤
│ anyway-core                 │  Tokio engine, module registry
│  username · email · ip      │  graph builder, confidence model
│  domain · phone · exif      │
└─────────────────────────────┘
```

- The Rust core owns the network: a Tokio engine fans every module out concurrently with a shared client, per-module progress and cancellation.
- Every snapshot ships a `graph` object — nodes and typed, confidence-scored edges derived from the findings, so the UI and exports always agree with the core.
- The FFI surface is a small C ABI (`anyway_scan_start` / `anyway_scan_poll` / `anyway_scan_cancel`) speaking JSON — no codegen, no magic.
- CMake invokes Cargo during the Flutter build, so `flutter run -d linux` builds the native core automatically.
- Every finished scan is archived as a case under the app support directory.

## Network & privacy

Scans run from your machine and hit the same endpoints a browser would. To keep your own IP out of other people's logs, open **Settings → Network** and pick a proxy:

- `socks5h://127.0.0.1:9050` — a local Tor daemon (DNS resolves remotely through the circuit)
- `socks5h://127.0.0.1:9150` — the SOCKS port of the Tor Browser bundle
- any `http://`, `https://` or `socks5://` endpoint, including authenticated ones: `socks5://user:pass@host:port`

The active route is shown on the scan screen. If the proxy URL is invalid, the scan fails immediately with a log entry — Anyway never silently falls back to a direct connection.

## Getting started

Requirements: [Flutter](https://docs.flutter.dev/get-started/install) 3.35+ and [Rust](https://rustup.rs) 1.85+ (`cargo` on PATH).

```sh
git clone https://github.com/mel0k1/Anyway.git
cd Anyway
flutter pub get
flutter run -d linux   # or: flutter run -d windows
```

Sanity-check the FFI bridge without the UI:

```sh
cargo build --manifest-path native/crates/anyway-ffi/Cargo.toml
export LD_LIBRARY_PATH=$PWD/native/target/debug
dart run tool/ffi_check.dart
```

## Project layout

```
native/
  crates/anyway-core/     engine, registry, modules, site list
  crates/anyway-ffi/      C ABI for Dart
lib/
  core/                   FFI bindings, models, state, cases, export
  ui/                     theme, widgets, screens
linux/ windows/           desktop runners wired to Cargo
```

## Roadmap

A feature-by-feature comparison with Sherlock, Maigret, Blackbird, holehe, theHarvester, SpiderFoot and Maltego — including what is missing here — lives in [docs/COMPARISON.md](docs/COMPARISON.md). The first P0 items from that comparison are landed: email → registered accounts probing, breach intel and a self-contained HTML report with the embedded graph.

## Credits

- [WhatsMyName](https://github.com/WebBreacher/WhatsMyName) by Micah Hoffman (WebBreacher) and the OSINT community — the bundled `wmn-data.json` snapshot (`native/crates/anyway-core/src/modules/`) is distributed unmodified under the [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) license and merged into the username database at build time.
- [holehe](https://github.com/megadose/holehe) by megadose — registration-probe endpoints adapted for the Email Accounts module.
- [Sherlock](https://github.com/sherlock-project/sherlock) and [Maigret](https://github.com/soxoj/maigret) — heritage of curated site entries and detection heuristics.

## Disclaimer

Anyway is intended for authorized research, security assessments and self-investigations. Respect the terms of service of the platforms it queries and the laws of your jurisdiction.

## License

Distributed under the [GPL-3.0](LICENSE) license.
