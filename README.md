# Anyway

<p align="center">
  <img src="assets/brand/banner.png" alt="Anyway" width="720"/>
</p>

**Anyway** is a cross-platform OSINT workstation: a Flutter interface on top of a Rust investigation core. Point it at a username, email, IP address, domain, phone number or a file — it figures out what the target is, runs the matching reconnaissance modules and keeps everything together in a case you can export as Markdown or JSON.

## Features

- **Adaptive scans** — one input, automatic target detection: username, email, IP, domain or phone number.
- **Username enumeration** — probe 40+ social platforms with status / message / redirect heuristics and WAF awareness.
- **Profile enrichment** — live API lookups on GitHub, Reddit, Hacker News, Dev.to and Keybase.
- **Email recon** — syntax and MX validation, disposable-provider detection, Gravatar resolution.
- **IP & domain intel** — geo/ASN via ip-api, RDAP registration data, DNS records over DoH, subdomains from crt.sh.
- **Phone analysis** — country, line type and validity via libphonenumber.
- **File metadata** — EXIF extraction with GPS and device details for images.
- **Case management** — every scan is a case: saved locally, re-openable, exportable to Markdown and JSON.
- **Material 3 Expressive** — dark-first UI with spring motion, shape morphing and tonal surfaces.

## Status

Pre-release. The Linux and Windows desktop builds are the primary targets; the architecture keeps Android and macOS in reach.

## Architecture

```
┌─────────────────────────────┐
│ Flutter (Material 3E)       │  UI, cases, reports
└────────────┬────────────────┘
             │ dart:ffi · JSON over C ABI
┌────────────┴────────────────┐
│ anyway-ffi                  │  scan manager, polling API
├─────────────────────────────┤
│ anyway-core                 │  async engine, module registry
│  username · email · ip      │
│  domain · phone · exif      │
└─────────────────────────────┘
```

The Rust core owns the network: a Tokio engine fans every module out concurrently, each module streams findings back into a polled snapshot. The Dart side never blocks.

## Getting started

Requirements: Flutter 3.35+, Rust 1.85+.

```sh
git clone https://github.com/mel0k1/Anyway.git
cd Anyway
flutter pub get
flutter run -d linux   # or -d windows
```

The Rust core is built automatically by CMake/Cargo during the first Flutter build.

## License

Distributed under the [GPL-3.0](LICENSE) license.
