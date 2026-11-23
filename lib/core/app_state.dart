import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'case_store.dart';
import 'models.dart';
import 'notifications.dart';
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
  String userAgent =
      'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36';
  String proxy = '';
  String torMode = 'auto';
  int torSocksPort = 9050;
  String hibpApiKey = '';
  String vtApiKey = '';
  bool embeddedTor = true;
  Set<String> disabledModules = {};
  bool pivotEnabled = true;
  int pivotDepth = 2;
  int pivotBudget = 8;
  bool longTail = false;
  bool browserChecks = false;
  Set<String> allowedCategories = {};
  Set<String> blockedCategories = {};
  List<CategoryInfo> categories = [];
  List<CategoryInfo> countries = [];
  Set<String> selectedCountries = {};
  String? detectedBrowser;
  TorStatus? torStatus;
  Map<String, dynamic> artiStatus = {};
  Map<String, dynamic>? ocrInfo;

  /// Persistent entity store (SQLite) fed by every finished scan.
  String estorePath = '';

  /// Planner events: scheduled runs that surfaced new findings.
  List<Map<String, dynamic>> plannerJournal = [];
  final Map<String, int> _lastNewFindings = {};
  Timer? _artiTimer;

  ScanSnapshot? activeScan;
  ScanSnapshot? lastSnapshot;
  Timer? _pollTimer;
  List<CaseMeta> caseList = [];

  List<ScheduleEntryInfo> scheduleEntries = [];
  Timer? _scheduleTimer;
  File? _scheduleFile;
  String? _lastScheduleSig;

  bool get darkTheme => _darkTheme;

  List<ModuleInfo> modulesFor(TargetKind kind) => modules
      .where((m) => m.accepts.contains(kind) && !disabledModules.contains(m.id))
      .toList();

  Future<void> init() async {
    final prefs = await SharedPreferences.getInstance();
    _darkTheme = prefs.getBool('darkTheme') ?? true;
    concurrency = prefs.getInt('concurrency') ?? 24;
    timeoutSecs = prefs.getInt('timeoutSecs') ?? 12;
    userAgent = prefs.getString('userAgent') ?? userAgent;
    proxy = prefs.getString('proxy') ?? '';
    torMode = prefs.getString('torMode') ?? 'auto';
    torSocksPort = prefs.getInt('torSocksPort') ?? 9050;
    hibpApiKey = prefs.getString('hibpApiKey') ?? '';
    vtApiKey = prefs.getString('vtApiKey') ?? '';
    embeddedTor = prefs.getBool('embeddedTor') ?? true;
    plannerJournal = _loadJournal(prefs);
    disabledModules = (prefs.getStringList('disabledModules') ?? []).toSet();
    pivotEnabled = prefs.getBool('pivotEnabled') ?? true;
    pivotDepth = prefs.getInt('pivotDepth') ?? 2;
    pivotBudget = prefs.getInt('pivotBudget') ?? 8;
    longTail = prefs.getBool('longTail') ?? false;
    browserChecks = prefs.getBool('browserChecks') ?? false;
    allowedCategories = (prefs.getStringList('allowedCategories') ?? [])
        .toSet();
    blockedCategories = (prefs.getStringList('blockedCategories') ?? [])
        .toSet();
    selectedCountries = (prefs.getStringList('selectedCountries') ?? [])
        .toSet();

    try {
      modules = RustCore.moduleList();
      categories = RustCore.categoryList();
      countries = RustCore.countryList();
      detectedBrowser = RustCore.detectBrowser();
      nativeReady = true;
    } catch (e) {
      nativeReady = false;
      nativeError = '$e';
    }
    caseList = await cases.list();
    notifyListeners();
    if (nativeReady) {
      await _initScheduler();
      await _initEstore();
      unawaited(checkTor());
      _scheduleTimer = Timer.periodic(
        const Duration(seconds: 3),
        (_) => _refreshSchedule(),
      );
      _artiTimer = Timer.periodic(
        const Duration(seconds: 10),
        (_) => _refreshArti(),
      );
      if (embeddedTor) unawaited(startArti());
      try {
        ocrInfo = RustCore.ocrAvailable();
      } catch (_) {}
      notifyListeners();
    }
  }

  Future<void> _initEstore() async {
    try {
      final base = await getApplicationSupportDirectory();
      estorePath = '${base.path}${Platform.pathSeparator}entities.sqlite';
    } catch (_) {}
  }

  Future<void> startArti() async {
    if (!nativeReady || !embeddedTor) return;
    try {
      final base = await getApplicationSupportDirectory();
      artiStatus = RustCore.artiStart(
        9051,
        '${base.path}${Platform.pathSeparator}arti-data',
      );
      notifyListeners();
    } catch (_) {}
  }

  void _refreshArti() {
    if (!nativeReady) return;
    try {
      final next = RustCore.artiStatus();
      if (next['status'] != artiStatus['status'] ||
          next['connections'] != artiStatus['connections']) {
        artiStatus = next;
        notifyListeners();
      }
    } catch (_) {}
  }

  // ---------- Tor ----------

  String _torOptionsJson() => jsonEncode({
    'tor_mode': torMode,
    'tor_socks_port': torSocksPort,
    'proxy': proxy,
    'embedded_tor': embeddedTor,
  });

  Future<void> checkTor() async {
    if (!nativeReady) return;
    try {
      torStatus = TorStatus.fromJson(RustCore.torCheck(_torOptionsJson()));
    } catch (_) {
      torStatus = null;
    }
    notifyListeners();
  }

  Future<void> setTorMode(String v) async {
    torMode = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('torMode', v);
    notifyListeners();
    await checkTor();
  }

  Future<void> setTorSocksPort(int v) async {
    torSocksPort = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setInt('torSocksPort', v);
    notifyListeners();
    await checkTor();
  }

  Future<void> setEmbeddedTor(bool v) async {
    embeddedTor = v;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool('embeddedTor', v);
    notifyListeners();
    if (v) {
      await startArti();
    }
    await checkTor();
  }

  Future<void> setVtApiKey(String v) async {
    vtApiKey = v.trim();
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('vtApiKey', vtApiKey);
    notifyListeners();
  }

  // ---------- Scan scheduler ----------

  Future<void> _initScheduler() async {
    try {
      final base = await getApplicationSupportDirectory();
      final file = File('${base.path}${Platform.pathSeparator}scheduler.json');
      _scheduleFile = file;
      if (await file.exists()) {
        final raw = await file.readAsString();
        if (raw.trim().isNotEmpty) {
          RustCore.schedulerSet(raw);
        }
      }
    } catch (_) {}
    await _refreshSchedule();
  }

  Future<void> _refreshSchedule() async {
    if (!nativeReady) return;
    try {
      final next = RustCore.schedulerList();
      final sig = next
          .map(
            (x) =>
                '${x.id}:${x.enabled ? 1 : 0}:${x.running ? 1 : 0}:${x.runs}:'
                '${x.lastStatus ?? '-'}:${x.newFindings ?? -1}:${x.findingsTotal}:'
                '${(x.dueIn / 30).floor()}',
          )
          .join('|');
      if (sig == _lastScheduleSig) return;
      _lastScheduleSig = sig;
      scheduleEntries = next;
      for (final e in next) {
        final added = e.addedFindings.length;
        final previous = _lastNewFindings[e.id];
        if (added > 0 && previous != added) {
          _addJournalRecord(e);
          await notifyOs(
            'Planner: new findings',
            '${e.query}: $added new finding${added == 1 ? '' : 's'}',
          );
        }
        _lastNewFindings[e.id] = added;
      }
      notifyListeners();
    } catch (_) {}
  }

  void _addJournalRecord(ScheduleEntryInfo e) {
    plannerJournal = [
      {
        'ts': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        'id': e.id,
        'query': e.query,
        'added': e.addedFindings.take(10).toList(),
        'total': e.addedFindings.length,
      },
      ...plannerJournal,
    ].take(50).toList();
    () async {
      try {
        final prefs = await SharedPreferences.getInstance();
        await prefs.setString('plannerJournal', jsonEncode(plannerJournal));
      } catch (_) {}
    }();
  }

  List<Map<String, dynamic>> _loadJournal(SharedPreferences prefs) {
    try {
      final raw = prefs.getString('plannerJournal');
      if (raw == null || raw.isEmpty) return [];
      return ((jsonDecode(raw) as List?) ?? const [])
          .map((e) => (e as Map).cast<String, dynamic>())
          .toList();
    } catch (_) {
      return [];
    }
  }

  Future<void> clearJournal() async {
    plannerJournal = [];
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('plannerJournal', '[]');
    notifyListeners();
  }

  Future<void> _persistSchedule() async {
    if (_scheduleFile == null) return;
    try {
      final list = scheduleEntries
          .map((e) => jsonEncode(e.toPersisted()))
          .toList();
      await _scheduleFile!.writeAsString('[${list.join(',')}]');
    } catch (_) {}
  }

  Future<void> addSchedule(
    String query,
    int intervalSecs, {
    bool enabled = true,
  }) async {
    RustCore.schedulerUpsert(
      jsonEncode({
        'query': query,
        'interval_secs': intervalSecs,
        'enabled': enabled,
      }),
    );
    await _refreshSchedule();
    await _persistSchedule();
  }

  Future<void> setScheduleEnabled(ScheduleEntryInfo e, bool enabled) async {
    RustCore.schedulerUpsert(
      jsonEncode({
        'id': e.id,
        'query': e.query,
        'interval_secs': e.intervalSecs,
        'enabled': enabled,
      }),
    );
    await _refreshSchedule();
    await _persistSchedule();
  }

  Future<void> removeSchedule(String id) async {
    RustCore.schedulerRemove(id);
    await _refreshSchedule();
    await _persistSchedule();
  }

  Future<void> runScheduleNow(String id) async {
    RustCore.schedulerRunNow(id);
    await _refreshSchedule();
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

  Future<void> toggleCountry(String name, bool selected) async {
    if (selected) {
      selectedCountries.add(name);
    } else {
      selectedCountries.remove(name);
    }
    final prefs = await SharedPreferences.getInstance();
    await prefs.setStringList('selectedCountries', selectedCountries.toList());
    notifyListeners();
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
    // free the previous finished scan's native snapshot before replacing it
    final previousId = activeScan?.scanId;
    if (previousId != null && activeScan?.running != true) {
      try {
        RustCore.scanRelease(previousId);
      } catch (_) {}
    }
    final enabled = modules
        .where((m) => !disabledModules.contains(m.id))
        .map((m) => m.id)
        .toList();
    final scanId = RustCore.scanStart(
      query,
      concurrency: concurrency,
      timeoutSecs: timeoutSecs,
      userAgent: userAgent,
      proxy: proxy,
      torMode: torMode,
      torSocksPort: torSocksPort,
      hibpApiKey: hibpApiKey,
      vtApiKey: vtApiKey,
      embeddedTor: embeddedTor,
      modules: enabled,
      pivot: pivotEnabled,
      pivotDepth: pivotDepth,
      pivotBudget: pivotBudget,
      longTail: longTail,
      browserChecks: browserChecks,
      allowedCategories: allowedCategories.toList(),
      blockedCategories: blockedCategories.toList(),
      countries: selectedCountries.toList(),
    );
    activeScan = ScanSnapshot(
      scanId: scanId,
      target: Target(
        kind: detectTargetKind(query),
        raw: query,
        normalized: query.trim().toLowerCase(),
      ),
      status: 'running',
      startedAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      modules: const [],
      findings: const [],
      log: const [],
    );
    notifyListeners();
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(
      const Duration(milliseconds: 400),
      (_) => _poll(),
    );
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
      unawaited(_ingestEntities(snap));
    }
    notifyListeners();
  }

  Future<void> _ingestEntities(ScanSnapshot snap) async {
    if (estorePath.isEmpty || snap.status != 'done') return;
    try {
      RustCore.estoreIngest(estorePath, snap.scanId, jsonEncode(snap.toJson()));
    } catch (_) {}
  }

  Future<void> cancelScan() async {
    final id = activeScan?.scanId;
    if (id == null) return;
    RustCore.scanCancel(id);
  }

  Future<void> pivotNode(String nodeId) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to pivot');
    if (snap.status == 'running') throw StateError('scan is still running');
    final target = RustCore.scanPivot(snap.scanId, nodeId);
    activeScan = ScanSnapshot(
      scanId: snap.scanId,
      target: snap.target,
      targets: snap.targets,
      status: 'running',
      startedAt: snap.startedAt,
      modules: snap.modules,
      findings: snap.findings,
      graph: snap.graph,
      pivots: snap.pivots,
      log: [...snap.log, 'manual pivot: + ${target.raw}'],
    );
    lastSnapshot = null;
    notifyListeners();
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(
      const Duration(milliseconds: 400),
      (_) => _poll(),
    );
    _poll();
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

  /// Correlation clusters + timeline from the native core; empty in mock mode.
  CorrelationSection? _correlationSection(ScanSnapshot snap) {
    if (!nativeReady) return null;
    try {
      final json = jsonEncode(snap.toJson());
      final report = RustCore.resolveEntities(json, 45);
      final timeline = RustCore.buildTimeline(json);
      return CorrelationSection(
        clusters: ((report['clusters'] as List?) ?? const [])
            .map((e) => (e as Map).cast<String, dynamic>())
            .where((c) => ((c['size'] as num?)?.toInt() ?? 0) > 1)
            .toList(),
        timeline: ((timeline['events'] as List?) ?? const [])
            .map((e) => (e as Map).cast<String, dynamic>())
            .toList(),
      );
    } catch (_) {
      return null;
    }
  }

  Future<File> exportLastMarkdown([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    return ReportExporter.exportMarkdown(
      snap,
      dir,
      _correlationSection(snap) ?? const CorrelationSection(),
    );
  }

  Future<File> exportLastJson([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    return ReportExporter.exportJson(snap, dir);
  }

  Future<File> exportLastHtml([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    return ReportExporter.exportHtml(
      snap,
      dir,
      _correlationSection(snap) ?? const CorrelationSection(),
    );
  }

  Future<File> exportLastPdf([Directory? dir]) async {
    final snap = lastSnapshot ?? activeScan;
    if (snap == null) throw StateError('no scan to export');
    if (!nativeReady) throw StateError('PDF export needs the native core');
    final res = RustCore.reportPdf(jsonEncode(snap.toJson()));
    final bytes = base64Decode((res['pdf_b64'] as String?) ?? '');
    if (bytes.isEmpty) throw StateError('empty PDF from native core');
    return ReportExporter.exportPdf(snap, bytes, dir);
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _scheduleTimer?.cancel();
    _artiTimer?.cancel();
    super.dispose();
  }
}
