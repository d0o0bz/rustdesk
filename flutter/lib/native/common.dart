import 'dart:io';

final isAndroid_ = Platform.isAndroid;
final isIOS_ = Platform.isIOS;
final isWindows_ = Platform.isWindows;
final isMacOS_ = Platform.isMacOS;
final isLinux_ = Platform.isLinux;
final isWeb_ = false;
final isWebDesktop_ = false;

final isDesktop_ = Platform.isWindows || Platform.isMacOS || Platform.isLinux;

String get screenInfo_ => '';

final isWebOnWindows_ = false;
final isWebOnLinux_ = false;
final isWebOnMacOS_ = false;

/// Opens a local directory in the system file manager.
///
/// `launchUrl(Uri.file(..))` ends up in `ShellExecuteW` on Windows, which waits
/// for the shell to answer over DDE. The shell is not always up when the app is
/// started with the session, and that wait blocks the flutter ui thread.
/// Starting the file manager as a child process never waits on the shell.
Future<bool> openDirectoryImpl(String path) async {
  if (path.isEmpty) {
    return false;
  }
  final exe = Platform.isWindows
      ? 'explorer.exe'
      : Platform.isMacOS
          ? 'open'
          : 'xdg-open';
  try {
    await Process.start(exe, [path]);
    return true;
  } catch (_) {
    return false;
  }
}
