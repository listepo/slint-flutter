import 'package:flutter/material.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:seven_guis/ui/booker.slint.dart';
import 'package:seven_guis/ui/counter.slint.dart';
import 'package:seven_guis/ui/crud.slint.dart';
import 'package:seven_guis/ui/tempconv.slint.dart';
import 'package:seven_guis/ui/timer.slint.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initSlint();
  runApp(const SevenGuisApp());
}

class SevenGuisApp extends StatelessWidget {
  const SevenGuisApp({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp(
        title: '7GUIs',
        theme: ThemeData(useMaterial3: true),
        home: const DemoListPage(),
      );
}

class _Demo {
  const _Demo(this.title, this.load);
  final String title;
  final SlintComponent Function() load;
}

class DemoListPage extends StatelessWidget {
  const DemoListPage({super.key});

  static final demos = <_Demo>[
    _Demo('Counter', () => Counter.load()),
    _Demo('Temperature converter', () => TempConv.load()),
    _Demo('Flight booker', buildBooker),
    _Demo('Timer', () => MainWindow.load()),
    _Demo('CRUD', buildCrud),
  ];

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('7GUIs')),
        body: ListView.separated(
          itemCount: demos.length,
          separatorBuilder: (_, __) => const Divider(height: 1),
          itemBuilder: (context, index) {
            final demo = demos[index];
            return ListTile(
              title: Text(demo.title),
              onTap: () => Navigator.of(context).push(
                MaterialPageRoute<void>(
                  builder: (_) => Scaffold(
                    appBar: AppBar(title: Text(demo.title)),
                    body: SlintView(load: demo.load),
                  ),
                ),
              ),
            );
          },
        ),
      );
}

Booker buildBooker() {
  final app = Booker.load();
  app.onValidateDate((date) {
    final parts = date.split('.');
    if (parts.length != 3) return false;
    final day = int.tryParse(parts[0]);
    final month = int.tryParse(parts[1]);
    final year = int.tryParse(parts[2]);
    if (day == null || month == null || year == null) return false;
    try {
      final parsed = DateTime(year, month, day);
      return parsed.year == year && parsed.month == month && parsed.day == day;
    } on FormatException {
      return false;
    }
  });
  app.onCompareDate((a, b) {
    DateTime? parse(String date) {
      final parts = date.split('.');
      if (parts.length != 3) return null;
      final day = int.tryParse(parts[0]);
      final month = int.tryParse(parts[1]);
      final year = int.tryParse(parts[2]);
      if (day == null || month == null || year == null) return null;
      try {
        return DateTime(year, month, day);
      } on FormatException {
        return null;
      }
    }

    final first = parse(a);
    final second = parse(b);
    if (first == null || second == null) return false;
    return !first.isAfter(second);
  });
  return app;
}

class _Name {
  _Name(this.first, this.last);
  String first;
  String last;
  String get label => '$last, $first';
}

CrudWindow buildCrud() {
  final app = CrudWindow.load();
  final names = <_Name>[
    _Name('Hans', 'Emil'),
    _Name('Max', 'Mustermann'),
    _Name('Roman', 'Tisch'),
  ];
  var visible = <int>[0, 1, 2];

  void refresh() {
    final prefix = app.prefix;
    visible = [
      for (var i = 0; i < names.length; i++)
        if (names[i].label.startsWith(prefix)) i,
    ];
    app.namesList = [
      for (final i in visible) {'text': names[i].label},
    ];
  }

  app.onCreateClicked(() {
    names.add(_Name(app.name, app.surname));
    refresh();
  });
  app.onUpdateClicked(() {
    final row = app.currentItem;
    if (row < 0 || row >= visible.length) return;
    final name = names[visible[row]];
    name.first = app.name;
    name.last = app.surname;
    refresh();
  });
  app.onDeleteClicked(() {
    final row = app.currentItem;
    if (row < 0 || row >= visible.length) return;
    names.removeAt(visible[row]);
    refresh();
  });
  app.onPrefixEdited(refresh);
  refresh();
  return app;
}
