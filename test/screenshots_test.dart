import 'dart:io';

import 'package:anyway/app.dart';
import 'package:anyway/core/app_state.dart';
import 'package:anyway/core/case_store.dart';
import 'package:anyway/core/models.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> _loadFonts() async {
  final loader = FontLoader('AnywaySans');
  for (final f in [
    'Roboto-Light.ttf',
    'Roboto-Regular.ttf',
    'Roboto-Medium.ttf',
    'Roboto-Bold.ttf',
    'Roboto-Black.ttf',
  ]) {
    final bytes = File('test/fonts/$f').readAsBytesSync();
    loader.addFont(Future.value(ByteData.view(bytes.buffer)));
  }
  await loader.load();

  final exe = Platform.resolvedExecutable;
  final flutterRoot = Directory(
    File(exe).parent.parent.parent.parent.parent.parent.path,
  );
  final icons = File(
    '${flutterRoot.path}/bin/cache/artifacts/material_fonts/MaterialIcons-Regular.otf',
  );
  final iconLoader = FontLoader('MaterialIcons')
    ..addFont(Future.value(ByteData.view(icons.readAsBytesSync().buffer)));
  await iconLoader.load();

  final mono = File('test/fonts/DejaVuSansMono.ttf');
  final monoLoader = FontLoader('monospace')
    ..addFont(Future.value(ByteData.view(mono.readAsBytesSync().buffer)));
  await monoLoader.load();
}

ModuleInfo _module(
  String id,
  String name,
  String description,
  List<TargetKind> accepts, {
  int? sites,
}) => ModuleInfo(
  id: id,
  name: name,
  description: description,
  accepts: accepts,
  sites: sites,
);

List<ModuleInfo> _demoModules() => [
  _module(
    'username.enumerate',
    'Username Enumeration',
    'Probes 1,100+ social, developer, forum, gaming and dating platforms merged from the built-in and WhatsMyName databases with marker, status, redirect heuristics and nonce calibration against false positives',
    [TargetKind.username],
    sites: 1123,
  ),
  _module(
    'username.enrich',
    'Profile Enrichment',
    'Official API lookups: GitHub, Reddit, Hacker News, Dev.to, Keybase, Telegram, Bluesky, Mastodon and Steam',
    [TargetKind.username],
  ),
  _module(
    'email.recon',
    'Email Recon',
    'Syntax and MX validation, disposable detection, provider lookup and Gravatar resolution',
    [TargetKind.email],
  ),
  _module(
    'ip.intel',
    'IP Intelligence',
    'Geo and ASN via ip-api, RDAP registration data and reverse DNS',
    [TargetKind.ipAddress],
  ),
  _module(
    'domain.intel',
    'Domain Intelligence',
    'RDAP whois, DNS records over DoH and subdomains from crt.sh',
    [TargetKind.domain],
  ),
  _module(
    'phone.analysis',
    'Phone Analysis',
    'Parses the number with libphonenumber: country, line type and validity',
    [TargetKind.phoneNumber],
  ),
  _module(
    'file.exif',
    'File Metadata',
    'Extracts EXIF from images: device, software, timestamps and GPS coordinates',
    [TargetKind.file],
  ),
  _module(
    'search.dorks',
    'Search Dorks',
    'One-click deep links: search engines, code hosts, paste sites and intel portals preloaded with the target',
    [
      TargetKind.username,
      TargetKind.email,
      TargetKind.ipAddress,
      TargetKind.domain,
      TargetKind.phoneNumber,
    ],
  ),
];

Finding _f(
  String module,
  String category,
  String label,
  String value, {
  String? url,
  Confidence conf = Confidence.medium,
  Map<String, dynamic>? detail,
  String? target,
}) => Finding(
  moduleId: module,
  category: category,
  label: label,
  value: value,
  url: url,
  confidence: conf,
  target: target,
  detail: detail,
);

