import 'dart:io';

import 'package:flutter/material.dart';

enum TargetKind {
  username,
  email,
  ipAddress,
  domain,
  phoneNumber,
  file,
  unknown;

  static TargetKind fromName(String name) {
    for (final k in values) {
      if (k.wire == name) return k;
    }
    return unknown;
  }

  String get wire => switch (this) {
        ipAddress => 'ip_address',
        phoneNumber => 'phone_number',
        _ => name,
      };

  String get title => switch (this) {
        username => 'Username',
        email => 'Email',
        ipAddress => 'IP address',
        domain => 'Domain',
        phoneNumber => 'Phone',
        file => 'File',
        unknown => 'Unknown',
      };
}

enum Confidence { high, medium, low }

extension ConfidenceX on Confidence {
  String get label => switch (this) {
        Confidence.high => 'Confirmed',
        Confidence.medium => 'Questionable',
        Confidence.low => 'Unverified',
      };

  static Confidence fromName(String? name) => Confidence.values.firstWhere(
        (c) => c.name == name,
        orElse: () => Confidence.medium,
      );
}

enum GraphNodeKind {
  username,
  email,
  ip,
  domain,
  phone,
  file,
  platform,
  org,
  country,
  location,
  device,
  person,
  provider,
  infra,
  lead,
  other;

  static GraphNodeKind fromName(String? name) => GraphNodeKind.values.firstWhere(
        (k) => k.name == name,
        orElse: () => GraphNodeKind.other,
      );

  String get title => switch (this) {
        username => 'Username',
        email => 'Email',
        ip => 'IP address',
        domain => 'Domain',
        phone => 'Phone',
        file => 'File',
        platform => 'Platform',
        org => 'Organization',
        country => 'Country',
        location => 'Location',
        device => 'Device',
        person => 'Person',
        provider => 'Mail provider',
        infra => 'Infrastructure',
        lead => 'Search lead',
        other => 'Entity',
      };

  IconData get icon => switch (this) {
        username => Icons.person,
        email => Icons.mail,
        ip => Icons.public,
        domain => Icons.dns,
        phone => Icons.call,
        file => Icons.image,
        platform => Icons.public,
        org => Icons.corporate_fare,
        country => Icons.flag,
        location => Icons.place,
        device => Icons.camera_alt,
        person => Icons.face,
        provider => Icons.alternate_email,
        infra => Icons.lan,
        lead => Icons.travel_explore,
        other => Icons.circle,
      };
}

class GraphNode {
  final String id;
  final GraphNodeKind kind;
  final String label;
  final Confidence confidence;
  final List<String> sources;
  final bool root;

  const GraphNode({
    required this.id,
    required this.kind,
    required this.label,
    required this.confidence,
    this.sources = const [],
    this.root = false,
  });

  factory GraphNode.fromJson(Map<String, dynamic> j) => GraphNode(
        id: j['id'] as String? ?? '',
        kind: GraphNodeKind.fromName(j['kind'] as String?),
        label: j['label'] as String? ?? '',
        confidence: ConfidenceX.fromName(j['confidence'] as String?),
        sources: ((j['sources'] as List?) ?? const [])
            .map((e) => e as String)
            .toList(),
        root: j['root'] as bool? ?? false,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'kind': kind.name,
        'label': label,
        'confidence': confidence.name,
        'sources': sources,
        'root': root,
      };
}

class GraphEdge {
  final String from;
  final String to;
  final String relation;
  final Confidence confidence;
  final String via;

  const GraphEdge({
    required this.from,
    required this.to,
    required this.relation,
    required this.confidence,
    this.via = '',
  });

  factory GraphEdge.fromJson(Map<String, dynamic> j) => GraphEdge(
        from: j['from'] as String? ?? '',
        to: j['to'] as String? ?? '',
        relation: j['relation'] as String? ?? '',
        confidence: ConfidenceX.fromName(j['confidence'] as String?),
        via: j['via'] as String? ?? '',
      );

  Map<String, dynamic> toJson() => {
        'from': from,
        'to': to,
        'relation': relation,
        'confidence': confidence.name,
        'via': via,
      };
}

class OsintGraph {
  final List<GraphNode> nodes;
  final List<GraphEdge> edges;

  const OsintGraph({this.nodes = const [], this.edges = const []});

  static const empty = OsintGraph();

  int get nodeCount => nodes.length;

  int get edgeCount => edges.length;

