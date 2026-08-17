import 'dart:async';
import 'dart:math';

import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:memory/ui/memory.slint.dart';

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
        title: 'Memory',
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

MainWindow buildUi() {
  final app = MainWindow.load();
  final tiles = List<TileData>.from(app.memoryTiles);
  tiles.addAll(List<TileData>.from(app.memoryTiles));
  tiles.shuffle(Random());
  app.memoryTiles = tiles;

  app.onCheckIfPairSolved(() {
    final flipped = <int>[];
    for (var i = 0; i < app.memoryTiles.length; i++) {
      final tile = app.memoryTiles[i];
      if (tile.imageVisible && !tile.solved) {
        flipped.add(i);
      }
    }
    if (flipped.length != 2) return;
    final t1 = app.memoryTiles[flipped[0]];
    final t2 = app.memoryTiles[flipped[1]];
    if (t1.image == t2.image) {
      final next = List<TileData>.from(app.memoryTiles);
      next[flipped[0]] = TileData(
        image: t1.image,
        imageVisible: true,
        solved: true,
      );
      next[flipped[1]] = TileData(
        image: t2.image,
        imageVisible: true,
        solved: true,
      );
      app.memoryTiles = next;
    } else {
      app.disableTiles = true;
      Timer(const Duration(seconds: 1), () {
        final next = List<TileData>.from(app.memoryTiles);
        final a = next[flipped[0]];
        final b = next[flipped[1]];
        next[flipped[0]] = TileData(
          image: a.image,
          imageVisible: false,
          solved: a.solved,
        );
        next[flipped[1]] = TileData(
          image: b.image,
          imageVisible: false,
          solved: b.solved,
        );
        app.memoryTiles = next;
        app.disableTiles = false;
      });
    }
  });

  return app;
}
