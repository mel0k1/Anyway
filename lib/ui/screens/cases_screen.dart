import 'package:flutter/material.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';

class CasesScreen extends StatelessWidget {
  final AppState state;
  final void Function() onOpenCase;

  const CasesScreen({super.key, required this.state, required this.onOpenCase});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;

    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Cases', style: Theme.of(context).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 6),
          Text('Every scan is archived automatically and can be reopened or exported', style: TextStyle(color: scheme.onSurfaceVariant)),
          const SizedBox(height: 20),
          Expanded(
            child: state.caseList.isEmpty
                ? Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(Icons.folder_open, size: 72, color: scheme.outlineVariant),
                        const SizedBox(height: 16),
                        Text('No cases yet', style: Theme.of(context).textTheme.titleLarge),
                      ],
                    ),
                  )
                : ListView.builder(
                    itemCount: state.caseList.length,
                    itemBuilder: (context, i) {
                      final c = state.caseList[i];
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 10),
                        child: Card(
                          child: ListTile(
                            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(24)),
                            contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
                            leading: CircleAvatar(
                              backgroundColor: scheme.primaryContainer,
                              child: Icon(_kindIcon(c.kind), color: scheme.onPrimaryContainer, size: 22),
                            ),
                            title: Text(c.title, style: const TextStyle(fontWeight: FontWeight.w700)),
                            subtitle: Text('${c.kind.title} · ${c.findingCount} findings · ${_date(c.createdAt)}'),
                            trailing: IconButton(
                              tooltip: 'Delete case',
                              icon: const Icon(Icons.delete_outline),
                              onPressed: () async {
                                final ok = await showDialog<bool>(
                                  context: context,
                                  builder: (context) => AlertDialog(
                                    title: const Text('Delete case'),
                                    content: Text('Remove "${c.title}" and its report?'),
                                    actions: [
                                      TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancel')),
                                      FilledButton(onPressed: () => Navigator.pop(context, true), child: const Text('Delete')),
                                    ],
                                  ),
                                );
                                if (ok == true) state.deleteCase(c);
                              },
                            ),
                            onTap: () {
                              state.openCase(c);
                              onOpenCase();
                            },
                          ),
                        ),
                      );
                    },
                  ),
          ),
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
    return '${d.year}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')} ${d.hour.toString().padLeft(2, '0')}:${d.minute.toString().padLeft(2, '0')}';
  }
}
