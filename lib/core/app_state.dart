import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'case_store.dart';
import 'models.dart';
import 'report_exporter.dart';
import 'rust_core.dart';

enum CategoryState { neutral, allowed, blocked }

class AppState extends ChangeNotifier {
  final CaseStore cases = CaseStore();

  List<ModuleInfo> modules = [];
  bool nativeReady = false;
  String nativeError = '';

  bool _darkTheme = true;
  int concurrency = 24;
  int timeoutSecs = 12;
  String userAgent = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36';
  String proxy = '';
  String hibpApiKey = '';
  Set<String> disabledModules = {};
  bool pivotEnabled = true;
  int pivotDepth = 2;
  int pivotBudget = 8;
  bool longTail = false;
  bool browserChecks = false;
  Set<String> allowedCategories = {};
  Set<String> blockedCategories = {};
  List<CategoryInfo> categories = [];
  String? detectedBrowser;

  ScanSnapshot? activeScan;
  ScanSnapshot? lastSnapshot;
  Timer? _pollTimer;
  List<CaseMeta> caseList = [];

  bool get darkTheme => _darkTheme;

  List<ModuleInfo> modulesFor(TargetKind kind) =>
      modules.where((m) => m.accepts.contains(kind) && !disabledModules.contains(m.id)).toList();

  Future<void> init() async {
    final prefs = await SharedPreferences.getInstance();
    _darkTheme = prefs.getBool('darkTheme') ?? true;
    concurrency = prefs.getInt('concurrency') ?? 24;
    timeoutSecs = prefs.getInt('timeoutSecs') ?? 12;
    userAgent = prefs.getString('userAgent') ?? userAgent;
    proxy = prefs.getString('proxy') ?? '';
    hibpApiKey = prefs.getString('hibpApiKey') ?? '';
    disabledModules = (prefs.getStringList('disabledModules') ?? []).toSet();
    pivotEnabled = prefs.getBool('pivotEnabled') ?? true;
    pivotDepth = prefs.getInt('pivotDepth') ?? 2;
    pivotBudget = prefs.getInt('pivotBudget') ?? 8;
    longTail = prefs.getBool('longTail') ?? false;
    browserChecks = prefs.getBool('browserChecks') ?? false;
    allowedCategories = (prefs.getStringList('allowedCategories') ?? []).toSet();
    blockedCategories = (prefs.getStringList('blockedCategories') ?? []).toSet();

    try {
      modules = RustCore.moduleList();
      categories = RustCore.categoryList();
      detectedBrowser = RustCore.detectBrowser();
      nativeReady = true;
    } catch (e) {
      nativeReady = false;
      nativeError = '$e';
    }
    caseList = await cases.list();
    notifyListeners();
  }

  Future<void> setDarkTheme(bool v) async {
    _darkTheme = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('darkTheme', v);
    notifyListeners();
  }

  Future<void> setConcurrency(int v) async {
    concurrency = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt('concurrency', v);
    notifyListeners();
  }

  Future<void> setTimeoutSecs(int v) async {
    timeoutSecs = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt('timeoutSecs', v);
    notifyListeners();
  }

  Future<void> setUserAgent(String v) async {
    userAgent = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('userAgent', v);
    notifyListeners();
  }

  Future<void> setProxy(String v) async {
    proxy = v.trim();
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('proxy', proxy);
    notifyListeners();
  }

  Future<void> setHibpApiKey(String v) async {
    hibpApiKey = v.trim();
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('hibpApiKey', hibpApiKey);
    notifyListeners();
  }

  Future<void> toggleModule(String id, bool enabled) async {
    if (enabled) {
      disabledModules.remove(id);
    } else {
      disabledModules.add(id);
    }
    final prefs = await SharedPreferences.getInstance();
    await prefs.setStringList('disabledModules', disabledModules.toList());
    notifyListeners();
  }