List<Finding> _demoFindings() => [
  _f(
    'username.enumerate',
    'social_profile',
    'GitHub',
    'https://github.com/jane-doe-1990',
    conf: Confidence.high,
    target: 'jane.doe.1990',
    detail: {
      'status': 200,
      'marker': 'og:description',
      'bio': 'Security researcher · she/her',
      'followers': 412,
      'joined': '2014-03-11',
    },
  ),
  _f(
    'username.enumerate',
    'social_profile',
    'Reddit',
    'https://www.reddit.com/user/jane_doe_1990',
    conf: Confidence.high,
    target: 'jane.doe.1990',
  ),
  _f(
    'username.enumerate',
    'social_profile',
    'Mastodon',
    'https://mastodon.social/@jane_doe_1990',
    conf: Confidence.medium,
    target: 'jane.doe.1990',
  ),
  _f(
    'username.enumerate',
    'social_profile',
    'Hacker News',
    'https://news.ycombinator.com/user?id=janedoe1990',
    conf: Confidence.medium,
    target: 'jane.doe.1990',
  ),
  _f(
    'username.enumerate',
    'social_profile',
    'Keybase',
    'https://keybase.io/janedoe1990',
    conf: Confidence.low,
    target: 'jane.doe.1990',
  ),
  _f(
    'username.enrich',
    'api_profile',
    'GitHub API',
    'https://api.github.com/users/jane-doe-1990',
    conf: Confidence.high,
    target: 'jane.doe.1990',
    detail: {
      'name': 'Jane D.',
      'company': '@malware-research',
      'location': 'Berlin, Germany',
      'public_repos': 23,
      'followers': 412,
    },
  ),
  _f(
    'username.enrich',
    'gravatar',
    'Gravatar',
    'jane.doe.1990@proton.me',
    conf: Confidence.medium,
    target: 'jane.doe.1990',
  ),
  _f(
    'email.recon',
    'email_info',
    'Mail provider',
    'proton.me · MX protonmail.net · not disposable',
    conf: Confidence.medium,
    target: 'jane.doe.1990@proton.me',
  ),
  _f(
    'search.dorks',
    'search_dork',
    'Google dork',
    '"jane.doe.1990" site:linkedin.com/in',
    conf: Confidence.low,
    target: 'jane.doe.1990',
  ),
  _f(
    'search.dorks',
    'search_dork',
    'GitHub code dork',
    '"jane.doe.1990" OR "jane_doe_1990"',
    conf: Confidence.low,
    target: 'jane.doe.1990',
  ),
];

OsintGraph _demoGraph() {
  final nodes = [
    const GraphNode(
      id: 'user',
      kind: GraphNodeKind.username,
      label: 'jane.doe.1990',
      confidence: Confidence.high,
      sources: ['username.enumerate'],
      root: true,
    ),
    const GraphNode(
      id: 'gh',
      kind: GraphNodeKind.platform,
      label: 'github.com/jane-doe-1990',
      confidence: Confidence.high,
      sources: ['username.enumerate', 'username.enrich'],
    ),
    const GraphNode(
      id: 'rd',
      kind: GraphNodeKind.platform,
      label: 'reddit.com/user/jane_doe_1990',
      confidence: Confidence.high,
      sources: ['username.enumerate'],
    ),
    const GraphNode(
      id: 'md',
      kind: GraphNodeKind.platform,
      label: 'mastodon.social/@jane_doe_1990',
      confidence: Confidence.medium,
      sources: ['username.enumerate'],
    ),
    const GraphNode(
      id: 'hn',
      kind: GraphNodeKind.platform,
      label: 'news.ycombinator.com/janedoe1990',
      confidence: Confidence.medium,
      sources: ['username.enumerate'],
    ),
    const GraphNode(
      id: 'kb',
      kind: GraphNodeKind.platform,
      label: 'keybase.io/janedoe1990',
      confidence: Confidence.low,
      sources: ['username.enumerate'],
    ),
    const GraphNode(
      id: 'mail',
      kind: GraphNodeKind.email,
      label: 'jane.doe.1990@proton.me',
      confidence: Confidence.medium,
      sources: ['username.enrich', 'email.recon'],
    ),
    const GraphNode(
      id: 'prov',
      kind: GraphNodeKind.provider,
      label: 'proton.me',
      confidence: Confidence.medium,
      sources: ['email.recon'],
    ),
    const GraphNode(
      id: 'person',
      kind: GraphNodeKind.person,
      label: 'Jane D. · Berlin',
      confidence: Confidence.medium,
      sources: ['username.enrich'],
    ),
    const GraphNode(
      id: 'lead1',
      kind: GraphNodeKind.lead,
      label: 'LinkedIn dork hit',
      confidence: Confidence.low,
      sources: ['search.dorks'],
    ),
    const GraphNode(
      id: 'lead2',
      kind: GraphNodeKind.lead,
      label: 'X (Twitter) @janedoe1990',
      confidence: Confidence.low,
      sources: ['search.dorks'],
    ),
  ];
  final edges = [
    const GraphEdge(
      from: 'user',
      to: 'gh',
      relation: 'links_to',
      confidence: Confidence.high,
      via: 'username.enumerate',
    ),
    const GraphEdge(
      from: 'user',
      to: 'rd',
      relation: 'links_to',
      confidence: Confidence.high,
      via: 'username.enumerate',
    ),
    const GraphEdge(
      from: 'user',
      to: 'md',
      relation: 'alt_handle',
      confidence: Confidence.medium,
      via: 'username.enumerate',
    ),
    const GraphEdge(
      from: 'user',
      to: 'hn',
      relation: 'links_to',
      confidence: Confidence.medium,
      via: 'username.enumerate',
    ),
    const GraphEdge(
      from: 'user',
      to: 'kb',
      relation: 'links_to',
      confidence: Confidence.low,
      via: 'username.enumerate',
    ),
    const GraphEdge(
      from: 'user',
      to: 'mail',
      relation: 'avatar_on',
      confidence: Confidence.medium,
      via: 'username.enrich',
    ),
    const GraphEdge(
      from: 'mail',
      to: 'prov',
      relation: 'exposes_email',
      confidence: Confidence.medium,
      via: 'email.recon',
    ),
    const GraphEdge(
      from: 'gh',
      to: 'person',
      relation: 'links_to',
      confidence: Confidence.high,
      via: 'username.enrich',
    ),
    const GraphEdge(
      from: 'rd',
      to: 'person',
      relation: 'links_to',
      confidence: Confidence.medium,
      via: 'username.enrich',
    ),
    const GraphEdge(
      from: 'user',
      to: 'lead1',
      relation: 'links_to',
      confidence: Confidence.low,
      via: 'search.dorks',
    ),
    const GraphEdge(
      from: 'user',
      to: 'lead2',
      relation: 'links_to',
      confidence: Confidence.low,
      via: 'search.dorks',
    ),
  ];
  return OsintGraph(nodes: nodes, edges: edges);
}

