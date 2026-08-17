import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slint_flutter/slint_flutter.dart';

/// The route builder [WidgetsApp] needs to turn `home` into a page route.
PageRoute<T> slintExampleRouteBuilder<T>(
        RouteSettings settings, WidgetBuilder builder) =>
    PageRouteBuilder<T>(
      settings: settings,
      pageBuilder: (context, animation, secondaryAnimation) => builder(context),
    );

/// Pumps a [SlintView] inside a sized box for widget tests.
Future<T> pumpSlintExample<T extends SlintComponent>({
  required WidgetTester tester,
  required T Function() load,
  required ValueKey<String> viewKey,
  double width = 800,
  double height = 600,
}) async {
  late T loaded;
  await tester.pumpWidget(WidgetsApp(
    color: const Color(0xff000000),
    pageRouteBuilder: slintExampleRouteBuilder,
    home: SizedBox(
      width: width,
      height: height,
      child: SlintView(
        key: viewKey,
        load: () {
          loaded = load();
          return loaded;
        },
      ),
    ),
  ));
  await tester.pump();
  expect(find.byKey(viewKey), findsOneWidget);
  return loaded;
}
