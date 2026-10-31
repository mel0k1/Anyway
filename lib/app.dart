import 'package:flutter/material.dart';

import 'core/app_state.dart';
import 'ui/screens/cases_screen.dart';
import 'ui/screens/dashboard_screen.dart';
import 'ui/screens/graph_screen.dart';
import 'ui/screens/modules_screen.dart';
import 'ui/screens/results_screen.dart';
import 'ui/screens/scan_screen.dart';
import 'ui/screens/settings_screen.dart';
import 'ui/theme/app_theme.dart';

class AnywayApp extends StatelessWidget {
  final AppState state;
  final int initialIndex;

  const AnywayApp({super.key, required this.state, this.initialIndex = 0});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: state,
      builder: (context, _) => MaterialApp(
        title: 'Anyway',
        debugShowCheckedModeBanner: false,
        theme: AppTheme.light(),
        darkTheme: AppTheme.dark(),
        themeMode: state.darkTheme ? ThemeMode.dark : ThemeMode.light,
        home: Shell(state: state, initialIndex: initialIndex),
      ),
    );
  }
}

class Shell extends StatefulWidget {
  final AppState state;
  final int initialIndex;

  const Shell({super.key, required this.state, this.initialIndex = 0});

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  late int _index = widget.initialIndex;

  static const _sections = [
    (Icons.dashboard_outlined, Icons.dashboard, 'Dashboard'),
    (Icons.radar_outlined, Icons.radar, 'Scan'),
    (Icons.fact_check_outlined, Icons.fact_check, 'Results'),
    (Icons.hub_outlined, Icons.hub, 'Graph'),
    (Icons.folder_outlined, Icons.folder, 'Cases'),
    (Icons.grid_view_outlined, Icons.grid_view, 'Modules'),
    (Icons.settings_outlined, Icons.settings, 'Settings'),
  ];

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final screens = <Widget>[
      DashboardScreen(
        state: state,
        onStartScan: () => setState(() => _index = 1),
      ),
      ScanScreen(state: state, onOpenResults: () => setState(() => _index = 2)),
      ResultsScreen(
        state: state,
        onOpenGraph: () => setState(() => _index = 3),
      ),
      GraphScreen(state: state),
      CasesScreen(state: state, onOpenCase: () => setState(() => _index = 2)),
      ModulesScreen(state: state),
      SettingsScreen(state: state),
    ];

    return Scaffold(
      body: Column(
        children: [
          _TopBar(
            index: _index,
            dark: state.darkTheme,
            onSelect: (i) => setState(() => _index = i),
            onToggleTheme: () => state.setDarkTheme(!state.darkTheme),
          ),
          Expanded(
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 260),
              switchInCurve: Curves.easeOutCubic,
              switchOutCurve: Curves.easeInCubic,
              transitionBuilder: (child, anim) => FadeTransition(
                opacity: anim,
                child: ScaleTransition(
                  scale: Tween(begin: 0.985, end: 1.0).animate(anim),
                  child: child,
                ),
              ),
              layoutBuilder: (currentChild, previousChildren) => Stack(
                fit: StackFit.expand,
                alignment: Alignment.topCenter,
                children: [...previousChildren, ?currentChild],
              ),
              child: KeyedSubtree(
                key: ValueKey(_index),
                child: screens[_index],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _TopBar extends StatelessWidget {
  final int index;
  final bool dark;
  final ValueChanged<int> onSelect;
  final VoidCallback onToggleTheme;

  const _TopBar({
    required this.index,
    required this.dark,
    required this.onSelect,
    required this.onToggleTheme,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final wide = MediaQuery.of(context).size.width >= 1240;

    return Container(
      decoration: BoxDecoration(
        color: scheme.surfaceContainer,
        borderRadius: const BorderRadius.vertical(bottom: Radius.circular(20)),
        boxShadow: [
          BoxShadow(
            color: scheme.shadow.withValues(alpha: 0.08),
            offset: const Offset(0, 2),
            blurRadius: 14,
          ),
        ],
      ),
      child: SafeArea(
        bottom: false,
        child: SizedBox(
          height: 64,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(20, 0, 10, 0),
            child: Row(
              children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(12),
                  child: Image.asset(
                    'assets/brand/icon-256.png',
                    width: 36,
                    height: 36,
                    fit: BoxFit.cover,
                  ),
                ),
                const SizedBox(width: 12),
                if (wide)
                  Text(
                    'Anyway',
                    style: textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.w800,
                      letterSpacing: -0.2,
                    ),
                  ),
                const Spacer(),
                Flexible(
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    reverse: true,
                    padding: const EdgeInsets.symmetric(vertical: 11),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        for (final (i, section) in _ShellState._sections.indexed)
                          _NavPill(
                            icon: section.$1,
                            activeIcon: section.$2,
                            label: section.$3,
                            selected: i == index,
                            onTap: () => onSelect(i),
                          ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(width: 4),
                IconButton(
                  tooltip: dark ? 'Switch to light theme' : 'Switch to dark theme',
                  onPressed: onToggleTheme,
                  icon: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 250),
                    transitionBuilder: (child, anim) =>
                        ScaleTransition(scale: anim, child: child),
                    child: Icon(
                      dark ? Icons.light_mode_outlined : Icons.dark_mode_outlined,
                      key: ValueKey(dark),
                      size: 20,
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _NavPill extends StatelessWidget {
  final IconData icon;
  final IconData activeIcon;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  const _NavPill({
    required this.icon,
    required this.activeIcon,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 3),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(22),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 420),
          curve: Curves.easeOutBack,
          padding: EdgeInsets.symmetric(
            horizontal: selected ? 16 : 13,
            vertical: 10,
          ),
          decoration: BoxDecoration(
            color: selected
                ? scheme.secondaryContainer
                : Colors.transparent,
            borderRadius: BorderRadius.circular(22),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              AnimatedSwitcher(
                duration: const Duration(milliseconds: 260),
                transitionBuilder: (child, anim) =>
                    ScaleTransition(scale: anim, child: child),
                child: Icon(
                  selected ? activeIcon : icon,
                  key: ValueKey(selected),
                  size: 20,
                  color: selected
                      ? scheme.onSecondaryContainer
                      : scheme.onSurfaceVariant,
                ),
              ),
              AnimatedSize(
                duration: const Duration(milliseconds: 300),
                curve: Curves.easeOutCubic,
                alignment: Alignment.centerLeft,
                child: selected
                    ? Padding(
                        padding: const EdgeInsets.only(left: 8),
                        child: Text(
                          label,
                          style: TextStyle(
                            fontWeight: FontWeight.w700,
                            fontSize: 13,
                            color: scheme.onSecondaryContainer,
                          ),
                        ),
                      )
                    : const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
