import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

import 'models.dart';

class CaseMeta {
  final String id;
  final String title;
  final TargetKind kind;
  final int createdAt;
  final int findingCount;

  const CaseMeta({required this.id, required this.title, required this.kind, required this.createdAt, required this.findingCount});

  factory CaseMeta.fromJson(Map<String, dynamic> j) => CaseMeta(
        id: j['id'] as String,
        title: j['title'] as String,
        kind: TargetKind.fromName(j['kind'] as String? ?? 'unknown'),
        createdAt: (j['createdAt'] as num?)?.toInt() ?? 0,
        findingCount: (j['findingCount'] as num?)?.toInt() ?? 0,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'title': title,
        'kind': kind.wire,
        'createdAt': createdAt,
        'findingCount': findingCount,
      };
}

class CaseStore {
  Directory? _dir;

  Future<Directory> get dir async {
    if (_dir != null) return _dir!;
    final base = await getApplicationSupportDirectory();
    final d = Directory('${base.path}${Platform.pathSeparator}cases');
    if (!await d.exists()) await d.create(recursive: true);
    _dir = d;
    return d;
  }

  Future<CaseMeta> save(ScanSnapshot snapshot) async {
    final d = await dir;
    final id = 'case-${snapshot.startedAt}-${snapshot.target.normalized.isEmpty ? 'query' : snapshot.target.normalized}';
    final meta = CaseMeta(
      id: id,
      title: snapshot.target.raw,
      kind: snapshot.target.kind,
      createdAt: snapshot.startedAt,
      findingCount: snapshot.findings.length,
    );
    final file = File('${d.path}${Platform.pathSeparator}$id.json');
    await file.writeAsString(jsonEncode({
      'meta': meta.toJson(),
      'snapshot': snapshot.toJson(),
    }));
    return meta;
  }

  Future<List<CaseMeta>> list() async {
    final d = await dir;
    final metas = <CaseMeta>[];
    await for (final e in d.list()) {
      if (e is! File || !e.path.endsWith('.json')) continue;
      try {
        final j = jsonDecode(await e.readAsString()) as Map<String, dynamic>;
        metas.add(CaseMeta.fromJson((j['meta'] as Map).cast<String, dynamic>()));
      } catch (_) {
        continue;
      }
    }
    metas.sort((a, b) => b.createdAt.compareTo(a.createdAt));
    return metas;
  }

  Future<ScanSnapshot?> load(String id) async {
    final d = await dir;
    final file = File('${d.path}${Platform.pathSeparator}$id.json');
    if (!await file.exists()) return null;
    try {
      final j = jsonDecode(await file.readAsString()) as Map<String, dynamic>;
      return ScanSnapshot.fromJson((j['snapshot'] as Map).cast<String, dynamic>());
    } catch (_) {
      return null;
    }
  }

  Future<void> delete(String id) async {
    final d = await dir;
    final file = File('${d.path}${Platform.pathSeparator}$id.json');
    if (await file.exists()) await file.delete();
  }
}
