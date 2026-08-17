# Todo, in Dart

The todo example driven from Dart, shown inside a Flutter application through
[`slint_flutter`](../../../slint_flutter).
It uses a generated `MainWindow` wrapper. `MainWindow.load()` instantiates the
native component compiled into `libslint_dart`; it does not read `.slint`
source at runtime.
[`lib/ui/todo.slint`](lib/ui/todo.slint) mirrors the shared
[`.slint` file](../ui/todo.slint) so `build_runner` can see it inside the
package. It omits the Rust-only `@rust-attr(…serde…)` on `TodoItem`, which
would otherwise pull `serde` into the AOT sidecar compiled into
`libslint_dart`. Packaged apps do not ship the `.slint` file.

Build the generate-time compiler and the runtime library, generate the typed
wrapper (and AOT Rust sidecar), then create a platform runner and run:

```sh
# From the repository root:
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd examples/todo/flutter
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs

# Rebuild the runtime so `.dart_tool/slint/aot` is compiled in
# (the Flutter build hook does this automatically on `flutter run`/`build`):
SLINT_DART_AOT_DIR="$PWD/.dart_tool/slint/aot" \
  cargo build --release -p slint-dart

fvm flutter create --platforms=macos --project-name=todo .
fvm flutter run -d macos
```

Use `linux` or `windows` in place of `macos` as needed.
The runner directories are generated, not committed.
If `flutter run` fails because Xcode cannot access
`macos/Flutter/ephemeral/Packages/.packages/FlutterFramework`, delete `macos/`
and run `flutter create` again.
A runner generated without Swift Package Manager package references cannot be
migrated in place. Recreating one platform's runner can leave another's in that
state, so expect to redo this after adding a platform.

Looking up generated `slint_aot_*` exports from Dart web is not wired yet, so
the Chrome / wasm path does not yet instantiate `MainWindow.load()`. When that
lands, the same generate step plus
`../../../scripts/build_slint_dart_wasm.bash web` will apply; `initSlint()` in
[`lib/main.dart`](lib/main.dart) already awaits the wasm module on the web.

Keep the generator running while you edit the `.slint` file:

```sh
fvm dart run build_runner watch --delete-conflicting-outputs
```

Widget tests use the embedded software renderer (`SlintSurface` via
`SlintView`) and a debug build of `libslint_dart` that includes the AOT
sidecar — not a release library without `MainWindow`:

```sh
fvm dart run build_runner build --delete-conflicting-outputs
SLINT_DART_AOT_DIR="$PWD/.dart_tool/slint/aot" \
  cargo build -p slint-dart --no-default-features --features renderer-software
SLINT_DART_LIBRARY="$PWD/../../../target/debug/libslint_dart.dylib" \
  fvm flutter test
```

Use `libslint_dart.so` on Linux and `slint_dart.dll` on Windows.
