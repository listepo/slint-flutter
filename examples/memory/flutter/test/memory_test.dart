import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memory/main.dart' as app;
import 'package:memory/ui/memory.slint.dart';
import 'package:slint_example_test_support/slint_example_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('buildUi duplicates and shuffles the tile deck', (tester) async {
    final window = await pumpSlintExample<MainWindow>(
      tester: tester,
      load: app.buildUi,
      viewKey: const ValueKey('memory-slint-view'),
      width: 400,
      height: 400,
    );

    expect(window.memoryTiles.length, 16);
    expect(window.disableTiles, isFalse);
  });
}
