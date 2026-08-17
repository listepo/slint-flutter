import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:gallery/ui/gallery.slint.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initSlint();
  runApp(const DemoApp());
}

class DemoApp extends StatelessWidget {
  const DemoApp({super.key});

  @override
  Widget build(BuildContext context) => WidgetsApp(
        color: const Color(0xff000000),
        pageRouteBuilder: _defaultRouteBuilder,
        title: 'Gallery',
        home: SlintView(load: buildUi),
      );

  static PageRoute<T> _defaultRouteBuilder<T>(
          RouteSettings settings, WidgetBuilder builder) =>
      PageRouteBuilder<T>(
        settings: settings,
        pageBuilder: (context, animation, secondaryAnimation) =>
            builder(context),
      );
}

App buildUi() {
  final app = App.load();
  // TableViewPageAdapter ships sample rows and a default filter callback in
  // the .slint; optional host-side filtering can be wired later.
  return app;
}
