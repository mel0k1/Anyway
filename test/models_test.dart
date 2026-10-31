import 'package:flutter_test/flutter_test.dart';
import 'package:anyway/core/models.dart';
import 'package:anyway/core/report_exporter.dart';

void main() {
  test('detectTargetKind classifies queries', () {
    expect(detectTargetKind('mel0k1'), TargetKind.username);
    expect(detectTargetKind('@mel0k1'), TargetKind.username);
    expect(detectTargetKind('user@example.org'), TargetKind.email);
    expect(detectTargetKind('8.8.8.8'), TargetKind.ipAddress);
    expect(detectTargetKind('example.org'), TargetKind.domain);
    expect(detectTargetKind('+79261234567'), TargetKind.phoneNumber);
    expect(detectTargetKind(''), TargetKind.unknown);
  });

  test('TargetKind wire names roundtrip', () {
    for (final k in TargetKind.values) {
      expect(TargetKind.fromName(k.wire), k);
    }
  });

  test('ScanSnapshot parses rust payload', () {
    final snap = ScanSnapshot.fromJson({
      'scanId': 'scan-1',
      'target': {'kind': 'ip_address', 'raw': '8.8.8.8', 'normalized': '8.8.8.8'},
      'status': 'done',
      'startedAt': 1700000000,
      'finishedAt': 1700000042,
      'modules': [
        {'id': 'ip.intel', 'done': 3, 'total': 3, 'status': 'done', 'found': 2},
      ],
      'findings': [
        {
          'module_id': 'ip.intel',
          'category': 'ip_geo',
          'label': 'Geolocation',
          'value': 'Mountain View, United States (America/Los_Angeles)',
          'url': null,
          'confidence': 'high',
          'detail': {'city': 'Mountain View'},
        },
      ],
      'log': ['target detected: TargetKind.ipAddress'],
    });

    expect(snap.status, 'done');
    expect(snap.target.kind, TargetKind.ipAddress);
    expect(snap.findings, hasLength(1));
    expect(snap.findings.first.confidence, Confidence.high);
    expect(snap.findings.first.detail?['city'], 'Mountain View');
    expect(snap.modules.first.found, 2);
    expect(snap.running, isFalse);
  });


  test('multi-target detection uses first token', () {
    expect(detectTargetKind('torvalds, example.org'), TargetKind.username);
    expect(detectTargetKind('8.8.8.8; 1.1.1.1; 9.9.9.9'), TargetKind.ipAddress);
    expect(detectTargetKind('a@b.co, c@d.io'), TargetKind.email);
  });

  test('snapshot parses multi-target payload with labels', () {
    final snap = ScanSnapshot.fromJson({
      'scanId': 'scan-3',
      'target': {'kind': 'username', 'raw': 'a, b', 'normalized': 'a, b'},
      'targets': [
        {'kind': 'username', 'raw': 'a', 'normalized': 'a'},
        {'kind': 'username', 'raw': 'b', 'normalized': 'b'},
      ],
      'status': 'done',
      'startedAt': 1,
      'finishedAt': 2,
      'modules': [
        {'id': 'username.enumerate', 'label': 'a', 'done': 5, 'total': 5, 'status': 'done', 'found': 2},
        {'id': 'username.enumerate', 'label': 'b', 'done': 5, 'total': 5, 'status': 'done', 'found': 1},
      ],
      'findings': [
        {
          'module_id': 'username.enumerate',
          'category': 'social_profile',
          'label': 'GitHub',
          'value': 'https://github.com/a',
          'url': null,
          'confidence': 'high',
          'target': 'a',
        },
      ],
      'log': [],
    });
    expect(snap.multiTarget, isTrue);
    expect(snap.targets, hasLength(2));
    expect(snap.modules.first.label, 'a');
    expect(snap.findings.first.target, 'a');
  });

  test('module info carries optional site count', () {
    final m = ModuleInfo.fromJson({
      'id': 'username.enumerate',
      'name': 'Username Enumeration',
      'description': 'Probes sites',
      'accepts': ['username'],
      'sites': 706,
    });
    expect(m.sites, 706);
    final plain = ModuleInfo.fromJson({
      'id': 'ip.intel',
      'name': 'IP Intelligence',
      'description': 'geo',
      'accepts': ['ip_address'],
    });
    expect(plain.sites, isNull);
  });

  test('snapshot parses graph payload with nodes and edges', () {
    final snap = ScanSnapshot.fromJson({
      'scanId': 'scan-4',
      'target': {'kind': 'username', 'raw': 'mel0k1', 'normalized': 'mel0k1'},
      'status': 'done',
      'startedAt': 1,
      'finishedAt': 2,
      'modules': [],
      'findings': [
        {
          'module_id': 'username.enumerate',
          'category': 'social_profile',
          'label': 'GitHub',
          'value': 'https://github.com/mel0k1',
          'confidence': 'high',
        },
      ],
      'graph': {
        'nodes': [
          {
            'id': 'Username:mel0k1',
            'kind': 'username',
            'label': 'mel0k1',
            'confidence': 'high',
            'sources': ['scan_target'],
            'root': true,
          },
          {
            'id': 'Platform:github',
            'kind': 'platform',
            'label': 'GitHub',
            'confidence': 'high',
            'sources': ['username.enumerate'],
            'root': false,
          },
        ],
        'edges': [
          {
            'from': 'Username:mel0k1',
            'to': 'Platform:github',
            'relation': 'profile_on',
            'confidence': 'high',
            'via': 'username.enumerate',
          },
        ],
      },
      'log': [],
    });

    expect(snap.graph.nodes, hasLength(2));
    expect(snap.graph.edges, hasLength(1));
    expect(snap.graph.nodes.first.root, isTrue);
    expect(snap.graph.nodes[1].kind, GraphNodeKind.platform);
    expect(snap.graph.edges.first.relation, 'profile_on');

    final restored = ScanSnapshot.fromJson(snap.toJson());
    expect(restored.graph.nodes, hasLength(2));
    expect(restored.graph.edges.first.via, 'username.enumerate');
  });

  test('snapshot without graph falls back to empty', () {
    final snap = ScanSnapshot.fromJson({
      'scanId': 's',
      'target': {'kind': 'username', 'raw': 'x', 'normalized': 'x'},
      'status': 'running',
      'startedAt': 1,
      'modules': [],
      'findings': [],
      'log': [],
    });
    expect(snap.graph.nodes, isEmpty);
    expect(snap.graph.edges, isEmpty);
  });

  test('confidence labels use confirmed taxonomy', () {
    expect(Confidence.high.label, 'Confirmed');
    expect(Confidence.medium.label, 'Questionable');
    expect(Confidence.low.label, 'Unverified');
    expect(ConfidenceX.fromName('high'), Confidence.high);
    expect(ConfidenceX.fromName('bogus'), Confidence.medium);
  });

  test('markdown report contains target and tables', () {
    final snap = ScanSnapshot.fromJson({
      'scanId': 'scan-2',
      'target': {'kind': 'username', 'raw': 'torvalds', 'normalized': 'torvalds'},
      'status': 'done',
      'startedAt': 1700000000,
      'finishedAt': 1700000010,
      'modules': [],
      'findings': [
        {
          'module_id': 'username.enumerate',
          'category': 'social_profile',
          'label': 'GitHub',
          'value': 'https://github.com/torvalds',
          'url': 'https://github.com/torvalds',
          'confidence': 'high',
        },
      ],
      'graph': {
        'nodes': [
          {
            'id': 'Username:torvalds',
            'kind': 'username',
            'label': 'torvalds',
            'confidence': 'high',
            'sources': ['scan_target'],
            'root': true,
          },
          {
            'id': 'Platform:github',
            'kind': 'platform',
            'label': 'GitHub',
            'confidence': 'high',
            'sources': ['username.enumerate'],
            'root': false,
          },
        ],
        'edges': [
          {
            'from': 'Username:torvalds',
            'to': 'Platform:github',
            'relation': 'profile_on',
            'confidence': 'high',
            'via': 'username.enumerate',
          },
        ],
      },
      'log': [],
    });

    final md = ReportExporter.markdown(snap);
    expect(md, contains('# Anyway report: torvalds'));
    expect(md, contains('## username.enumerate'));
    expect(md, contains('https://github.com/torvalds'));
    expect(md, contains('| Confirmed |'));
  });

  test('markdown report embeds mermaid graph', () {
    final snap = ScanSnapshot.fromJson({
      'scanId': 'scan-5',
      'target': {'kind': 'domain', 'raw': 'corp.io', 'normalized': 'corp.io'},
      'status': 'done',
      'startedAt': 1,
      'modules': [],
      'findings': [],
      'graph': {
        'nodes': [
          {
            'id': 'Domain:corp.io',
            'kind': 'domain',
            'label': 'corp.io',
            'confidence': 'high',
            'sources': ['scan_target'],
            'root': true,
          },
          {
            'id': 'Ip:93.184.216.34',
            'kind': 'ip',
            'label': '93.184.216.34',
            'confidence': 'high',
            'sources': ['domain.intel'],
            'root': false,
          },
          {
            'id': 'Domain:missing.io',
            'kind': 'domain',
            'label': 'missing.io',
            'confidence': 'low',
            'sources': ['ip.intel'],
            'root': false,
          },
        ],
        'edges': [
          {
            'from': 'Domain:corp.io',
            'to': 'Ip:93.184.216.34',
            'relation': 'resolves_to',
            'confidence': 'high',
            'via': 'domain.intel',
          },
          {
            'from': 'Domain:corp.io',
            'to': 'Domain:missing.io',
            'relation': 'ptr_record',
            'confidence': 'low',
            'via': 'ip.intel',
          },
        ],
      },
      'log': [],
    });

    final md = ReportExporter.markdown(snap);
    expect(md, contains('## Relationship graph'));
    expect(md, contains('3 nodes · 2 links'));
    expect(md, contains('```mermaid'));
    expect(md, contains('-->|"resolves_to"|'));
    expect(md, contains('ptr_record · unverified'));
    expect(md, contains('classDef high'));
  });
}
