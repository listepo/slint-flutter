import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slide_puzzle/main.dart' as app;
import 'package:slide_puzzle/ui/slide_puzzle.slint.dart';
import 'package:slint_example_test_support/slint_example_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('buildUi randomizes a solvable puzzle', (tester) async {
    final window = await pumpSlintExample<MainWindow>(
      tester: tester,
      load: app.buildUi,
      viewKey: const ValueKey('slide-puzzle-slint-view'),
      width: 500,
      height: 600,
    );

    expect(window.pieces.length, 15);
    expect(window.moves, 0);
    expect(window.tilesLeft, greaterThan(0));
    expect(window.autoPlay, isFalse);
  });
}
