import 'package:flutter/material.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';
import '../widgets/wavy_progress.dart';

class ScanScreen extends StatelessWidget {
  final AppState state;
  final void Function() onOpenResults;

  const ScanScreen({super.key, required this.state, required this.onOpenResults});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final snap = state.activeScan;

    if (snap == null) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.radar, size: 72, color: scheme.outlineVariant),
            const SizedBox(height: 16),
            Text('No active scan', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 8),
            Text('Start one from the dashboard', style: TextStyle(color: scheme.onSurfaceVariant)),
          ],
        ),
      );
    }

    final elapsed = ((snap.finishedAt ?? DateTime.now().millisecondsSinceEpoch ~/ 1000) - snap.startedAt).clamp(0, 1 << 30);

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
                    Row(
                      children: [
                        Chip(label: Text(snap.target.kind.title)),
                        const SizedBox(width: 8),
                        Expanded(child: Text(snap.target.raw, style: Theme.of(context).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800), overflow: TextOverflow.ellipsis)),
                      ],
                    ),
                    const SizedBox(height: 8),
                    Row(
                      children: [
                        _StatusPill(status: snap.status),
                        const SizedBox(width: 12),
                        Text('${elapsed}s', style: TextStyle(color: scheme.onSurfaceVariant)),
                        const SizedBox(width: 12),
                        Text('${snap.findings.length} findings', style: TextStyle(color: scheme.onSurfaceVariant)),
                      ],
                    ),
                  ],
                ),
              ),
              if (snap.running)
                OutlinedButton.icon(
                  onPressed: state.cancelScan,
                  icon: const Icon(Icons.stop),
                  label: const Text('Cancel'),
                ),
              if (!snap.running) ...[
                const SizedBox(width: 12),
                FilledButton.icon(onPressed: onOpenResults, icon: const Icon(Icons.fact_check), label: const Text('Results')),
              ],
            ],
          ),
          const SizedBox(height: 24),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 5,
                  child: Card(
                    child: ListView(
                      padding: const EdgeInsets.all(16),
                      children: [
                        for (final m in snap.modules)
                          Padding(
                            padding: const EdgeInsets.symmetric(vertical: 8),
                            child: _ModuleRow(m: m),
                          ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  flex: 4,
                  child: Card(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Padding(
                          padding: const EdgeInsets.all(16),
                          child: Text('Event log', style: Theme.of(context).textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w700)),
                        ),
                        Expanded(
                          child: Container(
                            width: double.infinity,
                            padding: const EdgeInsets.all(12),
                            child: SingleChildScrollView(
                              reverse: true,
                              child: SelectableText(
                                snap.log.takeLast(80).join('\n'),
                                style: const TextStyle(fontFamily: 'monospace', fontSize: 11),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _ModuleRow extends StatelessWidget {
  final ModuleProgress m;

  const _ModuleRow({required this.m});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final active = m.status == 'running';
    final done = m.status == 'done';
    final skipped = m.status == 'skipped';
    final ratio = m.total > 0 ? (m.done / m.total).clamp(0.0, 1.0) : null;

    return Opacity(
      opacity: skipped ? 0.35 : 1.0,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                done ? Icons.check_circle : (active ? Icons.autorenew : Icons.pause_circle),
                size: 18,
                color: done ? scheme.primary : (active ? scheme.secondary : scheme.outline),
              ),
              const SizedBox(width: 10),
              Expanded(child: Text(m.id, style: const TextStyle(fontWeight: FontWeight.w600))),
              if (m.found > 0)
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(color: scheme.primaryContainer, borderRadius: BorderRadius.circular(12)),
                  child: Text('${m.found}', style: TextStyle(color: scheme.onPrimaryContainer, fontSize: 11, fontWeight: FontWeight.w700)),
                ),
              const SizedBox(width: 8),
              SizedBox(
                width: 72,
                child: Text(m.status, textAlign: TextAlign.right, style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 11)),
              ),
            ],
          ),
          const SizedBox(height: 6),
          if (!skipped) WavyProgress(progress: active ? ratio : (done ? 1.0 : 0.0), height: 8),
        ],
      ),
    );
  }
}

class _StatusPill extends StatelessWidget {
  final String status;

  const _StatusPill({required this.status});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final (color, onColor, icon) = switch (status) {
      'running' => (scheme.secondaryContainer, scheme.onSecondaryContainer, Icons.autorenew),
      'done' => (scheme.primaryContainer, scheme.onPrimaryContainer, Icons.check_circle),
      'cancelled' => (scheme.errorContainer, scheme.onErrorContainer, Icons.cancel),
      _ => (scheme.surfaceContainerHighest, scheme.onSurfaceVariant, Icons.help),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(20)),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 14, color: onColor),
          const SizedBox(width: 6),
          Text(status, style: TextStyle(color: onColor, fontWeight: FontWeight.w700, fontSize: 12)),
        ],
      ),
    );
  }
}

extension TakeLast<T> on List<T> {
  List<T> takeLast(int n) => length <= n ? List<T>.from(this) : sublist(length - n);
}
