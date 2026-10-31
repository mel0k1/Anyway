import 'dart:async';

// ignore_for_file: avoid_print
import 'package:anyway/core/models.dart';
import 'package:anyway/core/rust_core.dart';

Future<void> main() async {
  print('core version: ${RustCore.version()}');
  final modules = RustCore.moduleList();
  print('modules: ${modules.map((m) => m.id).join(', ')}');

  final scanId = RustCore.scanStart('example.com', concurrency: 12, timeoutSecs: 10);
  print('scan started: $scanId');

  ScanSnapshot snap;
  while (true) {
    await Future<void>.delayed(const Duration(milliseconds: 500));
    final s = RustCore.scanPoll(scanId);
    if (s == null) {
      print('poll failed');
      return;
    }
    snap = s;
    if (!snap.running) break;
  }

  print('status: ${snap.status} · target: ${snap.target.kind.wire} · findings: ${snap.findings.length}');
  print(snap.modules.map((m) => '${m.id}:${m.status}:${m.found}').join(' | '));
  for (final f in snap.findings) {
    final value = f.value.length > 90 ? f.value.substring(0, 90) : f.value;
    print(' - ${f.moduleId} | ${f.label} | $value');
  }
  RustCore.scanCancel(scanId);
}
