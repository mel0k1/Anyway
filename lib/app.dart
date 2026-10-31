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

  static const _destinations = [
    NavigationRailDestination(
      icon: Icon(Icons.dashboard_outlined),
      selectedIcon: Icon(Icons.dashboard),
      label: Text('Dashboard'),
    ),
    NavigationRailDestination(
      icon: Icon(Icons.radar_outlined),
      selectedIcon: Icon(Icons.radar),
      label: Text('Scan'),
    ),
    NavigationRailDestination(
      icon: Icon(Icons.fact_check_outlined),
      selectedIcon: Icon(Icons.fact_check),
      label: Text('Results'),
    ),
    NavigationRailDestination(
      icon: Icon(Icons.hub_outlined),
      selectedIcon: Icon(Icons.hub),
      label: Text('Graph'),
    ),
    NavigationRailDestination(
      icon: Icon(Icons.folder_outlined),
      selectedIcon: Icon(Icons.folder),
      label: Text('Cases'),
    ),
    NavigationRailDestination(
      icon: Icon(Icons.grid_view_outlined),
      selectedIcon: Icon(Icons.grid_view),
      label: Text('Modules'),
    ),
    NavigationRailDestination(
      icon: Icon(Icons.settings_outlined),
      selectedIcon: Icon(Icons.settings),
      label: Text('Settings'),
    ),
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
      body: Row(
        children: [
          ListenableBuilder(
            listenable: state,
            builder: (context, _) => NavigationRail(
              selectedIndex: _index,
              onDestinationSelected: (i) => setState(() => _index = i),
              extended: MediaQuery.of(context).size.width >= 1280,
              minExtendedWidth: 200,
              leading: Padding(
                padding: const EdgeInsets.only(top: 20, bottom: 16),
                child: Container(
                  width: 44,
                  height: 44,
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.primaryContainer,
                    borderRadius: BorderRadius.circular(16),
                  ),
                  child: Icon(
                    Icons.travel_explore,
                    color: Theme.of(context).colorScheme.onPrimaryContainer,
                  ),
                ),
              ),
              destinations: _destinations,
            ),
          ),
          const VerticalDivider(width: 1, thickness: 1),
          Expanded(
            child: AnimatedSwitcher(
              duration: const Duration(milliseconds: 220),
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
