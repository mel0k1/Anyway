import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';

/// Cross-scan entity index: every finished scan feeds findings here, so
/// identities can be tracked over time and compared between scans.
class EntitiesScreen extends StatefulWidget {
  final AppState state;

  const EntitiesScreen({super.key, required this.state});

  @override
  State<EntitiesScreen> createState() => _EntitiesScreenState();
}

class _EntitiesScreenState extends State<EntitiesScreen> {
  final TextEditingController _query = TextEditingController();
  String _kind = 'all';
  List<EntityRecord> _rows = [];
  Map<String, dynamic>? _stats;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  Future<void> _reload() async {
    final db = widget.state.estorePath;
    if (db.isEmpty || !widget.state.nativeReady) {
      setState(() {
        _loading = false;
        _error = 'Native core unavailable';
      });
      return;
    }
    try {
      final rows = RustCore.estoreSearch(
        db,
        _query.text.trim(),
        kind: _kind,
        limit: 300,
      );
      final stats = RustCore.estoreStats(db);
      setState(() {
        _rows = rows;
        _stats = stats;
        _loading = false;
        _error = null;
      });
    } catch (e) {
      setState(() {
        _loading = false;
        _error = '$e';
      });
    }
  }

  Future<void> _openEntity(EntityRecord r) async {
    final db = widget.state.estorePath;
    Map<String, dynamic>? detail;
    try {
      detail = RustCore.estoreEntity(db, r.id);
    } catch (_) {}
    if (!mounted) return;
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (context) {
        final observations =
            ((detail?['observations'] as List?) ?? const []).cast<Map>();
        return SafeArea(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
            children: [
              SelectableText(
                r.value,
                style: Theme.of(context).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                children: [
                  Chip(label: Text(r.kind)),
                  if (r.label.isNotEmpty) Chip(label: Text(r.label)),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                'first seen ${_fmt(r.firstSeen)} · last seen ${_fmt(r.lastSeen)} · seen ${r.timesSeen}× in ${observations.length} scan${observations.length == 1 ? '' : 's'}',
                style: TextStyle(color: Theme.of(context).colorScheme.onSurfaceVariant),
              ),
              const Divider(height: 28),
              Text('Scan history', style: Theme.of(context).textTheme.titleSmall),
              const SizedBox(height: 8),
              for (final o in observations)
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  dense: true,
                  leading: const Icon(Icons.schedule, size: 18),
                  title: Text('${o['scan_id']}', style: const TextStyle(fontSize: 13)),
                  subtitle: Text('seen ${_fmt((o['seen_at'] as num?)?.toInt() ?? 0)}',
                      style: const TextStyle(fontSize: 12)),
                ),
              if (observations.isEmpty)
                Text('No scan observations recorded',
                    style: TextStyle(color: Theme.of(context).colorScheme.onSurfaceVariant)),
            ],
          ),
        );
      },
    );
  }

  String _fmt(int ts) {
    if (ts <= 0) return '—';
    final d = DateTime.fromMillisecondsSinceEpoch(ts * 1000);
    return DateFormat('yyyy-MM-dd HH:mm').format(d);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final kinds = ((_stats?['kinds'] as List?) ?? const [])
        .cast<Map>()
        .map((k) => '${k['kind']}')
        .toList();

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(32, 24, 32, 8),
          child: Row(
            children: [
              Expanded(
                child: Text('Entities', style: textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
              ),
              if (_stats != null)
                Text(
                  '${_stats?['entities']} entities · ${_stats?['scans']} scans',
                  style: TextStyle(color: scheme.onSurfaceVariant),
                ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(32, 8, 32, 8),
          child: Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _query,
                  decoration: const InputDecoration(
                    hintText: 'Search value or label…',
                    prefixIcon: Icon(Icons.search),
                    isDense: true,
                  ),
                  onSubmitted: (_) => _reload(),
                ),
              ),
              const SizedBox(width: 12),
              ActionChip(
                avatar: const Icon(Icons.refresh, size: 16),
                label: const Text('Search'),
                onPressed: _reload,
              ),
            ],
          ),
        ),
        SizedBox(
          height: 44,
          child: ListView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 32),
            children: [
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: FilterChip(
                  label: const Text('all'),
                  selected: _kind == 'all',
                  onSelected: (_) {
                    setState(() => _kind = 'all');
                    _reload();
                  },
                ),
              ),
              for (final k in kinds)
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: FilterChip(
                    label: Text(k),
                    selected: _kind == k,
                    onSelected: (_) {
                      setState(() => _kind = k);
                      _reload();
                    },
                  ),
                ),
            ],
          ),
        ),
        Expanded(
          child: _error != null
              ? Center(child: Text(_error!, style: TextStyle(color: scheme.error)))
              : _loading
                  ? const Center(child: CircularProgressIndicator())
                  : _rows.isEmpty
                      ? Center(
                          child: Text(
                            'No entities yet — finish a scan and its findings will be indexed here',
                            textAlign: TextAlign.center,
                            style: TextStyle(color: scheme.onSurfaceVariant),
                          ),
                        )
                      : ListView.builder(
                          padding: const EdgeInsets.fromLTRB(32, 8, 32, 24),
                          itemCount: _rows.length,
                          itemBuilder: (context, i) {
                            final r = _rows[i];
                            return Card(
                              child: ListTile(
                                onTap: () => _openEntity(r),
                                title: Text(
                                  r.value,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: const TextStyle(fontSize: 13.5),
                                ),
                                subtitle: Text(
                                  '${r.kind}${r.label.isEmpty ? '' : ' · ${r.label}'} · seen ${r.timesSeen}× · last ${_fmt(r.lastSeen)}',
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: const TextStyle(fontSize: 12),
                                ),
                                trailing: const Icon(Icons.chevron_right, size: 18),
                              ),
                            );
                          },
                        ),
        ),
      ],
    );
  }
}
