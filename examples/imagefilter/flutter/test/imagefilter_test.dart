import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:imagefilter/main.dart' as app;
import 'package:imagefilter/ui/main.slint.dart';
import 'package:slint_example_test_support/slint_example_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await app.preloadAssets();
  });

  testWidgets('buildUi loads filters and returns filtered images', (tester) async {
    final window = await pumpSlintExample<MainWindow>(
      tester: tester,
      load: app.buildUi,
      viewKey: const ValueKey('imagefilter-slint-view'),
      width: 900,
      height: 650,
    );

    expect(window.filters.length, 6);
    expect(window.originalImage.width, greaterThan(0));
    expect(window.originalImage.height, greaterThan(0));

    final inverted = window.invokeFilterImage(5);
    expect(inverted.width, window.originalImage.width);
    expect(inverted.height, window.originalImage.height);
  });
}
