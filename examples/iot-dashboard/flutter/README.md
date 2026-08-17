# IoT dashboard, in Dart

A Flutter application driven through [`slint_flutter`](../../../slint_flutter).
It uses a generated `MainWindow` wrapper. `MainWindow.load()` instantiates the
native component; it does not read `.slint` source at runtime.
[`lib/ui/main.slint`](lib/ui/main.slint) is the UI packaged with the app.

```sh
# From the repository root:
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd ../examples/iot-dashboard/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs

# First time only — create a platform runner:
fvm flutter create . --platforms=macos --project-name iot_dashboard

fvm flutter run -d macos
```

Keep the generator running while you edit the `.slint` file:

```sh
fvm dart run build_runner watch --delete-conflicting-outputs
```
