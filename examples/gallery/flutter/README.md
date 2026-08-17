# Gallery, in Dart

The Slint widgets gallery as a Flutter application.

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/gallery/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs
fvm flutter create . --platforms=macos --project-name gallery
fvm flutter run -d macos
```
