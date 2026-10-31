import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../core/models.dart';

class FindingCard extends StatefulWidget {
  final Finding finding;

  const FindingCard({super.key, required this.finding});

  @override
  State<FindingCard> createState() => _FindingCardState();
}

class _FindingCardState extends State<FindingCard> {
  bool _open = false;

  IconData get _icon => switch (widget.finding.category) {
        'social_profile' => Icons.person,
        'api_profile' => Icons.badge,
        'email_info' => Icons.mail,
        'gravatar' => Icons.face,
        'ip_geo' => Icons.public,
        'ip_network' => Icons.lan,
        'ip_rdap' => Icons.gavel,
        'ip_ptr' => Icons.dns,
        'dns_records' => Icons.dns,
        'subdomains' => Icons.account_tree,
        'phone_info' => Icons.call,
        'metadata' => Icons.image,
        _ => Icons.circle,
      };

  Color get _confidenceColor => switch (widget.finding.confidence) {
        Confidence.high => Colors.green.shade300,
        Confidence.medium => Colors.orange.shade300,
        Confidence.low => Colors.red.shade300,
      };

  @override
  Widget build(BuildContext context) {
    final f = widget.finding;
    final scheme = Theme.of(context).colorScheme;
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(_icon, size: 20, color: scheme.primary),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(f.label, style: const TextStyle(fontWeight: FontWeight.w600)),
                      const SizedBox(height: 2),
                      SelectableText(f.value, style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 13)),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                _ConfidenceChip(color: _confidenceColor, label: f.confidence.name),
                IconButton(
                  tooltip: 'Copy',
                  icon: const Icon(Icons.copy, size: 18),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: f.url ?? f.value));
                    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Copied: ${f.url ?? f.value}')));
                  },
                ),
                if (f.url != null)
                  IconButton(
                    tooltip: 'Open',
                    icon: const Icon(Icons.open_in_new, size: 18),
                    onPressed: () => launchUrl(Uri.parse(f.url!), mode: LaunchMode.externalApplication),
                  ),
                if (f.detail != null)
                  IconButton(
                    tooltip: 'Details',
                    icon: Icon(_open ? Icons.expand_less : Icons.expand_more, size: 20),
                    onPressed: () => setState(() => _open = !_open),
                  ),
              ],
            ),
            AnimatedCrossFade(
              crossFadeState: _open ? CrossFadeState.showSecond : CrossFadeState.showFirst,
              duration: const Duration(milliseconds: 180),
              firstChild: const SizedBox(width: double.infinity),
              secondChild: Padding(
                padding: const EdgeInsets.only(top: 10),
                child: Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: scheme.surfaceContainerHigh,
                    borderRadius: BorderRadius.circular(16),
                  ),
                  child: SelectableText(
                    const JsonEncoder.withIndent('  ').convert(f.detail),
                    style: const TextStyle(fontFamily: 'monospace', fontSize: 11.5),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ConfidenceChip extends StatelessWidget {
  final Color color;
  final String label;

  const _ConfidenceChip({required this.color, required this.label});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(20),
      ),
      child: Text(label.toUpperCase(), style: TextStyle(color: color, fontSize: 10, fontWeight: FontWeight.w700, letterSpacing: 0.5)),
    );
  }
}
