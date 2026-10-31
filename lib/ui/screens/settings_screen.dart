import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../core/app_state.dart';

class SettingsScreen extends StatelessWidget {
  final AppState state;

  const SettingsScreen({super.key, required this.state});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return ListView(
      padding: const EdgeInsets.all(32),
      children: [
        Text('Settings', style: textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
        const SizedBox(height: 24),
        _Section(
          title: 'Appearance',
          child: SegmentedButton<bool>(
            segments: const [
              ButtonSegment(value: true, icon: Icon(Icons.dark_mode), label: Text('Dark')),
              ButtonSegment(value: false, icon: Icon(Icons.light_mode), label: Text('Light')),
            ],
            selected: {state.darkTheme},
            onSelectionChanged: (s) => state.setDarkTheme(s.first),
          ),
        ),
        _Section(
          title: 'Scan engine',
          child: Column(
            children: [
              Row(
                children: [
                  const SizedBox(width: 160, child: Text('Concurrency')),
                  Expanded(
                    child: Slider(
                      min: 4,
                      max: 64,
                      divisions: 15,
                      label: '${state.concurrency}',
                      value: state.concurrency.toDouble(),
                      onChanged: (v) => state.setConcurrency(v.round()),
                    ),
                  ),
                  SizedBox(width: 40, child: Text('${state.concurrency}')),
                ],
              ),
              Row(
                children: [
                  const SizedBox(width: 160, child: Text('Request timeout')),
                  Expanded(
                    child: Slider(
                      min: 4,
                      max: 60,
                      divisions: 14,
                      label: '${state.timeoutSecs}s',
                      value: state.timeoutSecs.toDouble(),
                      onChanged: (v) => state.setTimeoutSecs(v.round()),
                    ),
                  ),
                  SizedBox(width: 40, child: Text('${state.timeoutSecs}s')),
                ],
              ),
              const SizedBox(height: 8),
              TextField(
                controller: TextEditingController(text: state.userAgent),
                onSubmitted: state.setUserAgent,
                decoration: const InputDecoration(hintText: 'User-Agent', labelText: 'User-Agent'),
              ),
            ],
          ),
        ),
        _Section(
          title: 'Network',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TextField(
                controller: TextEditingController(text: state.proxy),
                onSubmitted: state.setProxy,
                decoration: const InputDecoration(
                  hintText: 'socks5h://127.0.0.1:9050',
                  labelText: 'Proxy URL',
                  helperText: 'http, https, socks5 or socks5h · applies to every scan · leave empty for direct connection',
                ),
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                children: [
                  ActionChip(
                    avatar: const Icon(Icons.router, size: 18),
                    label: const Text('Tor 9050'),
                    tooltip: 'socks5h://127.0.0.1:9050',
                    onPressed: () => state.setProxy('socks5h://127.0.0.1:9050'),
                  ),
                  ActionChip(
                    avatar: const Icon(Icons.shield_outlined, size: 18),
                    label: const Text('Tor 9150'),
                    tooltip: 'socks5h://127.0.0.1:9150 (Tor Browser)',
                    onPressed: () => state.setProxy('socks5h://127.0.0.1:9150'),
                  ),
                  ActionChip(
                    avatar: const Icon(Icons.link_off, size: 18),
                    label: const Text('Direct'),
                    onPressed: () => state.setProxy(''),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Icon(
                    state.proxy.isEmpty ? Icons.lan : Icons.vpn_lock,
                    size: 18,
                    color: state.proxy.isEmpty ? scheme.onSurfaceVariant : scheme.primary,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      state.proxy.isEmpty ? 'Direct connection' : 'Traffic routed through ${state.proxy}',
                      style: TextStyle(color: scheme.onSurfaceVariant),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        _Section(
          title: 'Native core',
          child: Row(
            children: [
              Icon(state.nativeReady ? Icons.check_circle : Icons.error, color: state.nativeReady ? Colors.green.shade300 : scheme.error, size: 20),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  state.nativeReady ? 'Rust core connected · ${state.modules.length} modules loaded' : 'Rust core unavailable: ${state.nativeError}',
                  style: TextStyle(color: scheme.onSurfaceVariant),
                ),
              ),
            ],
          ),
        ),
        _Section(
          title: 'About',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Anyway v0.1.0', style: textTheme.bodyMedium),
              const SizedBox(height: 4),
              Text('Cross-platform OSINT workstation · GPL-3.0', style: TextStyle(color: scheme.onSurfaceVariant)),
              const SizedBox(height: 12),
              OutlinedButton.icon(
                onPressed: () => launchUrl(Uri.parse('https://github.com/mel0k1/Anyway'), mode: LaunchMode.externalApplication),
                icon: const Icon(Icons.code),
                label: const Text('github.com/mel0k1/Anyway'),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _Section extends StatelessWidget {
  final String title;
  final Widget child;

  const _Section({required this.title, required this.child});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 20),
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: Theme.of(context).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
              const SizedBox(height: 16),
              child,
            ],
          ),
        ),
      ),
    );
  }
}
