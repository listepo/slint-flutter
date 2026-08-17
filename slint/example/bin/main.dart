import 'dart:io';

import 'package:slint/slint.dart';
import 'package:slint_codegen_example/ui/counter.slint.dart';

void main() {
  // Slint's own window needs a native event loop, which macOS only allows on
  // the process main thread — and that is not where the Dart VM runs `main()`.
  // So on macOS the software renderer stands in as the platform: no window,
  // but the typed API below behaves identically. `slint_flutter` is how you
  // show a UI there; see `examples/todo/flutter`.
  final windowed = !Platform.isMacOS;
  if (!windowed) SlintSurface();

  final app = CounterWindow.load()
    ..statusMessage = 'Click the window'
    ..onCountChanged((value) => print('Count: $value'));

  app.currentCount = 3;
  app.invokeResetCounter();
  print(
      'statusMessage: ${app.statusMessage}, currentCount: ${app.currentCount}');

  if (!windowed) {
    print('Not opening a window: run this on Linux or Windows for that, or '
        'use slint_flutter to embed the UI in a Flutter app.');
    return;
  }
  app.run();
}
