import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:sprite_sheet/ui/demo.slint.dart';

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
        title: 'Sprite sheet',
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

AppWindow buildUi() => AppWindow.load();

