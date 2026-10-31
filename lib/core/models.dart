import 'dart:io';

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
  final Map<String, dynamic>? detail;

  const Finding({
    required this.moduleId,
    required this.category,
    required this.label,
    required this.value,
    this.url,
    this.confidence = Confidence.medium,
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
        detail: j['detail'] is Map<String, dynamic> ? j['detail'] as Map<String, dynamic> : null,
      );

  Map<String, dynamic> toJson() => {
        'module_id': moduleId,
        'category': category,
        'label': label,
        'value': value,
        'url': url,
        'confidence': confidence.name,
        'detail': detail,
      };
}

class ModuleInfo {
  final String id;
  final String name;
  final String description;
  final List<TargetKind> accepts;

  const ModuleInfo({required this.id, required this.name, required this.description, required this.accepts});

  factory ModuleInfo.fromJson(Map<String, dynamic> j) => ModuleInfo(
        id: j['id'] as String? ?? '',
        name: j['name'] as String? ?? '',
        description: j['description'] as String? ?? '',
        accepts: ((j['accepts'] as List?) ?? const [])
            .map((e) => TargetKind.fromName(e as String))
            .toList(),
      );
}

class ModuleProgress {
  final String id;
  final int done;
  final int total;
  final String status;
  final int found;

  const ModuleProgress({required this.id, required this.done, required this.total, required this.status, required this.found});

  factory ModuleProgress.fromJson(Map<String, dynamic> j) => ModuleProgress(
        id: j['id'] as String? ?? '',
        done: (j['done'] as num?)?.toInt() ?? 0,
        total: (j['total'] as num?)?.toInt() ?? 0,
        status: j['status'] as String? ?? '',
        found: (j['found'] as num?)?.toInt() ?? 0,
      );
}

class ScanSnapshot {
  final String scanId;
  final Target target;
  final String status;
  final int startedAt;
  final int? finishedAt;
  final List<ModuleProgress> modules;
  final List<Finding> findings;
  final List<String> log;

  const ScanSnapshot({
    required this.scanId,
    required this.target,
    required this.status,
    required this.startedAt,
    this.finishedAt,
    required this.modules,
    required this.findings,
    required this.log,
  });

  bool get running => status == 'running';

  factory ScanSnapshot.fromJson(Map<String, dynamic> j) => ScanSnapshot(
        scanId: j['scanId'] as String? ?? '',
        target: Target.fromJson((j['target'] as Map?)?.cast<String, dynamic>() ?? const {}),
        status: j['status'] as String? ?? 'unknown',
        startedAt: (j['startedAt'] as num?)?.toInt() ?? 0,
        finishedAt: (j['finishedAt'] as num?)?.toInt(),
        modules: ((j['modules'] as List?) ?? const [])
            .map((e) => ModuleProgress.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        findings: ((j['findings'] as List?) ?? const [])
            .map((e) => Finding.fromJson((e as Map).cast<String, dynamic>()))
            .toList(),
        log: ((j['log'] as List?) ?? const []).map((e) => e as String).toList(),
      );

  Map<String, dynamic> toJson() => {
        'scanId': scanId,
        'target': {'kind': target.kind.wire, 'raw': target.raw, 'normalized': target.normalized},
        'status': status,
        'startedAt': startedAt,
        'finishedAt': finishedAt,
        'modules': modules
            .map((m) => {'id': m.id, 'done': m.done, 'total': m.total, 'status': m.status, 'found': m.found})
            .toList(),
        'findings': findings.map((f) => f.toJson()).toList(),
        'log': log,
      };
}

TargetKind detectTargetKind(String query) {
  final q = query.trim().replaceFirst(RegExp(r'^@+'), '');
  if (q.isEmpty) return TargetKind.unknown;
  final email = RegExp(r'^[A-Za-z0-9._+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$');
  final domain = RegExp(r'^(?=.{4,253}$)[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)+$');
  final phone = RegExp(r'^\+?[0-9 ()\-.]{7,16}$');
  if (InternetAddress.tryParse(q) != null) return TargetKind.ipAddress;
  if (email.hasMatch(q)) return TargetKind.email;
  final digits = q.replaceAll(RegExp(r'[^0-9]'), '');
  if (phone.hasMatch(q) && digits.length >= 7) return TargetKind.phoneNumber;
  if (domain.hasMatch(q)) return TargetKind.domain;
  if (RegExp(r'^[A-Za-z0-9._-]{1,64}$').hasMatch(q)) return TargetKind.username;
  return TargetKind.unknown;
}
