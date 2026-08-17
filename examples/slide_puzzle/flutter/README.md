# Slide puzzle, in Dart

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/slide_puzzle/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs
flutter create . --platforms=macos --project-name slide_puzzle
flutter run -d macos
```
