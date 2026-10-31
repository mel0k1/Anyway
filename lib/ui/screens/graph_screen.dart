import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../core/app_state.dart';
import '../../core/models.dart';
import '../theme/app_theme.dart';

class GraphScreen extends StatefulWidget {
  final AppState state;

  const GraphScreen({super.key, required this.state});

  @override
  State<GraphScreen> createState() => _GraphScreenState();
}

class _NodePos {
  final GraphNode node;
  Offset pos;
  Offset velocity = Offset.zero;
  final bool pinned;
  int degree = 0;

  _NodePos(this.node, this.pos, {this.pinned = false});
}

class _GraphScreenState extends State<GraphScreen> {
  final TransformationController _transform = TransformationController();
  final Map<String, _NodePos> _positions = {};
  String? _selected;
  final Set<Confidence> _visible = Confidence.values.toSet();
  int _layoutSeed = 0;
  final Size _canvas = const Size(2400, 1600);
  Size _viewport = Size.zero;

  ScanSnapshot? get _snap =>
      widget.state.lastSnapshot ?? widget.state.activeScan;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncGraph();
  }

  void _syncGraph() {
    final graph = _snap?.graph ?? OsintGraph.empty;
    if (_positions.keys.length == graph.nodes.length &&
        graph.nodes.every((n) => _positions.containsKey(n.id))) {
      return;
    }
    _layout(graph);
  }

  void _layout(OsintGraph graph) {
    _layoutSeed++;
    final rng = math.Random(_layoutSeed);
    _positions.clear();
    final roots = graph.nodes.where((n) => n.root).toList();
    final rest = graph.nodes.where((n) => !n.root).toList();
    final center = Offset(_canvas.width / 2, _canvas.height / 2);
    for (var i = 0; i < roots.length; i++) {
      final angle = 2 * math.pi * i / math.max(roots.length, 1);
      _positions[roots[i].id] = _NodePos(
        roots[i],
        center + Offset(math.cos(angle) * 60, math.sin(angle) * 60),
        pinned: true,
      );
    }
    for (final n in rest) {
      _positions[n.id] = _NodePos(
        n,
        center +
            Offset(
              math.cos(rng.nextDouble() * 2 * math.pi) * 400,
              math.sin(rng.nextDouble() * 2 * math.pi) * 300,
            ),
      );
    }
    for (final e in graph.edges) {
      _positions[e.from]?.degree++;
      _positions[e.to]?.degree++;
    }
    _simulate(graph);
  }

  void _simulate(OsintGraph graph) {
    final rng = math.Random(_layoutSeed);
    final nodes = _positions.values.toList();
    if (nodes.isEmpty) return;
    const repulsion = 34000.0;
    const springLength = 150.0;
    const springK = 0.045;
    const gravity = 0.012;
    final iterations = nodes.length > 400 ? 90 : 200;
    final edges = graph.edges
        .where(
          (e) => _positions.containsKey(e.from) && _positions.containsKey(e.to),
        )
        .toList();
    final centers = nodes.where((n) => n.pinned).map((n) => n.pos).toList();
    final center = centers.isEmpty
        ? Offset(_canvas.width / 2, _canvas.height / 2)
        : Offset(
            centers.map((c) => c.dx).reduce((a, b) => a + b) / centers.length,
            centers.map((c) => c.dy).reduce((a, b) => a + b) / centers.length,
          );
    for (var step = 0; step < iterations; step++) {
      final cooling = 1.0 - step / iterations;
      for (var i = 0; i < nodes.length; i++) {
        for (var j = i + 1; j < nodes.length; j++) {
          final a = nodes[i];
          final b = nodes[j];
          var delta = a.pos - b.pos;
          var dist = delta.distance;
          if (dist < 1) {
            delta = Offset(rng.nextDouble() - 0.5, rng.nextDouble() - 0.5);
            dist = 1;
          }
          dist = math.max(dist, 24.0);
          final force = repulsion / (dist * dist);
          final apply = delta / dist * force;
          if (!a.pinned) a.velocity += apply;
          if (!b.pinned) b.velocity -= apply;
        }
      }
      for (final e in edges) {
        final a = _positions[e.from]!;
        final b = _positions[e.to]!;
        var delta = a.pos - b.pos;
        final dist = math.max(delta.distance, 1.0);
        final force = (dist - springLength) * springK;
        final apply = delta / dist * force;
        if (!a.pinned) a.velocity -= apply;
        if (!b.pinned) b.velocity += apply;
      }
      for (final n in nodes) {
        if (n.pinned) continue;
        n.velocity += (center - n.pos) * gravity;
        n.pos += n.velocity * cooling.clamp(0.05, 1.0);
        n.velocity *= 0.55;
      }
    }
  }

  Set<String> get _hiddenIds => {
    for (final entry in _positions.entries)
      if (!_visible.contains(entry.value.node.confidence)) entry.key,
  };

  void _fit() {
    if (_viewport == Size.zero || _positions.isEmpty) return;
    var minX = double.infinity;
    var minY = double.infinity;
    var maxX = double.negativeInfinity;
    var maxY = double.negativeInfinity;
    for (final n in _positions.values) {
      if (!_visible.contains(n.node.confidence)) continue;
      minX = math.min(minX, n.pos.dx);
      minY = math.min(minY, n.pos.dy);
      maxX = math.max(maxX, n.pos.dx);
      maxY = math.max(maxY, n.pos.dy);
    }
    if (minX == double.infinity) return;
    const pad = 90.0;
    final bw = math.max(maxX - minX + pad * 2, 1.0);
    final bh = math.max(maxY - minY + pad * 2, 1.0);
    final s = math
        .min(
          (_viewport.width - pad * 2) / bw,
          (_viewport.height - pad * 2) / bh,
        )
        .clamp(0.15, 3.5);
    final cx = (minX + maxX) / 2;
    final cy = (minY + maxY) / 2;
    _transform.value = Matrix4.identity()
      ..translateByDouble(_viewport.width / 2, _viewport.height / 2, 0, 1)
      ..scaleByDouble(s, s, 1, 1)
      ..translateByDouble(-cx, -cy, 0, 1);
  }

  Offset _toCanvas(Offset localPoint) {
    final inverse = Matrix4.inverted(_transform.value);
    return MatrixUtils.transformPoint(inverse, localPoint);
  }

  void _onTapUp(TapUpDetails details) {
    final canvasPoint = _toCanvas(details.localPosition);
    GraphNode? hit;
    double bestDist = double.infinity;
    for (final entry in _positions.entries) {
      if (_hiddenIds.contains(entry.key)) continue;
      final d = (entry.value.pos - canvasPoint).distance;
      final radius = _radius(entry.value) + 6;
      if (d < radius && d < bestDist) {
        bestDist = d;
        hit = entry.value.node;
      }
    }
    setState(() => _selected = hit?.id);
  }

  double _radius(_NodePos n) =>
      n.node.root ? 26 : math.min(12.0 + n.degree * 2.2, 30);

  Color _confidenceColor(Confidence c, ColorScheme scheme) => switch (c) {
    Confidence.high => const Color(0xFF43A047),
    Confidence.medium => const Color(0xFFFFB300),
    Confidence.low => const Color(0xFF78909C),
  };

  @override
  Widget build(BuildContext context) {
    final snap = _snap;
    final scheme = Theme.of(context).colorScheme;
    final graph = snap?.graph ?? OsintGraph.empty;

    if (snap == null || graph.nodes.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.hub, size: 72, color: scheme.outlineVariant),
            const SizedBox(height: 16),
            Text('No graph yet', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 8),
            Text(
              'Run a scan to see entities and their connections',
              style: TextStyle(color: scheme.onSurfaceVariant),
            ),
          ],
        ),
      );
    }

    final hidden = _hiddenIds;
    final visibleNodes = graph.nodes
        .where((n) => !hidden.contains(n.id))
        .length;
    final visibleEdges = graph.edges
        .where((e) => !hidden.contains(e.from) && !hidden.contains(e.to))
        .length;
    final selectedNode = graph.nodes
        .where((n) => n.id == _selected)
        .firstOrNull;

    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Graph · ${snap.target.raw}',
                      style: Theme.of(context).textTheme.headlineSmall
                          ?.copyWith(fontWeight: FontWeight.w800),
                    ),
                    const SizedBox(height: 6),
                    Text(
                      '$visibleNodes nodes · $visibleEdges links · confidence filters apply',
                      style: TextStyle(color: scheme.onSurfaceVariant),
                    ),
                  ],
                ),
              ),
              IconButton(
                tooltip: 'Fit to view',
                icon: const Icon(Icons.center_focus_strong),
                onPressed: _fit,
              ),
              IconButton(
                tooltip: 'Relayout',
                icon: const Icon(Icons.refresh),
                onPressed: () => setState(() => _layout(graph)),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            children: [
              for (final c in Confidence.values)
                FilterChip(
                  selected: _visible.contains(c),
                  label: Text(c.label),
                  avatar: Icon(
                    Icons.circle,
                    size: 12,
                    color: _confidenceColor(c, scheme),
                  ),
                  onSelected: (on) => setState(() {
                    on ? _visible.add(c) : _visible.remove(c);
                    if (_selected != null) {
                      final sel = graph.nodes
                          .where((n) => n.id == _selected)
                          .firstOrNull;
                      if (sel != null && !_visible.contains(sel.confidence)) {
                        _selected = null;
                      }
                    }
                  }),
                ),
              const SizedBox(width: 8),
              Chip(
                avatar: Icon(Icons.hub, size: 16, color: scheme.primary),
                label: Text('${graph.nodeCount} total'),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Expanded(
            child: LayoutBuilder(
              builder: (context, box) {
                _viewport = Size(box.maxWidth, box.maxHeight);
                return ClipRRect(
                  borderRadius: BorderRadius.circular(24),
                  child: Stack(
                    children: [
                      Positioned.fill(
                        child: GestureDetector(
                          onTapUp: _onTapUp,
                          child: InteractiveViewer(
                            transformationController: _transform,
                            boundaryMargin: const EdgeInsets.all(600),
                            minScale: 0.15,
                            maxScale: 3.5,
                            panEnabled: true,
                            scaleEnabled: true,
                            child: SizedBox(
                              width: _canvas.width,
                              height: _canvas.height,
                              child: CustomPaint(
                                painter: _GraphPainter(
                                  graph: graph,
                                  positions: _positions,
                                  hidden: hidden,
                                  selected: _selected,
                                  confidenceColor: (c) =>
                                      _confidenceColor(c, scheme),
                                  outline: scheme.outlineVariant,
                                  background: Colors.transparent,
                                  labelColor: scheme.onSurface,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                      Positioned(
                        left: 16,
                        bottom: 16,
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 14,
                            vertical: 10,
                          ),
                          decoration: BoxDecoration(
                            color: scheme.surfaceContainerHigh.withValues(
                              alpha: 0.92,
                            ),
                            borderRadius: BorderRadius.circular(16),
                          ),
                          child: Wrap(
                            spacing: 14,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: [
                              for (final k in GraphNodeKind.values)
                                if (graph.nodes.any(
                                  (n) => n.kind == k && !hidden.contains(n.id),
                                ))
                                  Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Icon(
                                        k.icon,
                                        size: 14,
                                        color: scheme.onSurfaceVariant,
                                      ),
                                      const SizedBox(width: 4),
                                      Text(
                                        k.title,
                                        style: const TextStyle(fontSize: 11),
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
              },
            ),
          ),
          if (selectedNode != null)
            _NodePanel(
              node: selectedNode,
              graph: graph,
              scheme: scheme,
              confidenceColor: (c) => _confidenceColor(c, scheme),
              onClose: () => setState(() => _selected = null),
              onSelect: (id) => setState(() => _selected = id),
            ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _transform.dispose();
    super.dispose();
  }
}

class _GraphPainter extends CustomPainter {
  final OsintGraph graph;
  final Map<String, _NodePos> positions;
  final Set<String> hidden;
  final String? selected;
  final Color Function(Confidence) confidenceColor;
  final Color outline;
  final Color background;
  final Color labelColor;

  _GraphPainter({
    required this.graph,
    required this.positions,
    required this.hidden,
    required this.selected,
    required this.confidenceColor,
    required this.outline,
    required this.background,
    required this.labelColor,
  });

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(Offset.zero & size, Paint()..color = background);
    final selectedEdges = <GraphEdge>{};
    for (final e in graph.edges) {
      if (hidden.contains(e.from) || hidden.contains(e.to)) continue;
      final a = positions[e.from]!;
      final b = positions[e.to]!;
      final isSelected =
          selected != null && (e.from == selected || e.to == selected);
      if (isSelected) selectedEdges.add(e);
      final paint = Paint()
        ..color = confidenceColor(e.confidence).withValues(
          alpha: selected == null ? 0.38 : (isSelected ? 0.85 : 0.08),
        )
        ..strokeWidth = isSelected ? 2.4 : 1.2
        ..style = PaintingStyle.stroke;
      canvas.drawLine(a.pos, b.pos, paint);
    }

    final labelStyle = TextStyle(
      color: labelColor.withValues(alpha: 0.85),
      fontSize: 11,
      fontFamily: AppTheme.family,
    );
    for (final entry in positions.entries) {
      if (hidden.contains(entry.key)) continue;
      final n = entry.value;
      final radius = _radius(n);
      final color = confidenceColor(n.node.confidence);
      final isSel = entry.key == selected;
      final isConnected =
          selected != null &&
          graph.edges.any(
            (e) =>
                (e.from == selected && e.to == entry.key) ||
                (e.to == selected && e.from == entry.key),
          );
      final dimmed = selected != null && !isSel && !isConnected;

      if (n.node.root) {
        canvas.drawCircle(
          n.pos,
          radius + 7,
          Paint()
            ..color = color.withValues(alpha: dimmed ? 0.15 : 0.3)
            ..style = PaintingStyle.stroke
            ..strokeWidth = 2,
        );
      }
      canvas.drawCircle(
        n.pos,
        radius,
        Paint()..color = color.withValues(alpha: dimmed ? 0.25 : 0.9),
      );
      canvas.drawCircle(
        n.pos,
        radius,
        Paint()
          ..color = isSel ? Colors.white : outline
          ..style = PaintingStyle.stroke
          ..strokeWidth = isSel ? 2.6 : 1,
      );
      final glyph = _kindGlyph(n.node.kind);
      if (glyph != null) {
        final tp = TextPainter(
          text: TextSpan(
            text: glyph,
            style: TextStyle(
              color: Colors.white.withValues(alpha: dimmed ? 0.4 : 0.95),
              fontSize: math.max(radius * 0.85, 10),
              fontWeight: FontWeight.w800,
              fontFamily: AppTheme.family,
            ),
          ),
          textDirection: TextDirection.ltr,
        )..layout();
        tp.paint(canvas, n.pos - Offset(tp.width / 2, tp.height / 2));
      }
      final tp = TextPainter(
        text: TextSpan(
          text: n.node.label.length > 26
              ? '${n.node.label.substring(0, 25)}…'
              : n.node.label,
          style: labelStyle,
        ),
        textDirection: TextDirection.ltr,
        maxLines: 1,
      )..layout();
      if (tp.width < radius * 4) {
        tp.paint(
          canvas,
          n.pos + Offset(0, radius + 4) - Offset(tp.width / 2, 0),
        );
      }
    }
  }

  String? _kindGlyph(GraphNodeKind kind) => switch (kind) {
    GraphNodeKind.username => 'U',
    GraphNodeKind.email => '@',
    GraphNodeKind.ip => 'IP',
    GraphNodeKind.domain => 'D',
    GraphNodeKind.phone => 'P',
    GraphNodeKind.file => 'F',
    GraphNodeKind.platform => 'S',
    GraphNodeKind.org => 'O',
    GraphNodeKind.country => 'C',
    GraphNodeKind.location => 'L',
    GraphNodeKind.device => 'V',
    GraphNodeKind.person => 'H',
    GraphNodeKind.provider => 'M',
    GraphNodeKind.infra => 'N',
    GraphNodeKind.lead => '?',
    GraphNodeKind.other => null,
  };

  double _radius(_NodePos n) =>
      n.node.root ? 26 : math.min(12.0 + n.degree * 2.2, 30);

  @override
  bool shouldRepaint(covariant _GraphPainter oldDelegate) =>
      oldDelegate.selected != selected ||
      oldDelegate.graph != graph ||
      oldDelegate.hidden.length != hidden.length;
}

class _NodePanel extends StatelessWidget {
  final GraphNode node;
  final OsintGraph graph;
  final ColorScheme scheme;
  final Color Function(Confidence) confidenceColor;
  final VoidCallback onClose;
  final void Function(String id) onSelect;

  const _NodePanel({
    required this.node,
    required this.graph,
    required this.scheme,
    required this.confidenceColor,
    required this.onClose,
    required this.onSelect,
  });

  List<GraphEdge> get _edges =>
      graph.edges.where((e) => e.from == node.id || e.to == node.id).toList();

  @override
  Widget build(BuildContext context) {
    final color = confidenceColor(node.confidence);
    return Container(
      margin: const EdgeInsets.only(top: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(20),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              Icon(node.kind.icon, size: 20, color: scheme.primary),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  node.label,
                  style: const TextStyle(fontWeight: FontWeight.w700),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 4,
                ),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(20),
                ),
                child: Text(
                  node.confidence.label.toUpperCase(),
                  style: TextStyle(
                    color: color,
                    fontSize: 10,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 0.5,
                  ),
                ),
              ),
              IconButton(
                icon: const Icon(Icons.close, size: 18),
                onPressed: onClose,
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            '${node.kind.title} · ${_edges.length} link${_edges.length == 1 ? '' : 's'}'
            '${node.sources.isEmpty ? '' : ' · via ${node.sources.join(', ')}'}',
            style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 12.5),
          ),
          const SizedBox(height: 8),
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 180),
            child: SingleChildScrollView(
              child: Column(
                children: [
                  for (final e in _edges)
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 2),
                      child: InkWell(
                        onTap: () =>
                            onSelect(e.from == node.id ? e.to : e.from),
                        child: Row(
                          children: [
                            Icon(
                              Icons.arrow_right,
                              size: 16,
                              color: confidenceColor(e.confidence),
                            ),
                            const SizedBox(width: 4),
                            Expanded(
                              child: Text(
                                '${e.from == node.id ? '→' : '←'} ${_labelOf(e.from == node.id ? e.to : e.from)} · ${_prettyRelation(e.relation)}',
                                style: const TextStyle(fontSize: 12.5),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  String _labelOf(String id) =>
      graph.nodes.where((n) => n.id == id).firstOrNull?.label ?? id;

  String _prettyRelation(String relation) => relation
      .split('_')
      .map((w) => w.isEmpty ? w : '${w[0]}${w.substring(1)}')
      .join(' ');
}
