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

  static String version() => _call(() => _v0()());

  static String scanStart(
    String query, {
    int concurrency = 24,
    int timeoutSecs = 12,
    String? userAgent,
    String? proxy,
    String? hibpApiKey,
    List<String>? modules,
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
            'hibp_api_key': hibpApiKey ?? '',
            'enabled_modules': modules ?? const <String>[],
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
