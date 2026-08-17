import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:seven_guis/main.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('launcher lists all ported 7GUIs demos', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: DemoListPage()));
    expect(find.text('Counter'), findsOneWidget);
    expect(find.text('Temperature converter'), findsOneWidget);
    expect(find.text('Flight booker'), findsOneWidget);
    expect(find.text('Timer'), findsOneWidget);
    expect(find.text('CRUD'), findsOneWidget);
  });

  test('buildBooker validates European dates', () {
    final booker = buildBooker();
    expect(booker.invokeValidateDate('27.03.2014'), isTrue);
    expect(booker.invokeValidateDate('not-a-date'), isFalse);
    expect(booker.invokeCompareDate('01.01.2020', '31.12.2020'), isTrue);
    expect(booker.invokeCompareDate('31.12.2020', '01.01.2020'), isFalse);
  });

  test('buildCrud manages the name list', () {
    final window = buildCrud();

    expect(window.namesList.length, 3);
    window.prefix = 'M';
    window.invokePrefixEdited();
    expect(window.namesList.length, 1);

    window.prefix = '';
    window.invokePrefixEdited();
    expect(window.namesList.length, 3);

    window.name = 'Ada';
    window.surname = 'Lovelace';
    window.invokeCreateClicked();
    expect(window.namesList.length, 4);
    expect(
      window.namesList.any((row) => row['text'] == 'Lovelace, Ada'),
      isTrue,
    );
  });
}
