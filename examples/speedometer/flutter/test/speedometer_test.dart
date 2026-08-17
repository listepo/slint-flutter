import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:speedometer/main.dart' as app;
import 'package:slint_example_test_support/slint_example_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('SlintView renders speedometer UI', (tester) async {
    await pumpSlintExample(
      tester: tester,
      load: app.buildUi,
      viewKey: const ValueKey('speedometer-slint-view'),
    );
  });
}
