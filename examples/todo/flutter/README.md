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

fvm flutter run -d macos
```

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
fvm flutter test
```
