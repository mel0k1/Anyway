import 'package:anyway/core/models.dart';
import 'package:anyway/core/report_exporter.dart';
import 'package:flutter_test/flutter_test.dart';

ScanSnapshot _fixture() {
  final graph = OsintGraph(
    nodes: const [
      GraphNode(
        id: 'user',
        kind: GraphNodeKind.username,
        label: 'jane.doe.1990',
        confidence: Confidence.high,
        sources: ['username.enumerate'],
        root: true,
      ),
      GraphNode(
        id: 'gh',
        kind: GraphNodeKind.platform,
        label: 'github.com/jane-doe-1990',
        confidence: Confidence.medium,
        sources: ['username.enumerate'],
      ),
    ],
    edges: const [
      GraphEdge(
        from: 'user',
        to: 'gh',
        relation: 'links_to',
        confidence: Confidence.high,
        via: 'username.enumerate',
      ),
    ],
  );
  return ScanSnapshot(
    scanId: 'scan-test-1',
    target: const Target(
      kind: TargetKind.username,
      raw: 'jane.doe.1990',
      normalized: 'jane.doe.1990',
    ),
    targets: const [
      Target(kind: TargetKind.username, raw: 'jane.doe.1990', normalized: 'jane.doe.1990'),
      Target(kind: TargetKind.email, raw: 'jane@proton.me', normalized: 'jane@proton.me'),
    ],
    status: 'done',
    startedAt: 1798764000,
    finishedAt: 1798764103,
    modules: const [
      ModuleProgress(
        id: 'username.enumerate',
        label: '',
        done: 1,
        total: 1,
        status: 'done',
        found: 1,
      ),
    ],
    findings: const [
      Finding(
        moduleId: 'username.enumerate',
        category: 'social_profile',
        label: 'GitHub',
        value: '<script>alert("xss")</script> & more',
        url: 'https://github.com/jane-doe-1990',
        confidence: Confidence.high,
        target: 'jane.doe.1990',
      ),
      Finding(
        moduleId: 'email.accounts',
        category: 'email_account',
        label: 'Discord',
        value: 'Registered account',
        url: 'https://discord.com',
        confidence: Confidence.high,
        target: 'jane@proton.me',
      ),
    ],
    graph: graph,
    log: const ['target detected: TargetKind.username', 'scan complete'],
  );
}

void main() {
  test('markdown report keeps graph section and pipes escaped', () {
    final md = ReportExporter.markdown(_fixture());
    expect(md, contains('## username.enumerate'));
    expect(md, contains('<script>alert("xss")</script>'));
    expect(md, contains('## Relationship graph'));
    expect(md, contains('classDef high'));
  });

  test('json report round-trips findings', () {
    final json = ReportExporter.jsonReport(_fixture());
    expect(json, contains('"scanId": "scan-test-1"'));
    expect(json, contains('email.accounts'));
  });

  test('html report escapes html and renders all sections', () {
    final html = ReportExporter.html(_fixture());
    expect(html.startsWith('<!DOCTYPE html>'), isTrue);
    expect(html, contains('<title>Anyway report — jane.doe.1990</title>'));
    expect(html, isNot(contains('<script>alert')));
    expect(html, contains('&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;'));
    expect(html, contains('username.enumerate'));
    expect(html, contains('email.accounts'));
    expect(html, contains('class="conf high"'));
    expect(html, contains('Relationship graph'));
    expect(html, contains('★ jane.doe.1990'));
    expect(html, contains('github.com/jane-doe-1990'));
    expect(html, contains('links_to'));
    expect(html, contains('Scan log (2 lines)'));
    expect(html, contains('https://github.com/mel0k1/Anyway'));
  });

  test('html report handles empty findings', () {
    final snap = ScanSnapshot(
      scanId: 'scan-empty',
      target: const Target(kind: TargetKind.unknown, raw: 'x', normalized: 'x'),
      status: 'done',
      startedAt: 1,
      modules: const [],
      findings: const [],
      log: const [],
    );
    final html = ReportExporter.html(snap);
    expect(html, contains('No findings recorded.'));
    expect(html, isNot(contains('Relationship graph')));
  });
}
