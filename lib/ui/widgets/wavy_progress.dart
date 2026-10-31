import 'dart:math' as math;

import 'package:flutter/material.dart';

class WavyProgress extends StatefulWidget {
  final double? progress;
  final double height;

  const WavyProgress({super.key, this.progress, this.height = 10});

  @override
  State<WavyProgress> createState() => _WavyProgressState();
}

class _WavyProgressState extends State<WavyProgress> with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(vsync: this, duration: const Duration(milliseconds: 1400))..repeat();

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _c,
      builder: (context, _) => CustomPaint(
        size: Size(double.infinity, widget.height),
        painter: _WavyPainter(
          phase: _c.value * 2 * math.pi,
          amplitude: widget.progress == null ? (3 + 2 * math.sin(_c.value * math.pi)) : 3.2,
          progress: widget.progress,
          color: Theme.of(context).colorScheme.primary,
          trackColor: Theme.of(context).colorScheme.surfaceContainerHighest,
        ),
      ),
    );
  }
}

class _WavyPainter extends CustomPainter {
  final double phase;
  final double amplitude;
  final double? progress;
  final Color color;
  final Color trackColor;

  _WavyPainter({required this.phase, required this.amplitude, required this.progress, required this.color, required this.trackColor});

  @override
  void paint(Canvas canvas, Size size) {
    final trackPaint = Paint()
      ..color = trackColor
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;
    final wavePaint = Paint()
      ..color = color
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;

    final cy = size.height / 2;
    canvas.drawLine(Offset(0, cy), Offset(size.width, cy), trackPaint);

    final done = progress == null ? size.width : (size.width * progress!.clamp(0.02, 1.0));
    const wavelength = 14.0;
    final path = Path()..moveTo(0, cy);
    for (double x = 0; x <= done; x += 1.5) {
      final y = cy + amplitude * math.sin(2 * math.pi * x / wavelength + phase);
      path.lineTo(x, y);
    }
    canvas.drawPath(path, wavePaint);

    if (progress != null && progress! >= 1.0) {
      final stop = Paint()..color = color;
      canvas.drawCircle(Offset(size.width - 4, cy), 4, stop);
    }
  }

  @override
  bool shouldRepaint(_WavyPainter old) => old.phase != phase || old.progress != progress || old.color != color;
}
