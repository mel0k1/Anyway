import 'dart:io';

Future<void> notifyOs(String title, String body) async {
  try {
    if (Platform.isLinux) {
      await Process.run('notify-send', [title, body]);
    } else if (Platform.isMacOS) {
      final escaped = body.replaceAll('"', r'\"');
      await Process.run('osascript', [
        '-e',
        'display notification "$escaped" with title "$title"',
      ]);
    } else if (Platform.isWindows) {
      final t = title.replaceAll("'", "''");
      final b = body.replaceAll("'", "''");
      final ps = "Add-Type -AssemblyName System.Windows.Forms; "
          "\$n = New-Object System.Windows.Forms.NotifyIcon; "
          "\$n.Icon = [System.Drawing.SystemIcons]::Information; "
          "\$n.Visible = \$true; "
          "\$n.ShowBalloonTip(5000, '$t', '$b', "
          "[System.Windows.Forms.ToolTipIcon]::Info); "
          "Start-Sleep -Seconds 6; \$n.Dispose()";
      await Process.run('powershell', ['-NoProfile', '-Command', ps]);
    }
  } catch (_) {}
}