ScanSnapshot _doneSnapshot() {
  final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
  return ScanSnapshot(
    scanId: 'demo-case-1',
    target: const Target(
      kind: TargetKind.username,
      raw: 'jane.doe.1990',
      normalized: 'jane.doe.1990',
    ),
    targets: const [
      Target(
        kind: TargetKind.username,
        raw: 'jane.doe.1990',
        normalized: 'jane.doe.1990',
      ),
      Target(
        kind: TargetKind.email,
        raw: 'jane.doe.1990@proton.me',
        normalized: 'jane.doe.1990@proton.me',
      ),
    ],
    status: 'done',
    startedAt: now - 91,
    finishedAt: now,
    modules: const [
      ModuleProgress(
        id: 'username.enumerate',
        label: '1123 sites · 24 workers',
        done: 1123,
        total: 1123,
        status: 'done',
        found: 5,
      ),
      ModuleProgress(
        id: 'username.enrich',
        label: '5 profiles resolved',
        done: 5,
        total: 5,
        status: 'done',
        found: 3,
      ),
      ModuleProgress(
        id: 'email.recon',
        label: '1 address · MX · Gravatar',
        done: 5,
        total: 5,
        status: 'done',
        found: 1,
      ),
      ModuleProgress(
        id: 'search.dorks',
        label: '14 dorks built',
        done: 14,
        total: 14,
        status: 'done',
        found: 2,
      ),
    ],
    findings: _demoFindings(),
    graph: _demoGraph(),
    log: const [
      '12:03:01 scan started · 2 targets · proxy socks5h://127.0.0.1:9050',
      '12:03:01 module username.enumerate · 1123 sites · 24 workers',
      '12:03:04 hit github.com/jane-doe-1990 · 200 · marker matched',
      '12:03:05 hit reddit.com/user/jane_doe_1990 · 200 · marker matched',
      '12:03:09 hit mastodon.social/@jane_doe_1990 · 200 · soft marker',
      '12:03:14 hit news.ycombinator.com/user?id=janedoe1990 · 200',
      '12:03:18 hit keybase.io/janedoe1990 · 200 · nonce check pending',
      '12:03:31 username.enumerate done · 1123 checked · 5 hits · 1118 empty',
      '12:03:32 module username.enrich · resolving 5 profiles',
      '12:03:40 github api · Jane D. · Berlin · 412 followers',
      '12:03:47 gravatar resolved · jane.doe.1990@proton.me',
      '12:04:03 module email.recon · MX protonmail.net · not disposable',
      '12:04:22 module search.dorks · 14 dorks built',
      '12:04:32 scan complete · 10 findings · graph 11 nodes / 11 edges · 91s',
    ],
  );
}

