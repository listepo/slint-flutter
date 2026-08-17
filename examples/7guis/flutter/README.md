# 7GUIs, in Dart

A Flutter launcher for several [7GUIs](https://eugenkiss.github.io/7guis/)
tasks. Each demo uses a generated typed wrapper via `Component.load()`.

Included: Counter, Temperature converter, Flight booker, Timer, and CRUD.
Circle drawer and Cells are not ported yet (they need more host-side
canvas / spreadsheet logic).

```sh
# From the repository root:
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd ../examples/7guis/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs
fvm flutter create . --platforms=macos --project-name=seven_guis
fvm flutter run -d macos
```
