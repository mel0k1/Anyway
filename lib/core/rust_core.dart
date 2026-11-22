import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'models.dart';

class RustCoreException implements Exception {
  final String message;
  RustCoreException(this.message);

  @override
  String toString() => message;
}

class RustCore {
  static DynamicLibrary? _lib;
  static bool _failed = false;

  static DynamicLibrary get lib {
    if (_lib != null) return _lib!;
    if (Platform.isWindows) {
      _lib = DynamicLibrary.open('anyway_ffi.dll');
    } else if (Platform.isMacOS) {
      _lib = DynamicLibrary.open('libanyway_ffi.dylib');
    } else if (Platform.isAndroid) {
      _lib = DynamicLibrary.open('libanyway_ffi.so');
    } else {
      _lib = DynamicLibrary.open('libanyway_ffi.so');
    }
    return _lib!;
  }

  static bool get available => !_failed && _tryOpen();

  static bool _tryOpen() {
    try {
      lib;
      return true;
    } catch (_) {
      _failed = true;
      return false;
    }
  }

  static String Function() _v0() {
    final f = lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('anyway_version');
    return () => _take(f());
  }

  static List<ModuleInfo> moduleList() => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('anyway_module_list');
        final json = _take(f());
        final map = jsonDecode(json) as Map<String, dynamic>;
        if (map['ok'] != true) throw RustCoreException(map['error'] as String? ?? 'module list failed');
        return ((map['modules'] as List?) ?? const [])
            .map((e) => ModuleInfo.fromJson((e as Map).cast<String, dynamic>()))
            .toList();
      });

  static List<CategoryInfo> categoryList() => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('anyway_module_list');
        final json = _take(f());
        final map = jsonDecode(json) as Map<String, dynamic>;
        if (map['ok'] != true) throw RustCoreException(map['error'] as String? ?? 'category list failed');
        return ((map['categories'] as List?) ?? const [])
            .map((e) => CategoryInfo.fromJson((e as Map).cast<String, dynamic>()))
            .toList();
      });

  static List<CategoryInfo> countryList() => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('anyway_module_list');
        final json = _take(f());
        final map = jsonDecode(json) as Map<String, dynamic>;
        if (map['ok'] != true) throw RustCoreException(map['error'] as String? ?? 'country list failed');
        return ((map['countries'] as List?) ?? const [])
            .map((e) => CategoryInfo.fromJson((e as Map).cast<String, dynamic>()))
            .toList();
      });

  static String? detectBrowser() => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>('anyway_browser_detect');
        final json = _take(f());
        final map = jsonDecode(json) as Map<String, dynamic>;
        if (map['ok'] != true) return null;
        return map['browser'] as String?;
      });

  static String version() => _call(() => _v0()());

  static String scanStart(
    String query, {
    int concurrency = 24,
    int timeoutSecs = 12,
    String? userAgent,
    String? proxy,
    String torMode = 'auto',
    int torSocksPort = 9050,
    String? hibpApiKey,
    List<String>? modules,
    bool pivot = false,
    int pivotDepth = 2,
    int pivotBudget = 8,
    bool longTail = false,
    bool browserChecks = false,
    List<String> allowedCategories = const <String>[],
    List<String> blockedCategories = const <String>[],
    List<String> countries = const <String>[],
  }) =>
      _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Utf8>)>(
            'anyway_scan_start');
        final body = jsonEncode({
          'query': query,
          'options': {
            'concurrency': concurrency,
            'timeout_secs': timeoutSecs,
            'user_agent': userAgent ?? '',
            'proxy': proxy ?? '',
            'tor_mode': torMode,
            'tor_socks_port': torSocksPort,
            'hibp_api_key': hibpApiKey ?? '',
            'enabled_modules': modules ?? const <String>[],
            'pivot': pivot,
            'pivot_depth': pivotDepth,
            'pivot_budget': pivotBudget,
            'long_tail': longTail,
            'browser_checks': browserChecks,
            'allowed_categories': allowedCategories,
            'blocked_categories': blockedCategories,
            'countries': countries,
          },
        });
        final ptr = body.toNativeUtf8();
        try {
          final resp = _take(f(ptr));
          final map = jsonDecode(resp) as Map<String, dynamic>;
          if (map['ok'] != true) throw RustCoreException(map['error'] as String? ?? 'scan start failed');
          return map['scanId'] as String;
        } finally {
          malloc.free(ptr);
        }
      });

  static ScanSnapshot? scanPoll(String scanId) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Utf8>)>(
            'anyway_scan_poll');
        final ptr = scanId.toNativeUtf8();
        try {
          final resp = _take(f(ptr));
          final map = jsonDecode(resp) as Map<String, dynamic>;
          if (map['ok'] != true) return null;
          return ScanSnapshot.fromJson((map['snapshot'] as Map).cast<String, dynamic>());
        } finally {
          malloc.free(ptr);
        }
      });

  static void scanCancel(String scanId) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Utf8>)>(
            'anyway_scan_cancel');
        final ptr = scanId.toNativeUtf8();
        try {
          _take(f(ptr));
        } finally {
          malloc.free(ptr);
        }
      });

  static Target scanPivot(String scanId, String nodeId) => _call(() {
        final f = lib.lookupFunction<
            Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>),
            Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>)>('anyway_scan_pivot');
        final id = scanId.toNativeUtf8();
        final node = nodeId.toNativeUtf8();
        try {
          final resp = _take(f(id, node));
          final map = jsonDecode(resp) as Map<String, dynamic>;
          if (map['ok'] != true) {
            throw RustCoreException(map['error'] as String? ?? 'pivot failed');
          }
          return Target.fromJson((map['target'] as Map).cast<String, dynamic>());
        } finally {
          malloc.free(id);
          malloc.free(node);
        }
      });

  // ---------- helpers for the newer JSON-string FFI surface ----------

  static Map<String, dynamic> _json0(String name) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(name);
        return jsonDecode(_take(f())) as Map<String, dynamic>;
      });

  static Map<String, dynamic> _json1(String name, String arg) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>), Pointer<Utf8> Function(Pointer<Utf8>)>(
            name);
        final ptr = arg.toNativeUtf8();
        try {
          return jsonDecode(_take(f(ptr))) as Map<String, dynamic>;
        } finally {
          malloc.free(ptr);
        }
      });

  static Map<String, dynamic> _json1u32(String name, String arg, int value) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>, Uint32),
            Pointer<Utf8> Function(Pointer<Utf8>, int)>(name);
        final ptr = arg.toNativeUtf8();
        try {
          return jsonDecode(_take(f(ptr, value))) as Map<String, dynamic>;
        } finally {
          malloc.free(ptr);
        }
      });

  static Map<String, dynamic> _json2u32(String name, String a, String b, int v1, int v2) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Uint32, Uint32),
            Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, int, int)>(name);
        final pa = a.toNativeUtf8();
        final pb = b.toNativeUtf8();
        try {
          return jsonDecode(_take(f(pa, pb, v1, v2))) as Map<String, dynamic>;
        } finally {
          malloc.free(pa);
          malloc.free(pb);
        }
      });

  static Map<String, dynamic> _json2(String name, String a, String b) => _call(() {
        final f = lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>),
            Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>)>(name);
        final pa = a.toNativeUtf8();
        final pb = b.toNativeUtf8();
        try {
          return jsonDecode(_take(f(pa, pb))) as Map<String, dynamic>;
        } finally {
          malloc.free(pa);
          malloc.free(pb);
        }
      });

  static Map<String, dynamic> _must(Map<String, dynamic> map, String what) {
    if (map['ok'] != true) {
      throw RustCoreException(map['error'] as String? ?? '$what failed');
    }
    return map;
  }

  // ---------- Tor ----------

  /// Returns {ok, listening, isTor, exitIp, proxy, mode} — never throws on an
  /// unreachable Tor, callers render the status from the map.
  static Map<String, dynamic> torCheck(String optionsJson) => _json1('anyway_tor_check', optionsJson);

  // ---------- Imaging ----------

  static Map<String, dynamic> imageIntel(String b64, {bool ocr = false, String langs = 'eng'}) =>
      _must(
        _json1('anyway_image_intel', jsonEncode({'b64': b64, 'ocr': ocr, 'langs': langs})),
        'image intel',
      );

  static Map<String, dynamic> ocrAvailable() => _json0('anyway_ocr_available');

  static Map<String, dynamic> reverseLinks(String imageUrl) =>
      _must(_json1('anyway_reverse_links', imageUrl), 'reverse links');

  static Map<String, dynamic> imageCompare(String hashAJson, String hashBJson) =>
      _must(_json2('anyway_image_compare', hashAJson, hashBJson), 'image compare');

  static String imageIndexAdd(String indexJson, String entryJson) =>
      _must(_json2('anyway_image_index_add', indexJson, entryJson), 'index add')['index'] as String;

  static Map<String, dynamic> imageIndexSearch(
          String indexJson, String probeJson, int threshold, int limit) =>
      _must(
        _json2u32('anyway_image_index_search', indexJson, probeJson, threshold, limit),
        'index search',
      );

  // ---------- Entity resolution ----------

  static Map<String, dynamic> resolveEntities(String snapshotJson, int threshold) =>
      _must(_json1u32('anyway_resolve_entities', snapshotJson, threshold), 'entity resolution');

  static Map<String, dynamic> buildTimeline(String snapshotJson) =>
      _must(_json1('anyway_build_timeline', snapshotJson), 'timeline');

  // ---------- Leak dumps ----------

  static Map<String, dynamic> leaksImport(String dbPath, String source, String text) => _must(
        _json1('anyway_leaks_import', jsonEncode({'db_path': dbPath, 'source': source, 'text': text})),
        'leaks import',
      );

  static Map<String, dynamic> leaksSearch(String dbPath, String query,
          {int limit = 200, bool reveal = false}) =>
      _must(
        _json1(
          'anyway_leaks_search',
          jsonEncode({'db_path': dbPath, 'query': query, 'limit': limit, 'reveal': reveal}),
        ),
        'leaks search',
      );

  static Map<String, dynamic> leaksStats(String dbPath) =>
      _must(_json1('anyway_leaks_stats', dbPath), 'leaks stats');

  static Map<String, dynamic> leaksClear(String dbPath) =>
      _must(_json1('anyway_leaks_clear', dbPath), 'leaks clear');

  // ---------- Scan scheduler ----------

  static int schedulerSet(String entriesJson) => _must(
        _json1('anyway_scheduler_set', entriesJson),
        'scheduler set',
      )['restored'] as int;

  static List<ScheduleEntryInfo> schedulerList() => _call(() {
        final map = _json0('anyway_scheduler_list');
        if (map['ok'] != true) throw RustCoreException('scheduler list failed');
        return ((map['entries'] as List?) ?? const [])
            .map((e) => ScheduleEntryInfo.fromJson((e as Map).cast<String, dynamic>()))
            .toList();
      });

  static String schedulerUpsert(String entryJson) => _must(
        _json1('anyway_scheduler_upsert', entryJson),
        'scheduler upsert',
      )['id'] as String;

  static bool schedulerRemove(String id) =>
      _json1('anyway_scheduler_remove', id)['ok'] == true;

  static void schedulerRunNow(String id) =>
      _must(_json1('anyway_scheduler_run_now', id), 'scheduler run')['ok'] == true;

  static Map<String, dynamic>? schedulerSnapshot(String id) {
    final map = _json1('anyway_scheduler_snapshot', id);
    if (map['ok'] != true) return null;
    return (map['snapshot'] as Map?)?.cast<String, dynamic>();
  }

  static String _take(Pointer<Utf8> ptr) {
    if (ptr == nullptr) throw RustCoreException('null response from native core');
    try {
      return ptr.toDartString();
    } finally {
      final fr = lib.lookupFunction<Void Function(Pointer<Utf8>), void Function(Pointer<Utf8>)>('anyway_free');
      fr(ptr);
    }
  }

  static T _call<T>(T Function() body) {
    try {
      lib;
    } catch (e) {
      _failed = true;
      throw RustCoreException('native core unavailable: $e');
    }
    return body();
  }
}
