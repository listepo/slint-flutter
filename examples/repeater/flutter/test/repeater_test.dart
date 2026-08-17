import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:repeater/main.dart' as app;
import 'package:slint_example_test_support/slint_example_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('SlintView renders repeater UI', (tester) async {
    await pumpSlintExample(
      tester: tester,
      load: app.buildUi,
      viewKey: const ValueKey('repeater-slint-view'),
    );
  });
}
