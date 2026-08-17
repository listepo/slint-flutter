# Async I/O stock ticker, in Dart

Fetches quotes from stooq.com and shows them in a Flutter `SlintView`.

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/async-io/flutter
dart pub get
dart run build_runner build --delete-conflicting-outputs
flutter create . --platforms=macos --project-name async_io
flutter run -d macos
```
