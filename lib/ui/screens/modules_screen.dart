import 'package:flutter/material.dart';

import '../../core/app_state.dart';

class ModulesScreen extends StatelessWidget {
  final AppState state;

  const ModulesScreen({super.key, required this.state});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;

    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Modules',
            style: Theme.of(context).textTheme.headlineSmall
                ?.copyWith(fontWeight: FontWeight.w800),
          ),
          const SizedBox(height: 6),
          Text(
            'Enabled modules run on every matching scan',
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          const SizedBox(height: 20),
          Expanded(
            child: LayoutBuilder(
              builder: (context, box) {
                final cols = (box.maxWidth / 380).floor().clamp(1, 4);
                return GridView.builder(
                  gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: cols,
                    crossAxisSpacing: 14,
                    mainAxisSpacing: 14,
                    childAspectRatio: 1.9,
                  ),
                  itemCount: state.modules.length,
                  itemBuilder: (context, i) {
                    final m = state.modules[i];
                    final enabled = !state.disabledModules.contains(m.id);
                    return Card(
                      child: Padding(
                        padding: const EdgeInsets.all(18),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Expanded(
                                  child: Text(
                                    m.name,
                                    style: const TextStyle(
                                      fontWeight: FontWeight.w800,
                                      fontSize: 15,
                                    ),
                                  ),
                                ),
                                Switch(
                                  value: enabled,
                                  onChanged: (v) => state.toggleModule(m.id, v),
                                ),
                              ],
                            ),
                            Expanded(
                              child: Text(
                                m.description,
                                style: TextStyle(
                                  color: scheme.onSurfaceVariant,
                                  fontSize: 12.5,
                                ),
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            Wrap(
                              spacing: 6,
                              children: [
                                for (final a in m.accepts)
                                  Chip(
                                    label: Text(
                                      a.title,
                                      style: const TextStyle(fontSize: 10.5),
                                    ),
                                  ),
                              ],
                            ),
                          ],
                        ),
                      ),
                    );
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
