# Dial, in Dart

A Flutter application driven through [`slint_flutter`](../../../slint_flutter).
It uses a generated `AppWindow` wrapper. `AppWindow.load()` instantiates the
native component; it does not read `.slint` source at runtime.
[`lib/ui/dial.slint`](lib/ui/dial.slint) is the UI packaged with the app.

```sh
# From the repository root:
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd ../examples/dial/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs

# First time only — create a platform runner:
flutter create . --platforms=macos --project-name dial

flutter run -d macos
```

Keep the generator running while you edit the `.slint` file:

```sh
dart run build_runner watch --delete-conflicting-outputs
```
