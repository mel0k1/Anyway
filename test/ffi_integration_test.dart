import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:anyway/core/rust_core.dart';

void main() {
  test(
    'native core integration: reportPdf, estore, leaks import_file, arti',
    () async {
      expect(RustCore.version(), isNotEmpty);
      final moduleIds = RustCore.moduleList().map((m) => m.id).toSet();
      expect(moduleIds, contains('username.enumerate'));

      final work = await Directory.systemTemp.createTemp('anyway_it_');
      addTearDown(() => work.delete(recursive: true));

      // ---- PDF report through the native core ----
      final snapshot = jsonEncode({
        'scan_id': 'it-scan-1',
        'target': {
          'raw': 'example.com',
          'kind': 'domain',
          'normalized': 'example.com',
        },
        'status': 'done',
        'started_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        'findings': [
          {
            'module_id': 'domain.intel',
            'category': 'domain',
            'label': 'Domain',
            'value': 'example.com',
            'confidence': 'confirmed',
          },
          {
            'module_id': 'email.accounts',
            'category': 'platform_account',
            'label': 'GitHub',
            'value': 'github.com/octocat',
            'confidence': 'confirmed',
          },
        ],
        'timeline': [],
      });
      final pdf = RustCore.reportPdf(snapshot);
      expect(pdf['ok'], true);
      final bytes = base64Decode(pdf['pdf_b64'] as String);
      expect(bytes.length, greaterThan(1000));
      expect(String.fromCharCodes(bytes.take(5)), '%PDF-');

      // ---- persistent entity store: ingest, stats, search, diff ----
      final estoreDb = '${work.path}/estore.db';
      final first = RustCore.estoreIngest(estoreDb, 'it-scan-1', snapshot);
      expect(first['ok'], true);
      expect((first['new_entities'] as num).toInt(), 2);

      final second = RustCore.estoreIngest(estoreDb, 'it-scan-2', snapshot);
      expect(second['ok'], true);
      expect((second['updated_entities'] as num).toInt(), 2);

      final stats = RustCore.estoreStats(estoreDb);
      expect((stats['entities'] as num).toInt(), 2);

      final hits = RustCore.estoreSearch(estoreDb, 'github');
      expect(hits, hasLength(1));
      expect(hits.first.value, contains('github.com'));
      expect(hits.first.timesSeen, 2);
      expect(hits.first.kind, 'platform_account');

      final detail = RustCore.estoreEntity(estoreDb, hits.first.id);
      expect(detail['ok'], true);
      expect(((detail['observations'] as List?) ?? const []).length, 2);

      final diff = RustCore.estoreDiff(estoreDb, 'it-scan-1', 'it-scan-2');
      expect(diff['ok'], true);

      // ---- leak dump: streamed import from a file, search, stats ----
      final leaksDb = '${work.path}/leaks.db';
      final dump = File('${work.path}/combo.txt');
      await dump.writeAsString(
        [
          'alice@example.com:Passw0rd!:alice',
          'bob@example.com;hunter2;bob',
          '{"email":"carol@example.com","password":"winter2024","username":"carol"}',
          '# a comment line must be skipped',
          'not-a-record',
        ].join('\n'),
      );
      final imp = RustCore.leaksImportFile(leaksDb, 'it-dump', dump.path);
      expect(imp['ok'], true);
      expect((imp['imported'] as num).toInt(), 3);

      final found = RustCore.leaksSearch(
        leaksDb,
        'alice@example.com',
        field: 'email',
      );
      expect(found['queryField'], 'email');
      expect((found['total'] as num).toInt(), 1);
      final row = ((found['matches'] ?? const []) as List)
          .cast<Map<String, dynamic>>()
          .first;
      expect(row['email'], contains('alice@example.com'));
      expect(
        row['password'],
        'P••••••',
        reason: 'passwords stay masked unless reveal=true',
      );

      final revealed = RustCore.leaksSearch(
        leaksDb,
        'alice@example.com',
        reveal: true,
        field: 'email',
      );
      final revealedRow = ((revealed['matches'] ?? const []) as List)
          .cast<Map<String, dynamic>>()
          .first;
      expect(revealedRow['password'], 'Passw0rd!');

      final statsLeak = RustCore.leaksStats(leaksDb);
      expect((statsLeak['records'] as num).toInt(), 3);

      // ---- embedded Tor (arti): lifecycle only, no network demands ----
      final arti = RustCore.artiStart(9053, work.path);
      expect(arti['ok'], true);
      final status = RustCore.artiStatus();
      expect(status['ok'], true);
      expect((status['status'] as String?) ?? '', isNotEmpty);
      // ignore: avoid_print
      print('arti lifecycle: ${status['status']}');
    },
    skip: !RustCore.available,
  );
}
