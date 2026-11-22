import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../core/models.dart';
import '../../core/rust_core.dart';

/// Visual recon: perceptual hashes, local CBIR index, OCR and deep links to
/// reverse image search engines.
class ImagesScreen extends StatefulWidget {
  const ImagesScreen({super.key});

  @override
  State<ImagesScreen> createState() => _ImagesScreenState();
}

class _ImagesScreenState extends State<ImagesScreen> {
  ImageHashInfo? _hashes;
  String? _ocrText;
  String? _lastFileName;
  bool _busy = false;
  bool _runOcr = true;
  String _langs = 'eng';
  Map<String, dynamic>? _ocrInfo;
  List<Map<String, dynamic>> _index = [];
  final _urlController = TextEditingController();
  List<Map<String, dynamic>> _matches = [];
  File? _indexFile;

  @override
  void initState() {
    super.initState();
    _loadIndex();
    _loadOcrInfo();
  }

  Future<void> _loadIndex() async {
    try {
      final base = await getApplicationSupportDirectory();
      final file = File('${base.path}${Platform.pathSeparator}image_index.json');
      _indexFile = file;
      if (await file.exists()) {
        final raw = await file.readAsString();
        if (raw.trim().isNotEmpty) {
          final v = jsonDecode(raw);
          if (v is List) {
            _index = v.cast<Map<String, dynamic>>();
          }
        }
      }
      if (mounted) setState(() {});
    } catch (_) {}
  }

  Future<void> _persistIndex() async {
    if (_indexFile == null) return;
    try {
      await _indexFile!.writeAsString(jsonEncode(_index));
    } catch (_) {}
  }

  Future<void> _loadOcrInfo() async {
    try {
      _ocrInfo = RustCore.ocrAvailable();
      if (mounted) setState(() {});
    } catch (_) {}
  }

  Future<void> _pickImage() async {
    final messenger = ScaffoldMessenger.of(context);
    const group = XTypeGroup(
      label: 'images',
      extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff'],
    );
    try {
      final file = await openFile(acceptedTypeGroups: [group]);
      if (file == null) return;
      setState(() => _busy = true);
      final bytes = await file.readAsBytes();
      final b64 = base64Encode(bytes);
      final intel = RustCore.imageIntel(b64, ocr: _runOcr, langs: _langs);
      setState(() {
        _hashes = ImageHashInfo.fromJson(intel);
        _ocrText = _hashes?.ocr?['text'] as String?;
        _lastFileName = file.name;
        _matches = [];
      });
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('Image analysis failed: $e')));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _addToIndex() async {
    final h = _hashes;
    if (h == null) return;
    final messenger = ScaffoldMessenger.of(context);
    try {
      final entry = {
        'label': _lastFileName ?? 'image',
        'phash': h.phash,
        'dhash': h.dhash,
        'ahash': h.ahash,
        'addedAt': DateTime.now().millisecondsSinceEpoch ~/ 1000,
      };
      final newIndex = RustCore.imageIndexAdd(jsonEncode(_index), jsonEncode(entry));
      setState(() {
        _index = (jsonDecode(newIndex) as List).cast<Map<String, dynamic>>();
      });
      await _persistIndex();
      messenger.showSnackBar(SnackBar(content: Text('Added to local index (${_index.length} images)')));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e')));
    }
  }

  void _searchIndex() {
    final h = _hashes;
    if (h == null) return;
    final messenger = ScaffoldMessenger.of(context);
    try {
      final res = RustCore.imageIndexSearch(
        jsonEncode(_index),
        jsonEncode(h.toJson()),
        10,
        20,
      );
      setState(() {
        _matches = ((res['matches'] as List?) ?? const [])
            .map((e) => (e as Map).cast<String, dynamic>())
            .toList();
      });
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e')));
    }
  }

