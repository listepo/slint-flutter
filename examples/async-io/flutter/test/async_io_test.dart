import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:async_io/main.dart' as app;
import 'package:async_io/ui/stockticker.slint.dart';
import 'package:slint_example_test_support/slint_example_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('buildUi wires refresh and keeps the stock list', (tester) async {
    final window = await pumpSlintExample<MainWindow>(
      tester: tester,
      load: app.buildUi,
      viewKey: const ValueKey('async-io-slint-view'),
    );

    expect(window.stocks.length, 3);
    expect(window.stocks.map((s) => s.name), contains('AAPL.US'));

    window.invokeRefresh();
    await tester.pump();
    expect(window.stocks.length, 3);
  });
}
