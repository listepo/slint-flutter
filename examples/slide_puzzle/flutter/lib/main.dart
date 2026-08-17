import 'dart:math';

import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:slide_puzzle/ui/slide_puzzle.slint.dart';

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
        title: 'Slide puzzle',
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

bool _isSolvable(List<int> positions) {
  var inversions = 0;
  for (var x = 0; x < positions.length - 1; x++) {
    final v = positions[x];
    for (var y = x + 1; y < positions.length; y++) {
      final other = positions[y];
      if (other >= 0 && other < v) inversions++;
    }
  }
  final blankRow = positions.indexOf(-1) ~/ 4;
  return inversions % 2 != blankRow % 2;
}

List<int> _shuffle(Random rng) {
  final vec = List<int>.generate(16, (i) => i - 1);
  do {
    vec.shuffle(rng);
  } while (!_isSolvable(vec));
  return vec;
}

bool _spring(List<double> state) {
  const c = 0.3;
  const damp = 0.7;
  const eps = 0.3;
  var offset = state[0];
  var speed = state[1];
  speed = (speed - offset * c) * damp;
  if (speed != 0 || offset != 0) {
    offset += speed;
    if (speed.abs() < eps && offset.abs() < eps) {
      speed = 0;
      offset = 0;
    }
    state[0] = offset;
    state[1] = speed;
    return true;
  }
  return false;
}

MainWindow buildUi() {
  final app = MainWindow.load();
  final rng = Random();
  var positions = <int>[];
  final kickSpeed = List.generate(15, (_) => [0.0, 0.0]);
  SlintTimer? autoPlayTimer;
  SlintTimer? kickTimer;
  var finished = false;

  void setPiecesPos(int p, int pos) {
    if (p < 0) return;
    final pieces = List<Piece>.from(app.pieces);
    final cur = pieces[p];
    pieces[p] = Piece(
      posX: pos ~/ 4,
      posY: pos % 4,
      offsetX: cur.offsetX,
      offsetY: cur.offsetY,
    );
    app.pieces = pieces;
  }

  void applyTilesLeft() {
    final left =
        15 - positions.asMap().entries.where((e) => e.key == e.value).length;
    app.tilesLeft = left;
    finished = left == 0;
  }

  void randomize() {
    positions = _shuffle(rng);
    for (var i = 0; i < positions.length; i++) {
      setPiecesPos(positions[i], i);
    }
    app.moves = 0;
    applyTilesLeft();
  }

  void slide(int pos, int offset) {
    var swap = pos;
    while (positions[pos] != -1) {
      swap += offset;
      final tmp = positions[pos];
      positions[pos] = positions[swap];
      positions[swap] = tmp;
      setPiecesPos(positions[swap], swap);
    }
  }

  bool pieceClicked(int p) {
    final piece = app.pieces[p];
    final hole = positions.indexOf(-1);
    final pos = piece.posX * 4 + piece.posY;
    final sign = pos > hole ? -1 : 1;
    if (hole % 4 == piece.posY) {
      slide(pos, sign * 4);
    } else if (hole ~/ 4 == piece.posX) {
      slide(pos, sign);
    } else {
      kickSpeed[p][0] = hole % 4 > piece.posY ? 10.0 : -10.0;
      kickSpeed[p][1] = hole ~/ 4 > piece.posX ? 10.0 : -10.0;
      return false;
    }
    applyTilesLeft();
    app.moves = app.moves + 1;
    return true;
  }

  void kickAnimation() {
    var hasAnimation = false;
    final pieces = List<Piece>.from(app.pieces);
    for (var idx = 0; idx < 15; idx++) {
      final ox = [pieces[idx].offsetX, kickSpeed[idx][0]];
      final oy = [pieces[idx].offsetY, kickSpeed[idx][1]];
      final ax = _spring(ox);
      final ay = _spring(oy);
      kickSpeed[idx][0] = ox[1];
      kickSpeed[idx][1] = oy[1];
      if (ax || ay) {
        pieces[idx] = Piece(
          posX: pieces[idx].posX,
          posY: pieces[idx].posY,
          offsetX: ox[0],
          offsetY: oy[0],
        );
        hasAnimation = true;
      }
    }
    if (hasAnimation) {
      app.pieces = pieces;
    } else {
      kickTimer?.stop();
      kickTimer = null;
    }
  }

  void randomMove() {
    final hole = positions.indexOf(-1);
    late int cell;
    while (true) {
      cell = rng.nextInt(16);
      if (hole == cell) continue;
      if ((hole % 4 == cell % 4) || (hole ~/ 4 == cell ~/ 4)) break;
    }
    pieceClicked(positions[cell]);
  }

  app.pieces = List.generate(
    15,
    (_) => Piece(posX: 0, posY: 0, offsetX: 0, offsetY: 0),
  );
  randomize();

  app.onPieceClicked((p) {
    autoPlayTimer?.stop();
    app.autoPlay = false;
    if (finished) return;
    if (!pieceClicked(p)) {
      kickTimer?.stop();
      kickTimer =
          SlintTimer.periodic(const Duration(milliseconds: 16), kickAnimation);
    }
  });

  app.onReset(() {
    autoPlayTimer?.stop();
    app.autoPlay = false;
    randomize();
  });

  app.onEnableAutoMode((enabled) {
    autoPlayTimer?.stop();
    autoPlayTimer = null;
    if (enabled) {
      autoPlayTimer = SlintTimer.periodic(
        const Duration(milliseconds: 200),
        randomMove,
      );
    }
  });

  return app;
}
