# Gallery, in Dart

The Slint widgets gallery as a Flutter application.

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/gallery/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs
flutter create . --platforms=macos --project-name gallery
flutter run -d macos
```
