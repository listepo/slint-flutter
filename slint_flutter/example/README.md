# Dart Code-Generation Example

A minimal Flutter application that shows a generated `CounterWindow` wrapper
through [`slint_flutter`](../lib/slint_flutter.dart).
`CounterWindow.load()` instantiates the native component compiled into the
generated wrapper; it does not read `counter.slint` at runtime.

Build the generate-time compiler and the runtime library, generate the typed
wrapper, then create a platform runner and run:

```sh
# From the repository root:
cd native
cargo build --release -p slint-dart-codegen
cargo build --release -p slint-dart

cd slint_flutter/example
dart pub get
dart run build_runner build --delete-conflicting-outputs

flutter run -d macos
```

Use `linux` or `windows` in place of `macos` as needed.
The runner directories are generated, not committed.

The generated `CounterWindow` wrapper exposes `current-count` as `currentCount`,
`status_message` as `statusMessage`, `count-changed` as `onCountChanged()`, and
`reset_counter` as `invokeResetCounter()`.

Keep the generator running while editing `lib/ui/counter.slint`:

```sh
dart run build_runner watch --delete-conflicting-outputs
```

Widget tests use the embedded software renderer (`SlintSurface` via
`SlintView`) and a debug build of `libslint_dart`:

```sh
dart run build_runner build --delete-conflicting-outputs
cd ../../native
cargo build -p slint-dart --no-default-features --features renderer-software
SLINT_DART_LIBRARY="$PWD/../../native/target/debug/libslint_dart.dylib" \
  flutter test
```

Use `libslint_dart.so` on Linux and `slint_dart.dll` on Windows.

Looking up generated `slint_aot_*` exports from Dart web is not wired yet, so
the Chrome / wasm path does not yet instantiate `CounterWindow.load()`. When that
lands, the same generate step plus
`bazel run //scripts:build_wasm -- web` will apply; `initSlint()` in
[`lib/main.dart`](lib/main.dart) already awaits the wasm module on the web.
