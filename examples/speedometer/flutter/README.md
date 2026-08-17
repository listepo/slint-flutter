# Speedometer, in Dart

A Flutter application driven through [`slint_flutter`](../../../slint_flutter).
It uses a generated `MainWindow` wrapper. `MainWindow.load()` instantiates the
native component; it does not read `.slint` source at runtime.
[`lib/ui/demo.slint`](lib/ui/demo.slint) is the UI packaged with the app.

```sh
# From the repository root:
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd ../examples/speedometer/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs

# First time only — create a platform runner:
flutter create . --platforms=macos --project-name speedometer

flutter run -d macos
```

Keep the generator running while you edit the `.slint` file:

```sh
dart run build_runner watch --delete-conflicting-outputs
```
