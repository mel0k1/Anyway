import 'package:flutter/material.dart';

class AppTheme {
  static const seed = Color(0xFF7C6AF0);
  static const family = 'AnywaySans';

  static ThemeData dark() =>
      _base(ColorScheme.fromSeed(seedColor: seed, brightness: Brightness.dark));

  static ThemeData light() => _base(
    ColorScheme.fromSeed(seedColor: seed, brightness: Brightness.light),
  );

  static ThemeData _base(ColorScheme scheme) {
    const shapeXL = RoundedRectangleBorder(
      borderRadius: BorderRadius.all(Radius.circular(32)),
    );
    const shapeM = RoundedRectangleBorder(
      borderRadius: BorderRadius.all(Radius.circular(16)),
    );

    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      fontFamily: family,
      scaffoldBackgroundColor: scheme.surface,
      splashFactory: InkSparkle.splashFactory,
      cardTheme: CardThemeData(
        elevation: 0,
        color: scheme.surfaceContainerLow,
        shape: shapeXL,
        margin: EdgeInsets.zero,
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size(0, 52),
          padding: const EdgeInsets.symmetric(horizontal: 28),
          shape: const StadiumBorder(),
          textStyle: TextStyle(
            fontWeight: FontWeight.w600,
            fontSize: 15,
            fontFamily: family,
          ),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size(0, 48),
          padding: const EdgeInsets.symmetric(horizontal: 24),
          shape: const StadiumBorder(),
          side: BorderSide(color: scheme.outlineVariant),
          textStyle: TextStyle(
            fontWeight: FontWeight.w600,
            fontSize: 15,
            fontFamily: family,
          ),
        ),
      ),
      segmentedButtonTheme: SegmentedButtonThemeData(
        style: ButtonStyle(
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
          ),
          side: WidgetStatePropertyAll(
            BorderSide(color: scheme.outlineVariant),
          ),
        ),
      ),
      chipTheme: ChipThemeData(
        shape: const StadiumBorder(),
        side: BorderSide.none,
        backgroundColor: scheme.surfaceContainerHigh,
        labelStyle: TextStyle(
          color: scheme.onSurfaceVariant,
          fontSize: 12.5,
          fontFamily: family,
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: scheme.surfaceContainerLow,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(28),
          borderSide: BorderSide.none,
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(28),
          borderSide: BorderSide.none,
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(28),
          borderSide: BorderSide(color: scheme.primary, width: 2),
        ),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 24,
          vertical: 18,
        ),
      ),
      sliderTheme: SliderThemeData(
        showValueIndicator: ShowValueIndicator.onDrag,
        trackHeight: 8,
        thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 10),
        overlayShape: const RoundSliderOverlayShape(overlayRadius: 20),
        valueIndicatorShape: const PaddleSliderValueIndicatorShape(),
      ),
      progressIndicatorTheme: ProgressIndicatorThemeData(
        color: scheme.primary,
        linearTrackColor: scheme.surfaceContainerHighest,
        circularTrackColor: scheme.surfaceContainerHighest,
        strokeCap: StrokeCap.round,
      ),
      switchTheme: SwitchThemeData(
        thumbColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected)
              ? scheme.onPrimary
              : scheme.outline,
        ),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: shapeM,
      ),
      tooltipTheme: const TooltipThemeData(preferBelow: false),
      dialogTheme: DialogThemeData(
        shape: shapeXL,
        backgroundColor: scheme.surfaceContainerHigh,
      ),
      menuTheme: MenuThemeData(
        style: MenuStyle(
          backgroundColor: WidgetStatePropertyAll(scheme.surfaceContainerHigh),
          surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
          shape: WidgetStatePropertyAll(shapeXL),
          elevation: const WidgetStatePropertyAll(8),
          padding: const WidgetStatePropertyAll(
            EdgeInsets.symmetric(vertical: 8),
          ),
        ),
      ),
      dropdownMenuTheme: DropdownMenuThemeData(
        menuStyle: MenuStyle(
          backgroundColor: WidgetStatePropertyAll(scheme.surfaceContainerHigh),
          surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
          shape: WidgetStatePropertyAll(shapeXL),
          elevation: const WidgetStatePropertyAll(8),
        ),
      ),
    );
  }
}
