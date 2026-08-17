import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:todo/ui/todo.slint.dart';

TodoItem todo(String title, {bool checked = false}) =>
    TodoItem(title: title, checked: checked);

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // On the web this fetches and instantiates the Slint WebAssembly module
  // served from `web/`; elsewhere it returns immediately.
  await initSlint();
  runApp(const TodoApp());
}

class TodoApp extends StatelessWidget {
  const TodoApp({super.key});

  @override
  Widget build(BuildContext context) => WidgetsApp(
        color: const Color(0xff000000),
        pageRouteBuilder: _defaultRouteBuilder,
        title: 'Slint todo',
        home: SlintView(load: buildTodoUi),
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

MainWindow buildTodoUi() {
  final app = MainWindow.load();

  // Dart owns the list; the `todo-model` property is the view of it. Every
  // mutation writes the whole list back, which keeps the sorting and filtering
  // below down to ordinary list operations.
  final items = [
    todo('Implement the .slint file', checked: true),
    todo('Do the Rust part', checked: true),
    todo('Make the C++ code'),
    todo('Write some JavaScript code'),
    todo('Write the Dart part'),
    todo('Test the application'),
    todo('Ship to customer'),
    todo('???'),
    todo('Profit'),
  ];

  void refresh() {
    // Pick up the checkboxes the user toggled in the UI before rewriting the
    // model, otherwise sorting or filtering would discard them.
    for (final row in app.todoModel) {
      final index = items.indexWhere((item) => item.title == row.title);
      items[index] = todo(row.title, checked: row.checked);
    }

    final visible = items.toList();
    if (app.hideDoneItems) {
      visible.removeWhere((item) => item.checked);
    }
    if (app.isSortByName) {
      visible.sort(
        (a, b) => a.title.toLowerCase().compareTo(b.title.toLowerCase()),
      );
    }
    app.todoModel = visible;
  }

  app.onTodoAdded((title) {
    items.add(todo(title));
    refresh();
  });

  app.onRemoveDone(() {
    refresh();
    items.removeWhere((item) => item.checked);
    refresh();
  });

  app.onApplySortingAndFiltering(refresh);

  // `SystemNavigator.pop` rather than `exit`: `dart:io` does not exist on the
  // web, and this is what Flutter offers everywhere.
  app.onPopupConfirmed(SystemNavigator.pop);

  app.showHeader = true;
  app.todoModel = items;
  return app;
}
