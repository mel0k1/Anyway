// ignore_for_file: avoid_print
import 'dart:convert';
import 'dart:io';

import 'package:anyway/core/rust_core.dart';
import 'package:flutter_test/flutter_test.dart';

/// Integration check for the FFI surface added in this iteration: Tor status,
/// image hashing + OCR, reverse links, entity resolution, timeline, the local
/// leak index and the scan scheduler.
///
/// Runs against the release FFI library when it is available on the machine:
///   LD_LIBRARY_PATH=native/target/release flutter test test/ffi_feature_test.dart
/// Skipped silently in environments without the native core (plain CI runs).
void main() {
  test('new FFI surface: tor, imaging, resolve, timeline, leaks, scheduler', () async {
    if (!RustCore.available) {
      return; // native core not built here — CI runs the Rust tests instead
    }
    print('core version: ${RustCore.version()}');

    // ---------- Tor ----------
    final tor = RustCore.torCheck(jsonEncode({'tor_mode': 'auto', 'tor_socks_port': 9050}));
    print('tor: listening=${tor['listening']} mode=${tor['mode']} isTor=${tor['isTor']}');
    final torOn = RustCore.torCheck(jsonEncode({'tor_mode': 'on', 'tor_socks_port': 9905}));
    print('tor fail-closed: proxy=${torOn['proxy']} listening=${torOn['listening']}');

    // ---------- Imaging ----------
    final png = await File('assets/brand/icon-256.png').readAsBytes();
    final intel = RustCore.imageIntel(base64Encode(png), ocr: true, langs: 'eng');
    print('image: ${intel['format']} ${intel['width']}x${intel['height']} phash=${intel['phash']}');
    expect(intel['phash'], isA<String>());
    final cmp = RustCore.imageCompare(
      jsonEncode({'phash': intel['phash'], 'dhash': intel['dhash'], 'ahash': intel['ahash']}),
      jsonEncode({'phash': intel['phash'], 'dhash': intel['dhash'], 'ahash': intel['ahash']}),
    );
    print('self compare: phashDistance=${cmp['phashDistance']} similar=${cmp['similar']}');
    expect(cmp['phashDistance'], 0);
    final links = RustCore.reverseLinks('https://example.com/pic.jpg');
    print('reverse engines: ${(links['engines'] as List).map((e) => e['name']).join(', ')}');

    // ---------- Entity resolution + timeline ----------
    final snapshot = jsonEncode({
      'findings': [
        {
          'module_id': 'username.enumerate',
          'category': 'social_profile',
          'label': 'GitHub',
          'value': 'https://github.com/mel0k1',
          'url': 'https://github.com/mel0k1',
        },
        {
          'module_id': 'username.enumerate',
          'category': 'social_profile',
          'label': 'GitLab',
          'value': 'https://gitlab.com/mel0k1',
          'url': 'https://gitlab.com/mel0k1',
        },
        {
          'module_id': 'breaches',
          'category': 'breach',
          'label': 'Adobe',
          'value': 'mel0k1@proton.me',
          'detail': {'breachDate': '2013-10-04'},
        },
      ],
    });
    final resolve = RustCore.resolveEntities(snapshot, 45);
    print('resolve stats: ${resolve['stats']}');
    expect((resolve['stats']['identities'] as num).toInt(), 3);
    expect((resolve['stats']['clusters'] as num).toInt(), 1);
    final timeline = RustCore.buildTimeline(snapshot);
    print('timeline events: ${timeline['total']}');
    expect((timeline['total'] as num).toInt(), 1);

    // ---------- Leaks ----------
    final db = '${Directory.systemTemp.path}/anyway_ffi_check_leaks.sqlite';
    final dbFile = File(db);
    if (dbFile.existsSync()) dbFile.deleteSync();
    final imported = RustCore.leaksImport(
      db,
      'check-dump',
      'victim@example.org:Secret123:10.0.0.1\njohn:doe:pw1\n',
    );
    print('leaks import: ${imported['imported']} records');
    expect((imported['imported'] as num).toInt(), 2);
    final found = RustCore.leaksSearch(db, 'victim@example.org', limit: 10, reveal: false);
    print('leaks search: field=${found['queryField']} total=${found['total']} pass=${found['matches'][0]['password']}');
    expect((found['total'] as num).toInt(), 1);
    expect((found['matches'][0]['password'] as String).contains('\u2022'), isTrue);
    expect(RustCore.leaksStats(db)['records'], 2);
    RustCore.leaksClear(db);
    if (dbFile.existsSync()) dbFile.deleteSync();

    // ---------- Scheduler ----------
    RustCore.schedulerSet('[]');
    final id = RustCore.schedulerUpsert(jsonEncode({
      'query': 'example.org',
      'interval_secs': 3600,
      'enabled': false,
    }));
    final entries = RustCore.schedulerList();
    print('scheduler: id=$id entries=${entries.length} dueIn=${entries.first.dueIn}');
    expect(entries, hasLength(1));
    expect(entries.first.enabled, isFalse);
    expect(entries.first.dueIn, greaterThan(0));
    expect(RustCore.schedulerRemove(id), isTrue);
    expect(RustCore.schedulerList(), hasLength(0));
  }, timeout: const Timeout(Duration(minutes: 5)));
}
