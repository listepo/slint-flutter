# Slide puzzle, in Dart

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/slide_puzzle/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs
fvm flutter create . --platforms=macos --project-name slide_puzzle
fvm flutter run -d macos
```
