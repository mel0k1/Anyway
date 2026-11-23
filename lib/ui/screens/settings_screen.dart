import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';

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
          title: 'Tor routing',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Auto routes every scan through a local Tor SOCKS port when one is listening and falls back to direct otherwise. On is fail-closed: scans never leak traffic when Tor is down, they simply fail. Off never touches Tor',
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
              const SizedBox(height: 12),
              SegmentedButton<String>(
                segments: const [
                  ButtonSegment(value: 'auto', icon: Icon(Icons.auto_mode), label: Text('Auto')),
                  ButtonSegment(value: 'on', icon: Icon(Icons.vpn_lock), label: Text('On')),
                  ButtonSegment(value: 'off', icon: Icon(Icons.link_off), label: Text('Off')),
                ],
                selected: {state.torMode},
                onSelectionChanged: (s) => state.setTorMode(s.first),
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                children: [
                  FilterChip(
                    label: const Text('9050 (tor service)'),
                    selected: state.torSocksPort == 9050,
                    onSelected: (_) => state.setTorSocksPort(9050),
                  ),
                  FilterChip(
                    label: const Text('9150 (Tor Browser)'),
                    selected: state.torSocksPort == 9150,
                    onSelected: (_) => state.setTorSocksPort(9150),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Embedded Tor (arti)'),
                subtitle: const Text(
                  'Runs a built-in Tor client inside the app when no external daemon is listening. Auto falls back to it; On waits for it and fails closed',
                ),
                secondary: const Icon(Icons.shield_moon),
                value: state.embeddedTor,
                onChanged: state.setEmbeddedTor,
              ),
              Row(
                children: [
                  Icon(
                    (state.artiStatus['status'] == 'ready')
                        ? Icons.verified_user
                        : (state.artiStatus['status'] == 'failed' ? Icons.error : Icons.hourglass_top),
                    size: 18,
                    color: state.artiStatus['status'] == 'ready'
                        ? scheme.primary
                        : scheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      'Embedded Tor: ${state.artiStatus['status'] ?? 'off'}'
                      '${state.artiStatus['status'] == 'ready' ? ' (port ${state.artiStatus['port']})' : ''}'
                      '${(state.artiStatus['detail'] as String?)?.isEmpty == false ? ' — ${state.artiStatus['detail']}' : ''}',
                      style: TextStyle(color: scheme.onSurfaceVariant),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Icon(
                    state.torStatus?.listening == true && state.torStatus?.isTor == true
                        ? Icons.verified_user
                        : Icons.help_outline,
                    size: 18,
                    color: state.torStatus?.listening == true && state.torStatus?.isTor == true
                        ? scheme.primary
                        : scheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      _torStatusText(state),
                      style: TextStyle(color: scheme.onSurfaceVariant),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  const SizedBox(width: 10),
                  ActionChip(
                    avatar: const Icon(Icons.refresh, size: 16),
                    label: const Text('Check now'),
                    onPressed: () => state.checkTor(),
                  ),
                ],
              ),
            ],
          ),
        ),
        _Section(
          title: 'Pivoting',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Recursive pivoting'),
                subtitle: const Text(
                  'Automatically scan entities discovered mid-scan: exposed emails, alt handles, resolved IPs, subdomains and linked domains. Only confirmed or questionable leads are followed',
                ),
                secondary: const Icon(Icons.hub),
                value: state.pivotEnabled,
                onChanged: state.setPivotEnabled,
              ),
              Row(
                children: [
                  const SizedBox(width: 160, child: Text('Pivot depth')),
                  Expanded(
                    child: Slider(
                      min: 1,
                      max: 4,
                      divisions: 3,
                      label: '${state.pivotDepth} wave${state.pivotDepth == 1 ? '' : 's'}',
                      value: state.pivotDepth.toDouble(),
                      onChanged: state.pivotEnabled ? (v) => state.setPivotDepth(v.round()) : null,
                    ),
                  ),
                  SizedBox(width: 40, child: Text('${state.pivotDepth}')),
                ],
              ),
              Row(
                children: [
                  const SizedBox(width: 160, child: Text('Pivot budget')),
                  Expanded(
                    child: Slider(
                      min: 2,
                      max: 16,
                      divisions: 14,
                      label: '${state.pivotBudget} entities',
                      value: state.pivotBudget.toDouble(),
                      onChanged: state.pivotEnabled ? (v) => state.setPivotBudget(v.round()) : null,
                    ),
                  ),
                  SizedBox(width: 40, child: Text('${state.pivotBudget}')),
                ],
              ),
            ],
          ),
        ),
        _Section(
          title: 'Database',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Long-tail database (Maigret)'),
                subtitle: const Text(
                  'Adds thousands of forum, wiki, gamedev and community sites from the Maigret database on top of the core set. Scans take noticeably longer but reach far deeper',
                ),
                secondary: const Icon(Icons.travel_explore),
                value: state.longTail,
                onChanged: state.setLongTail,
              ),
              Text(
                state.longTail
                    ? 'Username module will probe the full merged database'
                    : 'Username module will probe the core database only',
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
              const SizedBox(height: 12),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Headless browser verification'),
                subtitle: const Text(
                  'After the plain-HTTP probe, ambiguous sites (unknown answers, bot walls, JavaScript-rendered pages) are re-checked through a real Chromium-family browser and confirmed from the rendered DOM',
                ),
                secondary: const Icon(Icons.web_asset),
                value: state.browserChecks,
                onChanged: state.setBrowserChecks,
              ),
              Row(
                children: [
                  Icon(
                    state.detectedBrowser == null ? Icons.block : Icons.check_circle,
                    size: 18,
                    color: state.detectedBrowser == null ? scheme.onSurfaceVariant : scheme.primary,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      state.detectedBrowser == null
                          ? 'No Chromium-family browser detected on this system'
                          : 'Browser detected: ${state.detectedBrowser}',
                      style: TextStyle(color: scheme.onSurfaceVariant),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        _Section(
          title: 'Site categories',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Tap a category to cycle: every site, included only, or excluded. Filters apply to the username module',
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final c in state.categories)
                    _CategoryChip(
                      info: c,
                      state: state.categoryState(c.name),
                      onCycle: () {
                        final next = switch (state.categoryState(c.name)) {
                          CategoryState.neutral => CategoryState.allowed,
                          CategoryState.allowed => CategoryState.blocked,
                          CategoryState.blocked => CategoryState.neutral,
                        };
                        state.setCategoryState(c.name, next);
                      },
                    ),
                ],
              ),
              if (state.allowedCategories.isNotEmpty || state.blockedCategories.isNotEmpty) ...[
                const SizedBox(height: 12),
                Row(
                  children: [
                    if (state.allowedCategories.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(right: 8),
                        child: Text(
                          '${state.allowedCategories.length} included',
                          style: TextStyle(color: scheme.primary, fontWeight: FontWeight.w600),
                        ),
                      ),
                    if (state.blockedCategories.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(right: 8),
                        child: Text(
                          '${state.blockedCategories.length} excluded',
                          style: TextStyle(color: scheme.error, fontWeight: FontWeight.w600),
                        ),
                      ),
                    const Spacer(),
                    ActionChip(
                      avatar: const Icon(Icons.restart_alt, size: 16),
                      label: const Text('Reset'),
                      onPressed: state.resetCategories,
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
        _Section(
          title: 'Countries',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Select countries to focus the username module on region-specific sites. Global platforms with no country tag are always probed',
                style: TextStyle(color: scheme.onSurfaceVariant),
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final c in state.countries)
                    FilterChip(
                      label: Text('${c.name} · ${c.count}'),
                      selected: state.selectedCountries.contains(c.name),
                      onSelected: (on) => state.toggleCountry(c.name, on),
                    ),
                ],
              ),
              if (state.selectedCountries.isNotEmpty) ...[
                const SizedBox(height: 12),
                Row(
                  children: [
                    Text(
                      '${state.selectedCountries.length} countries selected',
                      style: TextStyle(color: scheme.primary, fontWeight: FontWeight.w600),
                    ),
                    const Spacer(),
                    ActionChip(
                      avatar: const Icon(Icons.restart_alt, size: 16),
                      label: const Text('Reset'),
                      onPressed: () async {
                        for (final name in state.selectedCountries.toList()) {
                          await state.toggleCountry(name, false);
                        }
                      },
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
        _Section(
          title: 'Breach lookup',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TextField(
                controller: TextEditingController(text: state.hibpApiKey),
                onSubmitted: state.setHibpApiKey,
                obscureText: true,
                autocorrect: false,
                enableSuggestions: false,
                decoration: const InputDecoration(
                  hintText: 'paste your key and press Enter',
                  labelText: 'Have I Been Pwned API key',
                  helperText: 'Required by the breach lookup module · free key at haveibeenpwned.com/API/Key',
                ),
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Icon(
                    state.hibpApiKey.isEmpty ? Icons.key_off : Icons.key,
                    size: 18,
                    color: state.hibpApiKey.isEmpty ? scheme.onSurfaceVariant : scheme.primary,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      state.hibpApiKey.isEmpty
                          ? 'Breach lookups will be skipped'
                          : 'Breach intel enabled · key stored on this device',
                      style: TextStyle(color: scheme.onSurfaceVariant),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        _Section(
          title: 'Passive DNS enrichment',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TextField(
                controller: TextEditingController(text: state.vtApiKey),
                onSubmitted: state.setVtApiKey,
                obscureText: true,
                autocorrect: false,
                enableSuggestions: false,
                decoration: const InputDecoration(
                  hintText: 'paste your key and press Enter',
                  labelText: 'VirusTotal API key',
                  helperText: 'Used by the passive DNS module for historical IPs and subdomains · free key at virustotal.com',
                ),
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Icon(
                    state.vtApiKey.isEmpty ? Icons.key_off : Icons.key,
                    size: 18,
                    color: state.vtApiKey.isEmpty ? scheme.onSurfaceVariant : scheme.primary,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      state.vtApiKey.isEmpty
                          ? 'Wayback Machine history still works without a key'
                          : 'VirusTotal passive DNS enabled · key stored on this device',
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
              Text('Anyway v0.4.0', style: textTheme.bodyMedium),
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

String _torStatusText(AppState state) {
  final t = state.torStatus;
  if (t == null) return 'Tor status unknown — run a check';
  if (!t.listening) {
    return 'No Tor SOCKS port on 127.0.0.1:${state.torSocksPort}'
        '${state.torMode == 'on' ? ' — scans will fail closed' : ''}';
  }
  if (t.isTor == true) {
    return 'Tor route verified${t.exitIp == null ? '' : ' · exit: ${t.exitIp}'}';
  }
  return 'Port open but the exit check did not confirm Tor traffic';
}


class _CategoryChip extends StatelessWidget {
  final CategoryInfo info;
  final CategoryState state;
  final VoidCallback onCycle;

  const _CategoryChip({
    required this.info,
    required this.state,
    required this.onCycle,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final (bg, fg, icon, tooltip) = switch (state) {
      CategoryState.allowed => (
        scheme.primaryContainer,
        scheme.onPrimaryContainer,
        Icons.check,
        'Included: only this category will be probed',
      ),
      CategoryState.blocked => (
        scheme.errorContainer,
        scheme.onErrorContainer,
        Icons.block,
        'Excluded: this category will be skipped',
      ),
      CategoryState.neutral => (
        scheme.surfaceContainerHigh,
        scheme.onSurfaceVariant,
        null,
        'All sites of this category are probed',
      ),
    };
    return Tooltip(
      message: tooltip,
      child: ActionChip(
        backgroundColor: bg,
        avatar: icon == null
            ? null
            : Icon(icon, size: 15, color: fg),
        label: Text(
          '${info.name} · ${info.count}',
          style: TextStyle(color: fg, fontSize: 11.5, fontWeight: FontWeight.w600),
        ),
        onPressed: onCycle,
      ),
    );
  }
}
