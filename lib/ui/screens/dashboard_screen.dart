import 'package:flutter/material.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';

class DashboardScreen extends StatefulWidget {
  final AppState state;
  final void Function() onStartScan;

  const DashboardScreen({super.key, required this.state, required this.onStartScan});

  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  final _controller = TextEditingController();
  TargetKind _kind = TargetKind.unknown;

  void _onChanged(String v) => setState(() => _kind = detectTargetKind(v));

  void _start() {
    if (_controller.text.trim().isEmpty) return;
    widget.state.startScan(_controller.text.trim());
    widget.onStartScan();
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final scheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return SingleChildScrollView(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 56,
                height: 56,
                decoration: BoxDecoration(color: scheme.primaryContainer, borderRadius: BorderRadius.circular(20)),
                child: Icon(Icons.travel_explore, size: 30, color: scheme.onPrimaryContainer),
              ),
              const SizedBox(width: 16),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Anyway', style: textTheme.headlineMedium?.copyWith(fontWeight: FontWeight.w800)),
                  Text('One query. Every source. Rust speed.', style: textTheme.bodyMedium?.copyWith(color: scheme.onSurfaceVariant)),
                ],
              ),
            ],
          ),
          const SizedBox(height: 32),
          if (!state.nativeReady)
            Card(
              color: scheme.errorContainer,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  children: [
                    Icon(Icons.warning, color: scheme.onErrorContainer),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text('Native core unavailable: ${state.nativeError}', style: TextStyle(color: scheme.onErrorContainer)),
                    ),
                  ],
                ),
              ),
            ),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('New scan', style: textTheme.titleLarge?.copyWith(fontWeight: FontWeight.w700)),
                  const SizedBox(height: 4),
                  Text('Username, email, IP, domain, phone number or image path · combine targets with commas', style: textTheme.bodySmall?.copyWith(color: scheme.onSurfaceVariant)),
                  const SizedBox(height: 20),
                  Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: _controller,
                          onChanged: _onChanged,
                          onSubmitted: (_) => _start(),
                          style: const TextStyle(fontSize: 17),
                          decoration: InputDecoration(
                            hintText: 'target to investigate',
                            prefixIcon: const Padding(padding: EdgeInsets.only(left: 12, right: 4), child: Icon(Icons.search)),
                            suffixIcon: _kind != TargetKind.unknown
                                ? Padding(
                                    padding: const EdgeInsets.only(right: 12),
                                    child: Chip(
                                      label: Text(_kind.title),
                                      backgroundColor: scheme.secondaryContainer,
                                      labelStyle: TextStyle(color: scheme.onSecondaryContainer, fontWeight: FontWeight.w700),
                                    ),
                                  )
                                : null,
                          ),
                        ),
                      ),
                      const SizedBox(width: 16),
                      AnimatedScale(
                        scale: _controller.text.trim().isEmpty ? 0.96 : 1.0,
                        duration: const Duration(milliseconds: 160),
                        curve: Curves.easeOutBack,
                        child: FilledButton.icon(
                          onPressed: _controller.text.trim().isEmpty ? null : _start,
                          icon: const Icon(Icons.radar),
                          label: const Text('Scan'),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 24),
          Text('Modules', style: textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
          const SizedBox(height: 12),
          Wrap(
            spacing: 12,
            runSpacing: 12,
            children: [
              for (final m in state.modules)
                SizedBox(
                  width: 320,
                  child: Card(
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(m.name, style: const TextStyle(fontWeight: FontWeight.w700)),
                          const SizedBox(height: 6),
                          Text(m.description, style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12.5), maxLines: 2, overflow: TextOverflow.ellipsis),
                          const SizedBox(height: 10),
                          Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: [
                              if (m.sites != null)
                                Chip(
                                  label: Text('${m.sites} sites', style: const TextStyle(fontSize: 10.5, fontWeight: FontWeight.w700)),
                                  backgroundColor: scheme.primaryContainer,
                                  labelStyle: TextStyle(color: scheme.onPrimaryContainer),
                                ),
                              for (final a in m.accepts) Chip(label: Text(a.title, style: const TextStyle(fontSize: 10.5))),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          ),
          if (state.caseList.isNotEmpty) ...[
            const SizedBox(height: 24),
            Text('Recent cases', style: textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
            const SizedBox(height: 12),
            for (final c in state.caseList.take(3))
              ListTile(
                contentPadding: const EdgeInsets.symmetric(horizontal: 8),
                leading: Icon(_kindIcon(c.kind), color: scheme.primary),
                title: Text(c.title),
                subtitle: Text('${c.findingCount} findings · ${_date(c.createdAt)}'),
              ),
          ],
        ],
      ),
    );
  }

  IconData _kindIcon(TargetKind k) => switch (k) {
        TargetKind.username => Icons.person_search,
        TargetKind.email => Icons.mail,
        TargetKind.ipAddress => Icons.public,
        TargetKind.domain => Icons.dns,
        TargetKind.phoneNumber => Icons.call,
        TargetKind.file => Icons.image,
        TargetKind.unknown => Icons.circle,
      };

  String _date(int ts) {
    final d = DateTime.fromMillisecondsSinceEpoch(ts * 1000);
    return '${d.year}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';
  }
}
