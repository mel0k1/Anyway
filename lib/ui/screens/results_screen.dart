import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';
import '../widgets/finding_card.dart';

class ResultsScreen extends StatefulWidget {
  final AppState state;
  final void Function()? onOpenGraph;

  const ResultsScreen({super.key, required this.state, this.onOpenGraph});

  @override
  State<ResultsScreen> createState() => _ResultsScreenState();
}

enum _ExportFormat { markdown, html, json }

class _ResultsScreenState extends State<ResultsScreen> {
  String? _moduleFilter;
  Confidence? _confidenceFilter;
  String _search = '';

  @override
  Widget build(BuildContext context) {
    final snap = widget.state.lastSnapshot ?? widget.state.activeScan;
    final scheme = Theme.of(context).colorScheme;

    if (snap == null || snap.findings.isEmpty && !snap.running) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.fact_check, size: 72, color: scheme.outlineVariant),
            const SizedBox(height: 16),
            Text('No results yet', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 8),
            Text('Run a scan or open a case', style: TextStyle(color: scheme.onSurfaceVariant)),
          ],
        ),
      );
    }

    final moduleIds = snap.findings.map((f) => f.moduleId).toSet().toList();
    final filtered = snap.findings.where((f) {
      if (_moduleFilter != null && f.moduleId != _moduleFilter) return false;
      if (_confidenceFilter != null && f.confidence != _confidenceFilter) return false;
      if (_search.isNotEmpty && !(f.label.toLowerCase().contains(_search) || f.value.toLowerCase().contains(_search))) {
        return false;
      }
      return true;
    }).toList();

    final grouped = <String, List<Finding>>{};
    for (final f in filtered) {
      grouped.putIfAbsent(f.moduleId, () => []).add(f);
    }

    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Results · ${snap.target.raw}', style: Theme.of(context).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
                    const SizedBox(height: 6),
                    Text('${filtered.length} of ${snap.findings.length} findings · target: ${snap.target.kind.title}', style: TextStyle(color: scheme.onSurfaceVariant)),
                  ],
                ),
              ),
              OutlinedButton.icon(
                onPressed: () => _export(context, _ExportFormat.markdown),
                icon: const Icon(Icons.description),
                label: const Text('Markdown'),
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                onPressed: () => _export(context, _ExportFormat.html),
                icon: const Icon(Icons.web),
                label: const Text('HTML'),
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                onPressed: () => _export(context, _ExportFormat.json),
                icon: const Icon(Icons.data_object),
                label: const Text('JSON'),
              ),
              const SizedBox(width: 12),
              if (widget.onOpenGraph != null)
                FilledButton.tonalIcon(
                  onPressed: widget.onOpenGraph,
                  icon: const Icon(Icons.hub),
                  label: const Text('Graph'),
                ),
            ],
          ),
          const SizedBox(height: 16),
          Row(
            children: [
              SizedBox(
                width: 320,
                child: TextField(
                  onChanged: (v) => setState(() => _search = v.toLowerCase()),
                  decoration: const InputDecoration(hintText: 'Filter findings', prefixIcon: Icon(Icons.filter_list, size: 20)),
                ),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Wrap(
                  spacing: 8,
                  children: [
                    ChoiceChip(
                      label: const Text('All'),
                      selected: _moduleFilter == null,
                      onSelected: (_) => setState(() => _moduleFilter = null),
                    ),
                    for (final id in moduleIds)
                      ChoiceChip(
                        label: Text(id),
                        selected: _moduleFilter == id,
                        onSelected: (_) => setState(() => _moduleFilter = id),
                      ),
                  ],
                ),
              ),
              PopupMenuButton<Confidence?>(
                initialValue: _confidenceFilter,
                onSelected: (v) => setState(() => _confidenceFilter = v),
                itemBuilder: (context) => [
                  const PopupMenuItem(value: null, child: Text('Any confidence')),
                  for (final c in Confidence.values)
                    PopupMenuItem(value: c, child: Text(c.label)),
                ],
                child: Chip(
                  avatar: Icon(Icons.verified, size: 16, color: scheme.primary),
                  label: Text(_confidenceFilter?.label ?? 'confidence'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          Expanded(
            child: filtered.isEmpty
                ? Center(child: Text('Nothing matches the filter', style: TextStyle(color: scheme.onSurfaceVariant)))
                : ListView(
                    children: [
                      for (final entry in grouped.entries) ...[
                        Padding(
                          padding: const EdgeInsets.only(top: 12, bottom: 8),
                          child: Row(
                            children: [
                              Text(entry.key.toUpperCase(), style: TextStyle(fontSize: 12, fontWeight: FontWeight.w800, letterSpacing: 1, color: scheme.primary)),
                              const SizedBox(width: 8),
                              Expanded(child: Divider(color: scheme.outlineVariant)),
                            ],
                          ),
                        ),
                        for (final f in entry.value)
                          Padding(
                            padding: const EdgeInsets.only(bottom: 10),
                            child: FindingCard(finding: f),
                          ),
                      ],
                    ],
                  ),
          ),
        ],
      ),
    );
  }

  Future<void> _export(BuildContext context, _ExportFormat format) async {
    final messenger = ScaffoldMessenger.of(context);
    try {
      final dirPath = await getDirectoryPath(confirmButtonText: 'Export here');
      final dir = dirPath == null ? null : Directory(dirPath);
      final file = switch (format) {
        _ExportFormat.markdown => await widget.state.exportLastMarkdown(dir),
        _ExportFormat.json => await widget.state.exportLastJson(dir),
        _ExportFormat.html => await widget.state.exportLastHtml(dir),
      };
      messenger.showSnackBar(SnackBar(content: Text('Saved: ${file.path}')));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('Export failed: $e')));
    }
  }
}
