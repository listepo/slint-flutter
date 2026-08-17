import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:http/http.dart' as http;
import 'package:slint_flutter/slint_flutter.dart';
import 'package:async_io/ui/stockticker.slint.dart';

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
        title: 'Stock ticker',
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
  Future<void> refresh() async {
    // Generated as `Symbol_` — `Symbol` is reserved by `dart:core`.
    final stocks = List<Symbol_>.from(app.stocks);
    final names = stocks.map((s) => s.name).join('+');
    final url = Uri.parse(
      'https://stooq.com/q/l/?s=$names&f=sd2t2ohlcvn&h&e=json',
    );
    try {
      final response = await http.get(url);
      if (response.statusCode != 200) return;
      final json = jsonDecode(response.body) as Map<String, Object?>;
      final symbols = (json['symbols'] as List<Object?>?) ?? const [];
      final byName = <String, double>{};
      for (final entry in symbols) {
        final map = (entry as Map).cast<String, Object?>();
        final name = map['symbol'] as String?;
        final close = (map['close'] as num?)?.toDouble();
        if (name != null && close != null) {
          byName[name] = close;
        }
      }
      app.stocks = [
        for (final stock in stocks)
          Symbol_(name: stock.name, price: byName[stock.name] ?? stock.price),
      ];
    } catch (_) {
      // Keep the last known prices when the network is unavailable.
    }
  }

  app.onRefresh(() {
    refresh();
  });
  refresh();
  return app;
}
