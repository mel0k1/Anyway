import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';

import '../../core/models.dart';
import '../../core/rust_core.dart';

/// Local breach-dump indexing: import text/JSONL/CSV dumps into a SQLite
/// database on this device and search it offline by email, username,
/// phone or domain.
class LeaksScreen extends StatefulWidget {
  const LeaksScreen({super.key});

  @override
  State<LeaksScreen> createState() => _LeaksScreenState();
}

class _LeaksScreenState extends State<LeaksScreen> {
  String? _dbPath;
  Map<String, dynamic>? _stats;
  bool _busy = false;
  bool _reveal = false;
  List<LeakRecord> _results = [];
  String _lastField = '';
  int _lastTotal = 0;
  final _queryController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _initDb();
  }

  Future<void> _initDb() async {
    try {
      final base = await getApplicationSupportDirectory();
      if (!await base.exists()) await base.create(recursive: true);
      _dbPath = '${base.path}${Platform.pathSeparator}leaks.sqlite';
      _refreshStats();
    } catch (_) {}
  }

  Future<void> _refreshStats() async {
    if (_dbPath == null) return;
    try {
      _stats = RustCore.leaksStats(_dbPath!);
      if (mounted) setState(() {});
    } catch (_) {}
  }

  Future<void> _importDump() async {
    final messenger = ScaffoldMessenger.of(context);
    const group = XTypeGroup(
      label: 'dumps',
      extensions: ['txt', 'csv', 'json', 'jsonl', 'log', 'dump', 'sql', 'lst'],
    );
    try {
      final file = await openFile(acceptedTypeGroups: [group]);
      if (file == null || _dbPath == null) return;
      setState(() => _busy = true);
      final text = await file.readAsString();
      final source = file.name.replaceAll(RegExp(r'\.[a-z0-9]+$', caseSensitive: false), '');
      final res = RustCore.leaksImport(_dbPath!, source, text);
      await _refreshStats();
      messenger.showSnackBar(SnackBar(
        content: Text(
          'Imported ${res['imported']} records from ${res['lines']} lines (${res['skipped']} skipped)',
        ),
      ));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('Import failed: $e')));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  void _search() {
    final messenger = ScaffoldMessenger.of(context);
    final q = _queryController.text.trim();
    if (q.isEmpty || _dbPath == null) return;
    try {
      final res = RustCore.leaksSearch(_dbPath!, q, limit: 200, reveal: _reveal);
      setState(() {
        _results = ((res['matches'] as List?) ?? const [])
            .map((e) => LeakRecord.fromJson((e as Map).cast<String, dynamic>()))
            .toList();
        _lastField = res['queryField'] as String? ?? '';
        _lastTotal = (res['total'] as num?)?.toInt() ?? 0;
      });
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e')));
    }
  }

  Future<void> _clearDb() async {
    final messenger = ScaffoldMessenger.of(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Clear the local leak index?'),
        content: const Text('All imported records and sources will be deleted from this device.'),
        actions: [
          TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
          FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Delete')),
        ],
      ),
    );
    if (confirmed != true || _dbPath == null) return;
    try {
      RustCore.leaksClear(_dbPath!);
      setState(() => _results = []);
      await _refreshStats();
      messenger.showSnackBar(const SnackBar(content: Text('Local index cleared')));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final total = (_stats?['records'] as num?)?.toInt() ?? 0;
    final withPass = (_stats?['withPassword'] as num?)?.toInt() ?? 0;
    final sources = ((_stats?['sources'] as List?) ?? const [])
        .map((e) => (e as Map).cast<String, dynamic>())
        .toList();

    return Padding(
      padding: const EdgeInsets.all(32),
      child: ListView(
        children: [
          Text('Leak index', style: textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 6),
          Text(
            'Import breach dumps (email:pass, user:email:pass, JSONL, CSV) into a local SQLite index and search it instantly by email, username, phone or domain. Everything stays on this device, nothing is sent anywhere. Passwords are masked unless you reveal them',
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(Icons.storage, size: 20, color: scheme.primary),
                      const SizedBox(width: 10),
                      Text('$total records · $withPass with passwords · ${sources.length} sources',
                          style: const TextStyle(fontWeight: FontWeight.w700)),
                      const Spacer(),
                      OutlinedButton.icon(
                        onPressed: _busy ? null : _importDump,
                        icon: const Icon(Icons.upload_file),
                        label: const Text('Import dump'),
                      ),
                      const SizedBox(width: 10),
                      IconButton(
                        tooltip: 'Clear local index',
                        icon: const Icon(Icons.delete_sweep, size: 20),
                        onPressed: total == 0 ? null : _clearDb,
                      ),
                    ],
                  ),
                  if (sources.isNotEmpty) ...[
                    const SizedBox(height: 12),
                    Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      children: [
                        for (final s in sources.take(24))
                          Chip(
                            label: Text(
                              '${s['name']} · ${s['records']}',
                              style: const TextStyle(fontSize: 11),
                            ),
                          ),
                      ],
                    ),
                  ],
                  if (_busy) const Padding(padding: EdgeInsets.all(12), child: CircularProgressIndicator()),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: _queryController,
                          onSubmitted: (_) => _search(),
                          decoration: const InputDecoration(
                            hintText: 'victim@example.org · john_doe · +7926… · corp.io',
                            labelText: 'Search the index',
                            prefixIcon: Icon(Icons.search, size: 20),
                          ),
                        ),
                      ),
                      const SizedBox(width: 16),
                      FilledButton.icon(
                        onPressed: _search,
                        icon: const Icon(Icons.search),
                        label: const Text('Search'),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                    title: const Text('Reveal passwords in results'),
                    subtitle: const Text('Off by default — results show masked passwords only'),
                    value: _reveal,
                    onChanged: (v) => setState(() => _reveal = v),
                  ),
                  if (_results.isNotEmpty) ...[
                    const SizedBox(height: 8),
                    Text(
                      '$_lastTotal match${_lastTotal == 1 ? '' : 'es'} for "${_queryController.text}" (searched field: $_lastField, showing ${_results.length})',
                      style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12),
                    ),
                    const SizedBox(height: 8),
                    SingleChildScrollView(
                      scrollDirection: Axis.horizontal,
                      child: DataTable(
                        columnSpacing: 28,
                        columns: const [
                          DataColumn(label: Text('Source')),
                          DataColumn(label: Text('Email')),
                          DataColumn(label: Text('Username')),
                          DataColumn(label: Text('Phone')),
                          DataColumn(label: Text('Password')),
                          DataColumn(label: Text('IP')),
                          DataColumn(label: Text('Domain')),
                        ],
                        rows: [
                          for (final r in _results.take(100))
                            DataRow(cells: [
                              DataCell(Text(r.source, style: const TextStyle(fontSize: 12))),
                              DataCell(Text(r.email, style: const TextStyle(fontSize: 12))),
                              DataCell(Text(r.username, style: const TextStyle(fontSize: 12))),
                              DataCell(Text(r.phone, style: const TextStyle(fontSize: 12))),
                              DataCell(Text(r.password, style: TextStyle(fontSize: 12, fontFamily: 'monospace', color: _reveal ? scheme.error : null))),
                              DataCell(Text(r.ip, style: const TextStyle(fontSize: 12))),
                              DataCell(Text(r.domain, style: const TextStyle(fontSize: 12))),
                            ]),
                        ],
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