  Future<void> setPivotEnabled(bool v) async {
    pivotEnabled = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('pivotEnabled', v);
    notifyListeners();
  }

  Future<void> setPivotDepth(int v) async {
    pivotDepth = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt('pivotDepth', v);
    notifyListeners();
  }

  Future<void> setPivotBudget(int v) async {
    pivotBudget = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt('pivotBudget', v);
    notifyListeners();
  }

  Future<void> setLongTail(bool v) async {
    longTail = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('longTail', v);
    notifyListeners();
  }

  Future<void> setBrowserChecks(bool v) async {
    browserChecks = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('browserChecks', v);
    notifyListeners();
  }

  Future<void> setCategoryState(String name, CategoryState state) async {
    allowedCategories.remove(name);
    blockedCategories.remove(name);
    if (state == CategoryState.allowed) {
      allowedCategories.add(name);
    } else if (state == CategoryState.blocked) {
      blockedCategories.add(name);
    }
    final prefs = await SharedPreferences.getInstance();
    await prefs.setStringList('allowedCategories', allowedCategories.toList());
    await prefs.setStringList('blockedCategories', blockedCategories.toList());
    notifyListeners();
  }

  CategoryState categoryState(String name) {
    if (allowedCategories.contains(name)) return CategoryState.allowed;
    if (blockedCategories.contains(name)) return CategoryState.blocked;
    return CategoryState.neutral;
  }

  void resetCategories() {
    allowedCategories.clear();
    blockedCategories.clear();
    () async {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setStringList('allowedCategories', []);
      await prefs.setStringList('blockedCategories', []);
      notifyListeners();
    }();
    notifyListeners();
  }

  void startScan(String query) {
    final enabled = modules.where((m) => !disabledModules.contains(m.id)).map((m) => m.id).toList();
    final scanId = RustCore.scanStart(
      query,
      concurrency: concurrency,
      timeoutSecs: timeoutSecs,
      userAgent: userAgent,
      proxy: proxy,
      hibpApiKey: hibpApiKey,
      modules: enabled,
      pivot: pivotEnabled,
      pivotDepth: pivotDepth,
      pivotBudget: pivotBudget,
      longTail: longTail,
      browserChecks: browserChecks,
      allowedCategories: allowedCategories.toList(),
      blockedCategories: blockedCategories.toList(),
    );
    activeScan = ScanSnapshot(
      scanId: scanId,
      target: Target(kind: detectTargetKind(query), raw: query, normalized: query.trim().toLowerCase()),
      status: 'running',
      startedAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      modules: const [],
      findings: const [],
      log: const [],
    );
    notifyListeners();
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(const Duration(milliseconds: 400), (_) => _poll());
    _poll();
  }

  Future<void> _poll() async {
    final id = activeScan?.scanId;
    if (id == null) return;
    final snap = RustCore.scanPoll(id);
    if (snap == null) return;
    activeScan = snap;
    if (!snap.running) {
      _pollTimer?.cancel();
      _pollTimer = null;
      lastSnapshot = snap;
      try {
        await cases.save(snap);
        caseList = await cases.list();
      } catch (_) {}
    }
    notifyListeners();
  }

  Future<void> cancelScan() async {
    final id = activeScan?.scanId;
    if (id == null) return;
    RustCore.scanCancel(id);
  }

  Future<void> openCase(CaseMeta meta) async {
    final snap = await cases.load(meta.id);
    if (snap != null) {
      lastSnapshot = snap;
      notifyListeners();
    }
  }

  Future<void> deleteCase(CaseMeta meta) async {
    await cases.delete(meta.id);
    caseList = await cases.list();
    notifyListeners();
  }

  Future<File> exportLastMarkdown([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    return ReportExporter.exportMarkdown(snap, dir);
  }

  Future<File> exportLastJson([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    return ReportExporter.exportJson(snap, dir);
  }

  Future<File> exportLastHtml([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    return ReportExporter.exportHtml(snap, dir);
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }
}