  factory OsintGraph.fromJson(Map<String, dynamic> j) => OsintGraph(
        nodes: ((j['nodes'] as List?) ?? const [])
            .map((e) => GraphNode.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        edges: ((j['edges'] as List?) ?? const [])
            .map((e) => GraphEdge.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
      );

  Map<String, dynamic> toJson() => {
        'nodes': nodes.map((n) => n.toJson()).toList(),
        'edges': edges.map((e) => e.toJson()).toList(),
      };
}

class Target {
  final TargetKind kind;
  final String raw;
  final String normalized;

  const Target({required this.kind, required this.raw, required this.normalized});

  factory Target.fromJson(Map<String, dynamic> j) => Target(
        kind: TargetKind.fromName(j['kind'] as String? ?? 'unknown'),
        raw: j['raw'] as String? ?? '',
        normalized: j['normalized'] as String? ?? '',
      );
}

class Finding {
  final String moduleId;
  final String category;
  final String label;
  final String value;
  final String? url;
  final Confidence confidence;
  final String? target;
  final Map<String, dynamic>? detail;

  const Finding({
    required this.moduleId,
    required this.category,
    required this.label,
    required this.value,
    this.url,
    this.confidence = Confidence.medium,
    this.target,
    this.detail,
  });

  factory Finding.fromJson(Map<String, dynamic> j) => Finding(
        moduleId: j['module_id'] as String? ?? '',
        category: j['category'] as String? ?? '',
        label: j['label'] as String? ?? '',
        value: j['value'] as String? ?? '',
        url: j['url'] as String?,
        confidence: Confidence.values.firstWhere(
          (c) => c.name == (j['confidence'] as String? ?? 'medium'),
          orElse: () => Confidence.medium,
        ),
        target: j['target'] as String?,
        detail: j['detail'] is Map<String, dynamic> ? j['detail'] as Map<String, dynamic> : null,
      );

  Map<String, dynamic> toJson() => {
        'module_id': moduleId,
        'category': category,
        'label': label,
        'value': value,
        'url': url,
        'confidence': confidence.name,
        'target': target,
        'detail': detail,
      };
}

class CategoryInfo {
  final String name;
  final int count;

  const CategoryInfo({required this.name, required this.count});

  factory CategoryInfo.fromJson(Map<String, dynamic> j) => CategoryInfo(
        name: j['name'] as String? ?? '',
        count: (j['count'] as num?)?.toInt() ?? 0,
      );
}

class ModuleInfo {
  final String id;
  final String name;
  final String description;
  final List<TargetKind> accepts;
  final int? sites;
  final int? coreSites;

  const ModuleInfo({
    required this.id,
    required this.name,
    required this.description,
    required this.accepts,
    this.sites,
    this.coreSites,
  });

  factory ModuleInfo.fromJson(Map<String, dynamic> j) => ModuleInfo(
        id: j['id'] as String? ?? '',
        name: j['name'] as String? ?? '',
        description: j['description'] as String? ?? '',
        accepts: ((j['accepts'] as List?) ?? const [])
            .map((e) => TargetKind.fromName(e as String))
            .toList(),
        sites: (j['sites'] as num?)?.toInt(),
        coreSites: (j['coreSites'] as num?)?.toInt(),
      );
}

class ModuleProgress {
  final String id;
  final String label;
  final int done;
  final int total;
  final String status;
  final int found;

  const ModuleProgress({
    required this.id,
    this.label = '',
    required this.done,
    required this.total,
    required this.status,
    required this.found,
  });

  factory ModuleProgress.fromJson(Map<String, dynamic> j) => ModuleProgress(
        id: j['id'] as String? ?? '',
        label: j['label'] as String? ?? '',
        done: (j['done'] as num?)?.toInt() ?? 0,
        total: (j['total'] as num?)?.toInt() ?? 0,
        status: j['status'] as String? ?? '',
        found: (j['found'] as num?)?.toInt() ?? 0,
      );
}

class PivotLink {
  final String parent;
  final TargetKind parentKind;
  final Target child;
  final int depth;
  final String via;

  const PivotLink({
    required this.parent,
    required this.parentKind,
    required this.child,
    required this.depth,
    required this.via,
  });

  factory PivotLink.fromJson(Map<String, dynamic> j) => PivotLink(
        parent: j['parent'] as String? ?? '',
        parentKind: TargetKind.fromName(j['parent_kind'] as String? ?? 'unknown'),
        child: Target.fromJson((j['child'] as Map?)?.cast<String, dynamic>() ?? const {}),
        depth: (j['depth'] as num?)?.toInt() ?? 0,
        via: j['via'] as String? ?? '',
      );
}

class ScanSnapshot {
  final String scanId;
  final Target target;
  final List<Target> targets;
  final String status;
  final int startedAt;
  final int? finishedAt;
  final List<ModuleProgress> modules;
  final List<Finding> findings;
  final OsintGraph graph;
  final List<PivotLink> pivots;
  final List<String> log;

  const ScanSnapshot({
    required this.scanId,
    required this.target,
    this.targets = const [],
    required this.status,
    required this.startedAt,
    this.finishedAt,
    required this.modules,
    required this.findings,
    this.graph = OsintGraph.empty,
    this.pivots = const [],
    required this.log,
  });

  bool get running => status == 'running';

  bool get multiTarget => targets.length > 1;

  factory ScanSnapshot.fromJson(Map<String, dynamic> j) => ScanSnapshot(
        scanId: j['scanId'] as String? ?? '',
        target: Target.fromJson((j['target'] as Map?)?.cast<String, dynamic>() ?? const {}),
        targets: ((j['targets'] as List?) ?? const [])
            .map((e) => Target.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        status: j['status'] as String? ?? 'unknown',
        startedAt: (j['startedAt'] as num?)?.toInt() ?? 0,
        finishedAt: (j['finishedAt'] as num?)?.toInt(),
        modules: ((j['modules'] as List?) ?? const [])
            .map((e) => ModuleProgress.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        findings: ((j['findings'] as List?) ?? const [])
            .map((e) => Finding.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        graph: j['graph'] is Map
            ? OsintGraph.fromJson((j['graph'] as Map).cast<String, dynamic>())
            : OsintGraph.empty,
        pivots: ((j['pivots'] as List?) ?? const [])
            .map((e) => PivotLink.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        log: ((j['log'] as List?) ?? const []).map((e) => e as String).toList(),
      );

  Map<String, dynamic> toJson() => {
        'scanId': scanId,
        'target': {'kind': target.kind.wire, 'raw': target.raw, 'normalized': target.normalized},
        'targets': targets.map((t) => {'kind': t.kind.wire, 'raw': t.raw, 'normalized': t.normalized}).toList(),
        'status': status,
        'startedAt': startedAt,
        'finishedAt': finishedAt,
        'modules': modules
            .map((m) => {'id': m.id, 'label': m.label, 'done': m.done, 'total': m.total, 'status': m.status, 'found': m.found})
            .toList(),
        'findings': findings.map((f) => f.toJson()).toList(),
        'graph': graph.toJson(),
        'pivots': pivots
            .map((p) => {
                  'parent': p.parent,
                  'parent_kind': p.parentKind.wire,
                  'child': {'kind': p.child.kind.wire, 'raw': p.child.raw, 'normalized': p.child.normalized},
                  'depth': p.depth,
                  'via': p.via,
                })
            .toList(),
        'log': log,
      };
}

TargetKind detectTargetKind(String query) {
  final first = query.trim().split(RegExp(r'[,;\n]')).first.trim();
  final q = first.replaceFirst(RegExp(r'^@+'), '');
  if (q.isEmpty) return TargetKind.unknown;
  final email = RegExp(r'^[A-Za-z0-9._+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$');
  final domain = RegExp(r'^(?=.{4,253}$)[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}$');
  final phone = RegExp(r'^\+?[0-9 ()\-.]{7,16}$');
  if (InternetAddress.tryParse(q) != null) return TargetKind.ipAddress;
  if (email.hasMatch(q)) return TargetKind.email;
  final digits = q.replaceAll(RegExp(r'[^0-9]'), '');
  if (phone.hasMatch(q) && digits.length >= 7) return TargetKind.phoneNumber;
  if (domain.hasMatch(q)) return TargetKind.domain;
  if (RegExp(r'^[A-Za-z0-9._-]{1,64}$').hasMatch(q)) return TargetKind.username;
  return TargetKind.unknown;
}

class ScheduleEntryInfo {
  final String id;
  final String query;
  final int intervalSecs;
  final bool enabled;
  final bool running;
  final int runs;
  final int? lastRun;
  final int nextRun;
  final int dueIn;
  final String? lastStatus;
  final int? newFindings;
  final List<String> addedFindings;
  final int findingsTotal;

  const ScheduleEntryInfo({
    required this.id,
    required this.query,
    required this.intervalSecs,
    required this.enabled,
    required this.running,
    required this.runs,
    required this.nextRun,
    required this.dueIn,
    required this.findingsTotal,
    this.lastRun,
    this.lastStatus,
    this.newFindings,
    this.addedFindings = const <String>[],
  });

  factory ScheduleEntryInfo.fromJson(Map<String, dynamic> j) => ScheduleEntryInfo(
        id: j['id'] as String? ?? '',
        query: j['query'] as String? ?? '',
        intervalSecs: (j['interval_secs'] as num?)?.toInt() ?? 3600,
        enabled: j['enabled'] as bool? ?? false,
        running: j['running'] as bool? ?? false,
        runs: (j['runs'] as num?)?.toInt() ?? 0,
        lastRun: (j['last_run'] as num?)?.toInt(),
        nextRun: (j['next_run'] as num?)?.toInt() ?? 0,
        dueIn: (j['due_in'] as num?)?.toInt() ?? 0,
        lastStatus: j['last_status'] as String?,
        newFindings: (j['new_findings'] as num?)?.toInt(),
        addedFindings: ((j['added_findings'] as List?) ?? const [])
            .map((e) => e as String)
            .toList(),
        findingsTotal: (j['findings_total'] as num?)?.toInt() ?? 0,
      );

  Map<String, dynamic> toPersisted() => {
        'id': id,
        'query': query,
        'interval_secs': intervalSecs,
        'enabled': enabled,
      };
}

class TorStatus {
  final bool listening;
  final bool? isTor;
  final String? exitIp;
  final String proxy;
  final String mode;

  const TorStatus({
    required this.listening,
    required this.proxy,
    required this.mode,
    this.isTor,
    this.exitIp,
  });

  factory TorStatus.fromJson(Map<String, dynamic> j) => TorStatus(
        listening: j['listening'] as bool? ?? false,
        isTor: j['isTor'] as bool?,
        exitIp: j['exitIp'] as String?,
        proxy: j['proxy'] as String? ?? '',
        mode: j['mode'] as String? ?? 'auto',
      );
}

class ImageHashInfo {
  final String ahash;
  final String dhash;
  final String phash;
  final int width;
  final int height;
  final String format;
  final Map<String, dynamic>? ocr;

  const ImageHashInfo({
    required this.ahash,
    required this.dhash,
    required this.phash,
    required this.width,
    required this.height,
    required this.format,
    this.ocr,
  });

  factory ImageHashInfo.fromJson(Map<String, dynamic> j) => ImageHashInfo(
        ahash: j['ahash'] as String? ?? '',
        dhash: j['dhash'] as String? ?? '',
        phash: j['phash'] as String? ?? '',
        width: (j['width'] as num?)?.toInt() ?? 0,
        height: (j['height'] as num?)?.toInt() ?? 0,
        format: j['format'] as String? ?? '',
        ocr: j['ocr'] is Map<String, dynamic> ? j['ocr'] as Map<String, dynamic> : null,
      );

  Map<String, dynamic> toJson() => {
        'ahash': ahash,
        'dhash': dhash,
        'phash': phash,
        'width': width,
        'height': height,
        'format': format,
      };
}

class TimelineEventInfo {
  final int? ts;
  final String date;
  final String label;
  final String kind;
  final String source;
  final String url;

  const TimelineEventInfo({
    required this.date,
    required this.label,
    required this.kind,
    required this.source,
    required this.url,
    this.ts,
  });

  factory TimelineEventInfo.fromJson(Map<String, dynamic> j) => TimelineEventInfo(
        ts: (j['ts'] as num?)?.toInt(),
        date: j['date'] as String? ?? '',
        label: j['label'] as String? ?? '',
        kind: j['kind'] as String? ?? '',
        source: j['source'] as String? ?? '',
        url: j['url'] as String? ?? '',
      );
}

class LeakRecord {
  final String source;
  final String email;
  final String username;
  final String phone;
  final String password;
  final String ip;
  final String domain;
  final String hash;
  final String extra;
  final int line;

  const LeakRecord({
    required this.source,
    required this.email,
    required this.username,
    required this.phone,
    required this.password,
    required this.ip,
    required this.domain,
    required this.hash,
    required this.extra,
    required this.line,
  });

  factory LeakRecord.fromJson(Map<String, dynamic> j) => LeakRecord(
        source: j['source'] as String? ?? '',
        email: j['email'] as String? ?? '',
        username: j['username'] as String? ?? '',
        phone: j['phone'] as String? ?? '',
        password: j['password'] as String? ?? '',
        ip: j['ip'] as String? ?? '',
        domain: j['domain'] as String? ?? '',
        hash: j['hash'] as String? ?? '',
        extra: j['extra'] as String? ?? '',
        line: (j['line'] as num?)?.toInt() ?? 0,
      );
}
