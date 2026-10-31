# Anyway

<p align="center">
  <img src="assets/brand/banner.png" alt="Anyway banner" width="840"/>
</p>

<h3 align="center">One query. Every source. Rust speed.</h3>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue.svg" alt="License"/></a>
  <img src="https://img.shields.io/badge/Flutter-3.35%2B-02569B?logo=flutter&logoColor=white" alt="Flutter"/>
  <img src="https://img.shields.io/badge/Rust-1.85%2B-DEA584?logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/platforms-linux%20%7C%20windows-lightgrey" alt="Platforms"/>
  <img src="https://img.shields.io/badge/UI-Material%203%20Expressive-7C6AF0" alt="M3 Expressive"/>
</p>

**Anyway** is a cross-platform OSINT workstation: a Flutter interface on top of a Rust investigation core. Point it at a username, email, IP address, domain, phone number or an image file — it detects the target type, runs the matching reconnaissance modules concurrently and keeps everything in a case you can export as Markdown or JSON.

## Modules

| Module | Target | What it does |
| --- | --- | --- |
| `username.enumerate` | Username | Probes 36 social and developer platforms with status / message / POST-probe heuristics and WAF awareness |
| `username.enrich` | Username | Official API lookups: GitHub, Reddit, Hacker News, Dev.to, Keybase |
| `email.recon` | Email | Syntax + MX validation over DoH, disposable and provider detection, Gravatar avatar & profile resolution |
| `ip.intel` | IP | Geo / ASN / ISP flags via ip-api, RDAP registration data with abuse contacts, reverse DNS |
| `domain.intel` | Domain | RDAP whois, A / AAAA / MX / NS / TXT over DoH, subdomain harvest from crt.sh |
| `phone.analysis` | Phone | libphonenumber parsing: country, line type, validity, E.164 / international formats |
| `file.exif` | File | EXIF extraction: device, software, timestamps, GPS with a map link |

## Screenshots

The interface is dark-first Material 3 Expressive: a navigation rail shell, wavy progress indicators per module, spring-driven motion and tonal surfaces. Screenshots will be added with the first release.

## Architecture

```
┌─────────────────────────────┐
│ Flutter (Material 3E)       │  UI, cases, reports
└────────────┬────────────────┘
             │ dart:ffi · JSON over C ABI
┌────────────┴────────────────┐
│ anyway-ffi                  │  scan manager, polling API
├─────────────────────────────┤
│ anyway-core                 │  Tokio engine, module registry
│  username · email · ip      │
│  domain · phone · exif      │
└─────────────────────────────┘
```

- The Rust core owns the network: a Tokio engine fans every module out concurrently with a shared client, per-module progress and cancellation.
- The FFI surface is a small C ABI (`anyway_scan_start` / `anyway_scan_poll` / `anyway_scan_cancel`) speaking JSON — no codegen, no magic.
- CMake invokes Cargo during the Flutter build, so `flutter run -d linux` builds the native core automatically.
- Every finished scan is archived as a case under the app support directory.

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

## Disclaimer

Anyway is intended for authorized research, security assessments and self-investigations. Respect the terms of service of the platforms it queries and the laws of your jurisdiction.

## License

Distributed under the [GPL-3.0](LICENSE) license.
