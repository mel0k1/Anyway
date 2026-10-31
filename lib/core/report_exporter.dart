import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

import 'models.dart';

class ReportExporter {
  static String markdown(ScanSnapshot s) {
    final b = StringBuffer();
    b.writeln('# Anyway report: ${s.target.raw}');
    b.writeln();
    b.writeln('- Target type: ${s.target.kind.title}');
    b.writeln('- Status: ${s.status}');
    b.writeln('- Started: ${DateTime.fromMillisecondsSinceEpoch(s.startedAt * 1000).toUtc().toIso8601String()}');
    if (s.finishedAt != null) {
      b.writeln('- Finished: ${DateTime.fromMillisecondsSinceEpoch(s.finishedAt! * 1000).toUtc().toIso8601String()}');
      final dur = s.finishedAt! - s.startedAt;
      b.writeln('- Duration: ${dur}s');
    }
    b.writeln('- Findings: ${s.findings.length}');
    b.writeln();

    final byModule = <String, List<Finding>>{};
    for (final f in s.findings) {
      byModule.putIfAbsent(f.moduleId, () => []).add(f);
    }
    for (final entry in byModule.entries) {
      b.writeln('## ${entry.key}');
      b.writeln();
      b.writeln('| Label | Value | Confidence | URL |');
      b.writeln('| --- | --- | --- | --- |');
      for (final f in entry.value) {
        final value = f.value.replaceAll('|', '\\|').replaceAll('\n', ' ');
        final url = (f.url ?? '').replaceAll('|', '\\|');
        b.writeln('| ${f.label} | $value | ${f.confidence.name} | $url |');
      }
      b.writeln();
    }
    return b.toString();
  }

  static String jsonReport(ScanSnapshot s) => const JsonEncoder.withIndent('  ').convert(s.toJson());

  static Future<Directory> defaultExportDir() async {
    final base = await getApplicationDocumentsDirectory();
    final d = Directory('${base.path}${Platform.pathSeparator}anyway-exports');
    if (!await d.exists()) await d.create(recursive: true);
    return d;
  }

  static Future<File> exportMarkdown(ScanSnapshot s, [Directory? target]) async {
    final dir = target ?? await defaultExportDir();
    final name = 'anyway-${s.target.normalized.isEmpty ? 'report' : s.target.normalized}-${s.startedAt}.md';
    final f = File('${dir.path}${Platform.pathSeparator}$name');
    await f.writeAsString(markdown(s));
    return f;
  }

  static Future<File> exportJson(ScanSnapshot s, [Directory? target]) async {
    final dir = target ?? await defaultExportDir();
    final name = 'anyway-${s.target.normalized.isEmpty ? 'report' : s.target.normalized}-${s.startedAt}.json';
    final f = File('${dir.path}${Platform.pathSeparator}$name');
    await f.writeAsString(jsonReport(s));
    return f;
  }
}
