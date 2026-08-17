# Image filter demo, in Dart

A Flutter application driven through [`slint_flutter`](../../../slint_flutter).
It uses a generated typed wrapper via `Component.load()`; it does not read
`.slint` source at runtime.

```sh
# From the repository root:
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd ../examples/imagefilter/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs
flutter create . --platforms=macos --project-name=imagefilter
flutter run -d macos
```
