import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:image/image.dart' as img;
import 'package:slint_flutter/slint_flutter.dart';
import 'package:imagefilter/ui/main.slint.dart';

late final img.Image _source;

Future<void> preloadAssets() async {
  final bytes = await rootBundle.load('assets/cat.jpg');
  _source = img.decodeImage(bytes.buffer.asUint8List())!;
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initSlint();
  await preloadAssets();
  runApp(const DemoApp());
}

class DemoApp extends StatelessWidget {
  const DemoApp({super.key});

  @override
  Widget build(BuildContext context) => WidgetsApp(
        color: const Color(0xff000000),
        pageRouteBuilder: _defaultRouteBuilder,
        title: 'Image filter',
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

SlintImage _toSlintImage(img.Image image) {
  final rgba = image.getBytes(order: img.ChannelOrder.rgba);
  return SlintImage.fromRgba(
    image.width,
    image.height,
    Uint8List.fromList(rgba),
  );
}

MainWindow buildUi() {
  final app = MainWindow.load();
  app.originalImage = _toSlintImage(_source);
  app.filters = const [
    'Blur',
    'Brighten',
    'Darken',
    'Increase Contrast',
    'Decrease Contrast',
    'Invert',
  ];

  app.onFilterImage((index) {
    final filtered = switch (index) {
      0 => img.gaussianBlur(_source, radius: 4),
      1 => img.adjustColor(_source, brightness: 1.2),
      2 => img.adjustColor(_source, brightness: 0.8),
      3 => img.adjustColor(_source, contrast: 1.3),
      4 => img.adjustColor(_source, contrast: 0.7),
      _ => () {
          final copy = img.Image.from(_source);
          for (final pixel in copy) {
            pixel
              ..r = 255 - pixel.r
              ..g = 255 - pixel.g
              ..b = 255 - pixel.b;
          }
          return copy;
        }(),
    };
    return _toSlintImage(filtered);
  });

  return app;
}