ScanSnapshot _runningSnapshot() {
  final done = _doneSnapshot();
  return ScanSnapshot(
    scanId: done.scanId,
    target: done.target,
    targets: done.targets,
    status: 'running',
    startedAt: done.startedAt,
    modules: const [
      ModuleProgress(
        id: 'username.enumerate',
        label: '1123 sites · 24 workers',
        done: 1123,
        total: 1123,
        status: 'done',
        found: 5,
      ),
      ModuleProgress(
        id: 'username.enrich',
        label: 'resolving 5 profiles',
        done: 2,
        total: 5,
        status: 'running',
        found: 2,
      ),
      ModuleProgress(
        id: 'email.recon',
        label: 'MX · Gravatar',
        done: 2,
        total: 5,
        status: 'running',
        found: 1,
      ),
      ModuleProgress(
        id: 'search.dorks',
        label: 'queued',
        done: 0,
        total: 14,
        status: 'queued',
        found: 0,
      ),
    ],
    findings: _demoFindings().take(3).toList(),
    log: const [
      '12:03:01 scan started · 2 targets · proxy socks5h://127.0.0.1:9050',
      '12:03:01 module username.enumerate · 1123 sites · 24 workers',
      '12:03:04 hit github.com/jane-doe-1990 · 200 · marker matched',
      '12:03:05 hit reddit.com/user/jane_doe_1990 · 200 · marker matched',
      '12:03:09 hit mastodon.social/@jane_doe_1990 · 200 · soft marker',
      '12:03:14 hit news.ycombinator.com/user?id=janedoe1990 · 200',
      '12:03:18 hit keybase.io/janedoe1990 · 200 · nonce check pending',
      '12:03:31 username.enumerate done · 1123 checked · 5 hits · 1118 empty',
      '12:03:32 module username.enrich · resolving 5 profiles',
      '12:03:40 github api · Jane D. · Berlin · 412 followers',
      '12:03:47 gravatar resolved · jane.doe.1990@proton.me',
    ],
  );
}

List<CaseMeta> _demoCases() => [
  const CaseMeta(
    id: 'demo-case-1',
    title: 'jane.doe.1990',
    kind: TargetKind.username,
    createdAt: 1798764000,
    findingCount: 10,
  ),
  const CaseMeta(
    id: 'demo-case-2',
    title: 'evilcorp-assets.com',
    kind: TargetKind.domain,
    createdAt: 1798677600,
    findingCount: 23,
  ),
  const CaseMeta(
    id: 'demo-case-3',
    title: '91.219.237.22',
    kind: TargetKind.ipAddress,
    createdAt: 1798591200,
    findingCount: 8,
  ),
];

AppState _state({ScanSnapshot? running, ScanSnapshot? done}) {
  final s = AppState();
  s.nativeReady = true;
  s.modules = _demoModules();
  s.proxy = 'socks5h://127.0.0.1:9050';
  s.lastSnapshot = done;
  s.activeScan = running;
  s.caseList = _demoCases();
  return s;
}

Future<void> _pump(WidgetTester tester, AppState state, {int index = 0}) async {
  tester.view.physicalSize = const Size(2880, 1800);
  tester.view.devicePixelRatio = 2.0;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(AnywayApp(state: state, initialIndex: index));
  await tester.runAsync(() async {
    await Future<void>.delayed(const Duration(milliseconds: 250));
  });
  await tester.pump(const Duration(milliseconds: 450));
}

void main() {
  setUpAll(() async {
    await _loadFonts();
  });

  testWidgets('screenshot dashboard', (tester) async {
    await _pump(tester, _state());
    await tester.enterText(find.byType(TextField), 'jane.doe.1990');
    await tester.pump(const Duration(milliseconds: 300));
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/01_dashboard.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));

  testWidgets('screenshot scan running', (tester) async {
    await _pump(tester, _state(running: _runningSnapshot()), index: 1);
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/02_scan.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));

  testWidgets('screenshot results', (tester) async {
    await _pump(tester, _state(done: _doneSnapshot()), index: 2);
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/03_results.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));

  testWidgets('screenshot graph', (tester) async {
    await _pump(tester, _state(done: _doneSnapshot()), index: 3);
    await tester.tap(find.byIcon(Icons.center_focus_strong));
    await tester.pump(const Duration(milliseconds: 300));
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/04_graph.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));

  testWidgets('screenshot cases', (tester) async {
    await _pump(tester, _state(done: _doneSnapshot()), index: 4);
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/05_cases.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));

  testWidgets('screenshot modules', (tester) async {
    await _pump(tester, _state(), index: 5);
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/06_modules.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));

  testWidgets('screenshot settings', (tester) async {
    await _pump(tester, _state(), index: 6);
    await expectLater(
      find.byType(AnywayApp),
      matchesGoldenFile('goldens/07_settings.png'),
    );
  }, timeout: const Timeout(Duration(minutes: 3)));
}
