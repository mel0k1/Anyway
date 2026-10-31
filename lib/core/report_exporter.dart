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
        b.writeln('| ${f.label} | $value | ${f.confidence.label} | $url |');
      }
      b.writeln();
    }
    b.writeln(_graphSection(s.graph));
    return b.toString();
  }

  static String _graphSection(OsintGraph g) {
    if (g.nodes.isEmpty) return '';
    final b = StringBuffer();
    b.writeln('## Relationship graph');
    b.writeln();
    b.writeln('${g.nodes.length} nodes · ${g.edges.length} links');
    b.writeln();
    b.writeln('```mermaid');
    b.writeln('graph LR');
    final ids = <String, String>{};
    for (var i = 0; i < g.nodes.length; i++) {
      final n = g.nodes[i];
      final alias = 'n$i';
      ids[n.id] = alias;
      final title = n.label
          .replaceAll('"', "'")
          .replaceAll(RegExp(r'[\[\]{}()<>|\\]'), ' ')
          .trim();
      b.writeln('  $alias["${n.kind.title}: $title"]:::${n.confidence.name}');
    }
    const maxEdges = 250;
    for (final e in g.edges.take(maxEdges)) {
      final from = ids[e.from];
      final to = ids[e.to];
      if (from == null || to == null) continue;
      final label = e.confidence == Confidence.high
          ? e.relation
          : '${e.relation} · ${e.confidence.label.toLowerCase()}';
      b.writeln(
          '  $from -->|"${label.replaceAll('"', "'")}"| $to');
    }
    if (g.edges.length > maxEdges) {
      b.writeln('  note["... ${g.edges.length - maxEdges} more links omitted"]');
    }
    b.writeln('  classDef high fill:#1b5e20,stroke:#4caf50,color:#ffffff');
    b.writeln('  classDef medium fill:#5d4037,stroke:#ffb300,color:#ffffff');
    b.writeln('  classDef low fill:#37474f,stroke:#90a4ae,color:#ffffff');
    b.writeln('```');
    b.writeln();
    return b.toString();
  }

  static String jsonReport(ScanSnapshot s) => const JsonEncoder.withIndent('  ').convert(s.toJson());

  static String _esc(String v) => v
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');

  static String _confClass(Confidence c) => switch (c) {
        Confidence.high => 'high',
        Confidence.medium => 'medium',
        Confidence.low => 'low',
      };

  static String html(ScanSnapshot s) {
    final started = DateTime.fromMillisecondsSinceEpoch(s.startedAt * 1000).toUtc().toIso8601String();
    final finished = s.finishedAt == null
        ? '—'
        : DateTime.fromMillisecondsSinceEpoch(s.finishedAt! * 1000).toUtc().toIso8601String();
    final duration = s.finishedAt == null ? '—' : '${s.finishedAt! - s.startedAt}s';
    final generated = DateTime.now().toUtc().toIso8601String();

    final byModule = <String, List<Finding>>{};
    for (final f in s.findings) {
      byModule.putIfAbsent(f.moduleId, () => []).add(f);
    }

    final b = StringBuffer();
    b.writeln('<!DOCTYPE html>');
    b.writeln('<html lang="en">');
    b.writeln('<head>');
    b.writeln('<meta charset="utf-8">');
    b.writeln('<meta name="viewport" content="width=device-width, initial-scale=1">');
    b.writeln('<title>Anyway report — ${_esc(s.target.raw)}</title>');
    b.writeln('<style>');
    b.writeln(':root { color-scheme: dark; }');
    b.writeln('* { box-sizing: border-box; margin: 0; padding: 0; }');
    b.writeln('body { background: #131218; color: #e6e0e9; font: 15px/1.55 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; padding: 40px 20px 60px; }');
    b.writeln('.wrap { max-width: 1080px; margin: 0 auto; }');
    b.writeln('header h1 { font-size: 30px; font-weight: 800; letter-spacing: -0.5px; }');
    b.writeln('header .sub { color: #d0bcff; font-weight: 600; margin-bottom: 14px; }');
    b.writeln('.chips { display: flex; flex-wrap: wrap; gap: 8px; margin: 16px 0 8px; }');
    b.writeln('.chip { background: #211f26; border: 1px solid #36343b; border-radius: 999px; padding: 5px 14px; font-size: 13px; color: #cac4d0; }');
    b.writeln('.chip b { color: #e6e0e9; }');
    b.writeln('.meta { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; margin: 24px 0 32px; }');
    b.writeln('.meta .cell { background: #1d1b20; border-radius: 16px; padding: 14px 18px; border: 1px solid #2a2731; }');
    b.writeln('.meta .cell small { display: block; color: #938f99; text-transform: uppercase; letter-spacing: 0.8px; font-size: 11px; margin-bottom: 4px; }');
    b.writeln('h2 { font-size: 20px; font-weight: 800; margin: 34px 0 12px; padding-top: 18px; border-top: 1px solid #2a2731; }');
    b.writeln('h2 span { color: #938f99; font-weight: 500; font-size: 14px; margin-left: 8px; }');
    b.writeln('table { width: 100%; border-collapse: collapse; background: #1d1b20; border-radius: 16px; overflow: hidden; border: 1px solid #2a2731; }');
    b.writeln('th { text-align: left; font-size: 12px; text-transform: uppercase; letter-spacing: 0.8px; color: #938f99; padding: 12px 16px; border-bottom: 1px solid #2a2731; }');
    b.writeln('td { padding: 12px 16px; border-bottom: 1px solid #241f2b; vertical-align: top; word-break: break-word; }');
    b.writeln('tr:last-child td { border-bottom: none; }');
    b.writeln('td.lab { font-weight: 700; white-space: nowrap; }');
    b.writeln('a { color: #d0bcff; text-decoration: none; } a:hover { text-decoration: underline; }');
    b.writeln('.conf { display: inline-block; border-radius: 999px; padding: 2px 12px; font-size: 12px; font-weight: 700; }');
    b.writeln('.conf.high { background: #103c1c; color: #7ddb8a; }');
    b.writeln('.conf.medium { background: #402d0e; color: #f2b95c; }');
    b.writeln('.conf.low { background: #2a2f36; color: #9aa3ad; }');
    b.writeln('.empty { color: #938f99; padding: 24px 0; }');
    b.writeln('details { margin-top: 30px; }');
    b.writeln('summary { cursor: pointer; color: #d0bcff; font-weight: 600; }');
    b.writeln('pre { background: #1d1b20; border: 1px solid #2a2731; border-radius: 16px; padding: 16px; margin-top: 12px; overflow-x: auto; font: 12px/1.6 ui-monospace, "Cascadia Mono", Consolas, monospace; color: #b6aeC4; }');
    b.writeln('footer { margin-top: 44px; color: #938f99; font-size: 13px; }');
    b.writeln('footer a { color: #d0bcff; }');
    b.writeln('</style>');
    b.writeln('</head>');
    b.writeln('<body><div class="wrap">');
    b.writeln('<header>');
    b.writeln('<h1>Anyway</h1>');
    b.writeln('<div class="sub">Investigation report</div>');
    b.writeln('<div class="chips">');
    b.writeln('<span class="chip">Target <b>${_esc(s.target.raw)}</b></span>');
    b.writeln('<span class="chip">${_esc(s.target.kind.title)}</span>');
    b.writeln('<span class="chip">Status <b>${_esc(s.status)}</b></span>');
    b.writeln('<span class="chip">${s.findings.length} findings</span>');
    b.writeln('<span class="chip">${s.targets.length} target${s.targets.length == 1 ? '' : 's'}</span>');
    b.writeln('</div>');
    b.writeln('</header>');
    b.writeln('<div class="meta">');
    b.writeln('<div class="cell"><small>Started</small>${_esc(started)}</div>');
    b.writeln('<div class="cell"><small>Finished</small>${_esc(finished)}</div>');
    b.writeln('<div class="cell"><small>Duration</small>${_esc(duration)}</div>');
    b.writeln('<div class="cell"><small>Scan id</small>${_esc(s.scanId)}</div>');
    b.writeln('</div>');

    if (s.targets.length > 1) {
      b.writeln('<h2>Targets</h2>');
      b.writeln('<table><tr><th>Target</th><th>Type</th></tr>');
      for (final t in s.targets) {
        b.writeln('<tr><td class="lab">${_esc(t.raw)}</td><td>${_esc(t.kind.title)}</td></tr>');
      }
      b.writeln('</table>');
    }

    if (byModule.isEmpty) {
      b.writeln('<h2>Findings</h2><p class="empty">No findings recorded.</p>');
    }
    for (final entry in byModule.entries) {
      b.writeln('<h2>${_esc(entry.key)}<span>${entry.value.length} finding${entry.value.length == 1 ? '' : 's'}</span></h2>');
      b.writeln('<table><tr><th>Finding</th><th>Value</th><th>Confidence</th><th>Link</th></tr>');
      for (final f in entry.value) {
        final value = _esc(f.value).replaceAll('\n', '<br>');
        final link = f.url == null ? '' : '<a href="${_esc(f.url!)}">open</a>';
        final target = f.target == null ? '' : '<br><small style="color:#938f99">${_esc(f.target!)}</small>';
        b.writeln(
            '<tr><td class="lab">${_esc(f.label)}$target</td><td>$value</td><td><span class="conf ${_confClass(f.confidence)}">${_esc(f.confidence.label)}</span></td><td>$link</td></tr>');
      }
      b.writeln('</table>');
    }

    if (s.graph.nodes.isNotEmpty) {
      final labels = {for (final n in s.graph.nodes) n.id: n.label};
      b.writeln('<h2>Relationship graph<span>${s.graph.nodes.length} nodes · ${s.graph.edges.length} links</span></h2>');
      b.writeln('<table><tr><th>Entity</th><th>Type</th><th>Confidence</th><th>Evidence</th></tr>');
      for (final n in s.graph.nodes) {
        final name = n.root ? '★ ${n.label}' : n.label;
        b.writeln(
            '<tr><td class="lab">${_esc(name)}</td><td>${_esc(n.kind.title)}</td><td><span class="conf ${_confClass(n.confidence)}">${_esc(n.confidence.label)}</span></td><td>${_esc(n.sources.join(', '))}</td></tr>');
      }
      b.writeln('</table>');
      b.writeln('<table style="margin-top:12px"><tr><th>From</th><th>Relation</th><th>To</th><th>Confidence</th></tr>');
      for (final e in s.graph.edges.take(300)) {
        final from = labels[e.from] ?? e.from;
        final to = labels[e.to] ?? e.to;
        b.writeln(
            '<tr><td class="lab">${_esc(from)}</td><td>${_esc(e.relation)}</td><td>${_esc(to)}</td><td><span class="conf ${_confClass(e.confidence)}">${_esc(e.confidence.label)}</span></td></tr>');
      }
      if (s.graph.edges.length > 300) {
        b.writeln('<tr><td colspan="4" style="color:#938f99">… ${s.graph.edges.length - 300} more links omitted</td></tr>');
      }
      b.writeln('</table>');
    }

    b.writeln('<details><summary>Scan log (${s.log.length} lines)</summary><pre>${_esc(s.log.join('\n'))}</pre></details>');
    b.writeln('<footer>Generated by <a href="https://github.com/mel0k1/Anyway">Anyway</a> · ${_esc(generated)} UTC · GPL-3.0</footer>');
    b.writeln('</div></body></html>');
    return b.toString();
  }

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

  static Future<File> exportHtml(ScanSnapshot s, [Directory? target]) async {
    final dir = target ?? await defaultExportDir();
    final name = 'anyway-${s.target.normalized.isEmpty ? 'report' : s.target.normalized}-${s.startedAt}.html';
    final f = File('${dir.path}${Platform.pathSeparator}$name');
    await f.writeAsString(html(s));
    return f;
  }
}
