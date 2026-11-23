import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../../core/app_state.dart';
import '../../core/rust_core.dart';

class PlannerScreen extends StatefulWidget {
  final AppState state;

  const PlannerScreen({super.key, required this.state});

  @override
  State<PlannerScreen> createState() => _PlannerScreenState();
}

class _PlannerScreenState extends State<PlannerScreen> {
  final _queryController = TextEditingController();
  int _intervalSecs = 3600;

  static const _intervals = [
    (900, 'every 15 min'),
    (3600, 'every hour'),
    (21600, 'every 6 hours'),
    (86400, 'every day'),
    (604800, 'every week'),
  ];

  @override
  void dispose() {
    _queryController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final entries = widget.state.scheduleEntries;

    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Re-scan planner', style: Theme.of(context).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 6),
          Text(
            'Scheduled scans run in the background while the app is open. Each run is compared with the previous one and new findings are flagged',
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          const SizedBox(height: 20),
          if (widget.state.plannerJournal.isNotEmpty)
            Card(
              margin: const EdgeInsets.only(bottom: 12),
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        const Icon(Icons.notifications_active, size: 18),
                        const SizedBox(width: 8),
                        Text('Planner events', style: Theme.of(context).textTheme.titleSmall),
                        const Spacer(),
                        ActionChip(
                          avatar: const Icon(Icons.clear_all, size: 16),
                          label: const Text('Clear'),
                          onPressed: () => widget.state.clearJournal(),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    for (final rec in widget.state.plannerJournal.take(8))
                      Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: Text(
                          '${_fmtTs(rec['ts'])} · ${rec['query']} · +${rec['total']} new'
                          '${(rec['added'] as List?) == null || (rec['added'] as List).isEmpty ? '' : ': ${(rec['added'] as List).take(4).join(', ')}${(rec['added'] as List).length > 4 ? '…' : ''}'}',
                          style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: TextField(
                          controller: _queryController,
                          decoration: const InputDecoration(
                            labelText: 'Target to re-scan',
                            hintText: 'username, email, domain, IP or phone',
                            prefixIcon: Icon(Icons.travel_explore, size: 20),
                          ),
                        ),
                      ),
                      const SizedBox(width: 16),
                      DropdownButton<int>(
                        value: _intervalSecs,
                        items: [
                          for (final (secs, label) in _intervals)
                            DropdownMenuItem(value: secs, child: Text(label)),
                        ],
                        onChanged: (v) => setState(() => _intervalSecs = v ?? 3600),
                      ),
                      const SizedBox(width: 16),
                      FilledButton.icon(
                        onPressed: _add,
                        icon: const Icon(Icons.add_alarm),
                        label: const Text('Schedule'),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'The engine re-uses your current network settings (Tor mode, proxy, module set)',
                    style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          if (entries.isEmpty)
            Expanded(
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(Icons.schedule, size: 72, color: scheme.outlineVariant),
                    const SizedBox(height: 16),
                    Text('No scheduled re-scans', style: Theme.of(context).textTheme.titleLarge),
                    const SizedBox(height: 8),
                    Text('Add a target above to monitor it over time', style: TextStyle(color: scheme.onSurfaceVariant)),
                  ],
                ),
              ),
            )
          else
            Expanded(
              child: ListView(
                children: [
                  for (final e in entries)
                    Card(
                      margin: const EdgeInsets.only(bottom: 10),
                      child: ListTile(
                        leading: Icon(
                          e.running
                              ? Icons.autorenew
                              : (e.enabled ? Icons.alarm_on : Icons.alarm_off),
                          color: e.running
                              ? scheme.primary
                              : (e.enabled ? scheme.primary : scheme.onSurfaceVariant),
                        ),
                        title: Text(
                          e.query,
                          style: const TextStyle(fontWeight: FontWeight.w700),
                        ),
                        subtitle: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              '${_intervalLabel(e.intervalSecs)} · runs: ${e.runs} · findings: ${e.findingsTotal}'
                              '${e.lastStatus == null ? '' : ' · last: ${e.lastStatus}'}'
                              '${(e.newFindings ?? 0) > 0 ? ' · ${e.newFindings} NEW' : ''}'
                              '${e.running ? ' · scanning now' : ' · due in ${_humanDur(e.dueIn)}'}',
                            ),
                            if (e.addedFindings.isNotEmpty)
                              Padding(
                                padding: const EdgeInsets.only(top: 4),
                                child: Wrap(
                                  spacing: 6,
                                  runSpacing: 4,
                                  children: [
                                    for (final f in e.addedFindings.take(6))
                                      Container(
                                        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                                        decoration: BoxDecoration(
                                          color: scheme.primaryContainer,
                                          borderRadius: BorderRadius.circular(999),
                                        ),
                                        child: Text(
                                          f,
                                          style: TextStyle(
                                            fontSize: 10.5,
                                            color: scheme.onPrimaryContainer,
                                          ),
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      ),
                                    if (e.addedFindings.length > 6)
                                      Text(
                                        '+${e.addedFindings.length - 6} more',
                                        style: TextStyle(
                                          fontSize: 10.5,
                                          color: scheme.onSurfaceVariant,
                                        ),
                                      ),
                                  ],
                                ),
                              ),
                          ],
                        ),
                        trailing: Wrap(
                          spacing: 4,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            IconButton(
                              tooltip: 'Run now',
                              icon: const Icon(Icons.play_arrow, size: 20),
                              onPressed: e.running ? null : () => widget.state.runScheduleNow(e.id),
                            ),
                            IconButton(
                              tooltip: 'Last snapshot',
                              icon: const Icon(Icons.receipt_long, size: 20),
                              onPressed: () => _showSnapshot(context, e.id),
                            ),
                            IconButton(
                              tooltip: 'Remove',
                              icon: const Icon(Icons.delete_outline, size: 20),
                              onPressed: () => widget.state.removeSchedule(e.id),
                            ),
                            Switch(
                              value: e.enabled,
                              onChanged: (v) => widget.state.setScheduleEnabled(e, v),
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

  void _add() {
    final query = _queryController.text.trim();
    if (query.isEmpty) return;
    widget.state.addSchedule(query, _intervalSecs);
    _queryController.clear();
  }

  String _fmtTs(Object? ts) {
    final v = (ts as num?)?.toInt() ?? 0;
    if (v <= 0) return '—';
    final d = DateTime.fromMillisecondsSinceEpoch(v * 1000);
    return DateFormat('MM-dd HH:mm').format(d);
  }

  String _intervalLabel(int secs) {
    for (final (s, label) in _intervals) {
      if (s == secs) return label;
    }
    return 'every ${_humanDur(secs)}';
  }

  String _humanDur(int secs) {
    if (secs < 60) return '${secs}s';
    if (secs < 3600) return '${(secs / 60).round()}m';
    if (secs < 86400) return '${(secs / 3600).round()}h';
    return '${(secs / 86400).round()}d';
  }

  Future<void> _showSnapshot(BuildContext context, String id) async {
    final messenger = ScaffoldMessenger.of(context);
    try {
      final snap = RustCore.schedulerSnapshot(id);
      if (!mounted) return;
      final findings = ((snap?['findings'] as List?) ?? const []).length;
      final log = ((snap?['log'] as List?) ?? const []).cast<String>();
      await showDialog<void>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Last scheduled scan'),
          content: SizedBox(
            width: 480,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('Status: ${snap?['status'] ?? '?'} · findings: $findings'),
                const SizedBox(height: 12),
                Flexible(
                  child: SingleChildScrollView(
                    child: Text(
                      log.take(30).join('\n'),
                      style: const TextStyle(fontSize: 11, fontFamily: 'monospace'),
                    ),
                  ),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Close')),
          ],
        ),
      );
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('Snapshot unavailable: $e')));
    }
  }
}
