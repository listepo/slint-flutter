import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:counter/ui/counter.slint.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // On the web this fetches and instantiates the Slint WebAssembly module
  // served from `web/`; elsewhere it returns immediately.
  await initSlint();
  runApp(const CounterApp());
}

class CounterApp extends StatelessWidget {
  const CounterApp({super.key});

  @override
  Widget build(BuildContext context) => WidgetsApp(
        color: const Color(0xff20252b),
        pageRouteBuilder: _defaultRouteBuilder,
        title: 'Slint counter',
        home: SlintView(load: buildCounterUi),
      );

  /// The route builder `WidgetsApp` needs to turn `home` into a page route.
  static PageRoute<T> _defaultRouteBuilder<T>(
          RouteSettings settings, WidgetBuilder builder) =>
      PageRouteBuilder<T>(
        settings: settings,
        pageBuilder: (context, animation, secondaryAnimation) =>
            builder(context),
      );
}

CounterWindow buildCounterUi() {
  final window = CounterWindow.load()
    ..statusMessage = 'Count';

  window.onCountChanged((count) {
    window.statusMessage = count == 0 ? 'Ready' : 'Count';
  });

  return window;
}
