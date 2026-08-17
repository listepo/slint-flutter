# Async I/O stock ticker, in Dart

Fetches quotes from stooq.com and shows them in a Flutter `SlintView`.

```sh
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart
cd ../examples/async-io/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs
fvm flutter create . --platforms=macos --project-name async_io
fvm flutter run -d macos
```
