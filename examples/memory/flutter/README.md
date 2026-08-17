# Memory, in Dart

A Flutter memory-game demo through [`slint_flutter`](../../../slint_flutter).

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/memory/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs
fvm flutter create . --platforms=macos --project-name memory
fvm flutter run -d macos
```