  Future<void> _openReverseLinks() async {
    final messenger = ScaffoldMessenger.of(context);
    final url = _urlController.text.trim();
    if (url.isEmpty) return;
    try {
      final res = RustCore.reverseLinks(url);
      final engines = ((res['engines'] as List?) ?? const [])
          .map((e) => (e as Map).cast<String, dynamic>())
          .toList();
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Reverse image search'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final e in engines)
                ListTile(
                  leading: const Icon(Icons.open_in_new, size: 18),
                  title: Text(e['name'] as String),
                  onTap: () => launchUrl(Uri.parse(e['url'] as String), mode: LaunchMode.externalApplication),
                ),
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  res['note'] as String? ?? '',
                  style: const TextStyle(fontSize: 11),
                ),
              ),
            ],
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('Close')),
          ],
        ),
      );
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final ocrOk = _ocrInfo?['available'] as bool? ?? false;

    return Padding(
      padding: const EdgeInsets.all(32),
      child: ListView(
        children: [
          Text('Visual recon', style: textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w800)),
          const SizedBox(height: 6),
          Text(
            'Perceptual hashes (pHash / dHash / aHash) identify the same picture across re-encodes and crops, the local index matches images you already analysed, OCR pulls text out of screenshots, and reverse-search deep links hand the image URL to external engines',
            style: TextStyle(color: scheme.onSurfaceVariant),
          ),
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      FilledButton.icon(
                        onPressed: _busy ? null : _pickImage,
                        icon: const Icon(Icons.image_search),
                        label: const Text('Analyse image'),
                      ),
                      const SizedBox(width: 16),
                      Row(
                        children: [
                          Switch(
                            value: _runOcr,
                            onChanged: (v) => setState(() => _runOcr = v),
                          ),
                          const Text('Run OCR'),
                        ],
                      ),
                      const SizedBox(width: 16),
                      SizedBox(
                        width: 120,
                        child: TextField(
                          controller: TextEditingController(text: _langs),
                          decoration: const InputDecoration(
                            labelText: 'OCR langs',
                            helperText: 'eng, rus, eng+rus…',
                          ),
                          onSubmitted: (v) => setState(() => _langs = v.trim()),
                        ),
                      ),
                      const Spacer(),
                      Row(
                        children: [
                          Icon(
                            ocrOk ? Icons.check_circle : Icons.warning_amber,
                            size: 18,
                            color: ocrOk ? scheme.primary : scheme.onSurfaceVariant,
                          ),
                          const SizedBox(width: 6),
                          Text(
                            ocrOk
                                ? 'tesseract ready (${((_ocrInfo?['langs'] as List?) ?? const []).length} langs)'
                                : 'tesseract not found on PATH',
                            style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12),
                          ),
                        ],
                      ),
                    ],
                  ),
                  if (_busy) const Padding(padding: EdgeInsets.all(12), child: CircularProgressIndicator()),
                  if (_hashes != null) ...[
                    const SizedBox(height: 16),
                    Text(
                      '${_lastFileName ?? 'image'} · ${_hashes!.width}x${_hashes!.height} · ${_hashes!.format}',
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    const SizedBox(height: 10),
                    Wrap(
                      spacing: 10,
                      runSpacing: 8,
                      children: [
                        _HashChip(label: 'pHash', value: _hashes!.phash, color: scheme.primary),
                        _HashChip(label: 'dHash', value: _hashes!.dhash, color: scheme.secondary),
                        _HashChip(label: 'aHash', value: _hashes!.ahash, color: scheme.tertiary),
                      ],
                    ),
                    const SizedBox(height: 14),
                    Wrap(
                      spacing: 10,
                      children: [
                        OutlinedButton.icon(
                          onPressed: _addToIndex,
                          icon: const Icon(Icons.playlist_add),
                          label: Text('Add to index (${_index.length})'),
                        ),
                        OutlinedButton.icon(
                          onPressed: _searchIndex,
                          icon: const Icon(Icons.compare),
                          label: const Text('Find similar in index'),
                        ),
                      ],
                    ),
                    if (_matches.isNotEmpty) ...[
                      const SizedBox(height: 12),
                      for (final m in _matches)
                        ListTile(
                          dense: true,
                          leading: const Icon(Icons.compare_arrows, size: 18),
                          title: Text('${m['entry']?['label'] ?? '?'}'),
                          subtitle: Text(
                            'pHash distance: ${m['phashDistance']} · dHash: ${m['dhashDistance']}',
                          ),
                        ),
                    ],
                  ],
                ],
              ),
            ),
          ),
          if (_ocrText != null && _ocrText!.trim().isNotEmpty) ...[
            const SizedBox(height: 16),
            Card(
              child: Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('OCR text', style: textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
                    const SizedBox(height: 10),
                    SelectableText(_ocrText!),
                  ],
                ),
              ),
            ),
          ],
          const SizedBox(height: 16),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Reverse image search by URL', style: textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w700)),
                  const SizedBox(height: 10),
                  Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: _urlController,
                          decoration: const InputDecoration(
                            hintText: 'https://example.com/photo.jpg',
                            labelText: 'Public image URL',
                          ),
                        ),
                      ),
                      const SizedBox(width: 16),
                      FilledButton.tonalIcon(
                        onPressed: _openReverseLinks,
                        icon: const Icon(Icons.travel_explore),
                        label: const Text('Search engines'),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _HashChip extends StatelessWidget {
  final String label;
  final String value;
  final Color color;

  const _HashChip({required this.label, required this.value, required this.color});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: TextStyle(fontSize: 10, fontWeight: FontWeight.w800, color: color, letterSpacing: 1)),
          const SizedBox(height: 2),
          SelectableText(value, style: const TextStyle(fontFamily: 'monospace', fontSize: 12)),
        ],
      ),
    );
  }
}
