import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:counter/main.dart' as app;
import 'package:counter/ui/counter.slint.dart';

/// The route builder `WidgetsApp` needs to turn `home` into a page route.
PageRoute<T> _defaultRouteBuilder<T>(
        RouteSettings settings, WidgetBuilder builder) =>
    PageRouteBuilder<T>(
      settings: settings,
      pageBuilder: (context, animation, secondaryAnimation) => builder(context),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  // Only one SlintView can be alive in an isolate, so this test pumps the
  // widget once and exercises `buildCounterUi` through the loaded window.
  testWidgets('SlintView renders counter UI and buildCounterUi wires callbacks',
      (tester) async {
    late CounterWindow window;

    await tester.pumpWidget(WidgetsApp(
      color: const Color(0xff20252b),
      pageRouteBuilder: _defaultRouteBuilder,
      home: SizedBox(
        width: 320,
        height: 180,
        child: SlintView(
          key: const ValueKey('counter-slint-view'),
          load: () => window = app.buildCounterUi(),
        ),
      ),
    ));
    await tester.pump();

    expect(find.byKey(const ValueKey('counter-slint-view')), findsOneWidget);
    expect(window.currentCount, 0);
    expect(window.statusMessage, 'Count');

    window.currentCount = 3;
    window.invokeCountChanged(3);
    expect(window.statusMessage, 'Count');

    window.invokeResetCounter();
    expect(window.currentCount, 0);
    expect(window.statusMessage, 'Ready');
  });
}
