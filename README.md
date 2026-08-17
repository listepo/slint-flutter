# Slint for Dart and Flutter

Write the user interface in `.slint`, the logic in Dart.

```dart
import 'package:my_app/ui/counter.slint.dart';

void main() {
  final app = CounterWindow.load()
    ..statusMessage = 'Ready from Dart'
    ..onCountChanged((value) => print('Count: $value'));

  app.currentCount = 1;
  app.invokeResetCounter();
  app.run();
}
```

Three packages live here:

| Package | What it is |
| --- | --- |
| [`slint`](./slint/pubspec.yaml) | The binding itself. Pure Dart, no Flutter dependency: `dart:ffi` natively, WebAssembly on the web. |
| [`slint_generator`](./slint_generator) | The `build_runner` builder that turns a `.slint` file into a typed Dart API. A dev dependency only. |
| [`slint_flutter`](./slint_flutter) | A `SlintView` widget that renders a Slint UI inside a Flutter app. |

## Generate a Typed Dart API

Put `.slint` files under your application's `lib` directory.
For example, use `lib/ui/counter.slint`.

Add `slint` to the application's `pubspec.yaml`, and `slint_generator` with
`build_runner` next to it. Only the generator needs the second package, so it
stays out of the shipped application:

```yaml
dependencies:
  slint:
    path: path/to/slint-dart/slint

dev_dependencies:
  build_runner: ^2.4.9
  slint_generator:
    path: path/to/slint-dart/slint_generator
```

Generation runs `slint-dart-generate`, a binary built from the
[`codegen`](./native/codegen) crate. The builder finds it under `native/target/`,
builds it
on first use if it isn't there, and `SLINT_DART_GENERATE` points at a copy
elsewhere. It does not need `libslint_dart`, so generating a wrapper and
building the runtime are independent.

```sh
cd native
cargo build --release -p slint-dart-codegen    # optional: pre-build it
```

Generate the Dart wrapper once:

```sh
dart run build_runner build --delete-conflicting-outputs
```

Keep the generator running during development:

```sh
dart run build_runner watch --delete-conflicting-outputs
```

Configure import search paths and the widget style in the application's
`build.yaml`:

```yaml
targets:
  $default:
    builders:
      slint_generator|slint:
        options:
          style: material
          include_paths:
            - lib/ui/includes
```

Each `include_paths` entry is relative to the application package unless it is
absolute.
Imported files must remain inside the package so `build_runner` can watch them.
The generator compiles those files — and the configured style — into the
wrapper. `load()` instantiates that compilation and does not take `style` or
`includePaths`.

By default the builder writes `lib/ui/counter.slint.dart` next to the input
file.
Set `output_dir` to generate the wrappers into a custom folder instead,
mirroring each source's path under `lib`:

```yaml
targets:
  $default:
    builders:
      slint_generator|slint:
        options:
          output_dir: lib/generated
```

`lib/ui/counter.slint` then becomes `lib/generated/ui/counter.slint.dart`,
imported as `package:my_app/generated/ui/counter.slint.dart`.
`output_dir` is relative to the package unless it is absolute, and must stay
inside it.
It applies to `.slint` files under the package's `lib` directory.

The builder regenerates a wrapper when its input or one of its package-local
Slint dependencies changes.
Don't edit the generated file.

Import the wrapper through your package:

```dart
import 'package:my_app/ui/counter.slint.dart';

final app = CounterWindow.load();
```

`load()` instantiates the component compiled into the wrapper at generate time.
It does not read the `.slint` file, so packaged Flutter apps do not need to
ship that file as an asset.

The untyped `loadFile()` / `loadSource()` API is still there when the component
isn't known at build time, and still compiles `.slint` at runtime.

Generated Dart types use UpperCamelCase, and generated fields and methods use
lowerCamelCase:

| Slint declaration | Generated Dart API |
| --- | --- |
| `export component counter-window` | `CounterWindow.load()` |
| `in-out property <int> current-count` | `currentCount` |
| `callback count-changed(int)` | `onCountChanged(...)`, `invokeCountChanged(...)` |
| `public function reset_counter()` | `invokeResetCounter()` |

The generated wrapper keeps the Slint spelling for runtime lookup, so the
public Dart identifier is never reconstructed from the Dart name.
The compiler reports those names in their canonical form, with `_` written as
`-`, so `reset_counter` is looked up as `"reset-counter"`.
`-` and `_` name the same member everywhere in this binding, in generated
wrappers and in the string-based API below, exactly as they do in `.slint`.

Code generation is optional.
Use `loadFile()` or `loadSource()` and the string-based `ComponentInstance` API
when the component isn't known at build time:

```dart
import 'package:slint/slint.dart';

final app = loadFile('ui/todo.slint');
app['todo-model'] = [
  {'title': 'Write the Dart part', 'checked': false},
];
app.setCallback('todo-added', (args) {
  final items = app['todo-model']! as List<Object?>;
  app['todo-model'] = [...items, {'title': args[0], 'checked': false}];
});
```

See the [`slint_flutter` code-generation example](./slint_flutter/example) for a
complete Flutter application.

## Building

Two Rust crates, and only one of them ends up in an application:

| Crate | When it runs | What it produces |
| --- | --- | --- |
| [`codegen`](./native/codegen) (`slint-dart-generate`) | generate time, on the developer's machine | one `.slint.dart` per `.slint`, and nothing else |
| [`native`](./native) (`libslint_dart`) | runtime, inside the application | the C ABI the Dart package calls |

Code generation emits **Dart only**. It never writes Rust, C or C++, and an
application never compiles a generated artifact — the `.slint.dart` is ordinary
Dart source, and the same prebuilt `libslint_dart` runs every application. That
is also why the library carries the Slint interpreter: the wrapper hands it the
compiled module to instantiate at `load()`.

```sh
cd native
cargo build --release -p slint-dart
```

`package:slint` finds the library by looking at `SLINT_DART_LIBRARY` first, then
walking up from the working directory, the running executable, the running
script, and the linked package root for a `native/target/release` or
`native/target/debug` copy, and finally asking the platform loader.
That last step is the one a packaged application takes.

### The `dart:ffi` bindings are generated

The 38 entry points are not declared by hand on both sides. cbindgen writes a
C header from `native/rust/`, and ffigen turns that into
[`slint/lib/src/ffi.g.dart`](./slint/lib/src/ffi.g.dart):

```sh
cargo install cbindgen        # once
bazel run //scripts:generate_bindings
```

`ffi.g.dart` is committed, so building the package needs neither tool. Run the
target with `-- --check` in CI: it regenerates into a temporary copy and fails if
the result differs, which is what stops a changed Rust signature from silently
disagreeing with Dart.

[`ffigen.yaml`](./slint/ffigen.yaml) carries a rename map so the generated
methods keep the names the rest of the package calls — those aren't derivable
from the C names, since `slint_dart_compiler_build_from_path` is `buildFromPath`
while `slint_dart_instance_show` is `instanceShow`.

[`ffi.dart`](./slint/lib/src/ffi.dart) keeps only what a generator can't
produce: finding the library at runtime, and the `takeEnvelope` / `takeString`
/ `withNativeString` helpers that convert the JSON envelope. Those three are
the single place that casts between the generated `Pointer<Char>` and
`package:ffi`'s `Pointer<Utf8>`.

### Flutter builds the library automatically

The package ships a [build hook](https://dart.dev/tools/hooks)
(`hook/build.dart`): every `flutter build` and `flutter run` that has `slint`
in its dependency graph runs it, invokes `cargo build --release -p slint-dart`,
and bundles the result into the application.
On macOS the library becomes `slint_dart.framework` inside the app bundle,
which `package:slint` finds at runtime.

A Flutter build needs the Rust toolchain (`cargo` and `rustc` on `PATH`) and
only supports the host platform; cross-compiling to another OS or architecture
is not supported yet. iOS takes the route below instead. Android is the
exception: the hook cross-compiles each ABI with `cargo-ndk` against the
Android NDK, so an Android build needs `cargo-ndk` installed and an NDK
(usually from Android Studio). The hook builds one architecture per invocation
and Flutter places each `libslint_dart.so` into the right `jniLibs` ABI
directory (`armeabi-v7a`, `arm64-v8a`, `x86_64`).
Set `SLINT_DART_LIBRARY` if you want to build the library yourself, or pin a
profile for the hook (debug builds are faster to produce):

```yaml
# pubspec.yaml of the Flutter application
hooks:
  user_defines:
    slint:
      cargo_profile: debug
```

The hook is [`native_toolchain_rust`](https://pub.dev/packages/native_toolchain_rust),
which owns the target triples, the Android NDK toolchain, and the rustup
version pinned in [`native/rust-toolchain.toml`](./native/rust-toolchain.toml).
It re-runs cargo only when cargo's own depfile says an input changed.

### Release artifacts come from Bazel

Bazel drives the multi-target builds: one cargo invocation per target triple,
fanned out in parallel and cached per artifact.

```sh
bazel build //native:release        # xcframework + AAR + generator
bazel build //native:xcframework    # Apple only
bazel build //native:aar            # Android only
```

`//native:xcframework` produces `SlintDart.xcframework.zip` with
`slint_dart.framework` for the device (`arm64`), the simulator (`arm64` and
`x86_64`) and macOS (`arm64` and `x86_64`). Add it to the Runner target's
*Frameworks, Libraries, and Embedded Content* with **Embed & Sign**. These are
ordinary dynamic frameworks — the same shape the hook bundles on macOS — so
`package:slint` opens them from the app bundle with no special case.

`//native:aar` produces `slint-dart.aar` carrying `libslint_dart.so` for
`armeabi-v7a`, `arm64-v8a` and `x86_64` under `jni/`, which Gradle unpacks into
the application's `jniLibs`.

Both carry only the software renderer, since the binding always draws through
the embedded surface on mobile.

Cargo still compiles the Rust. The crate graph is 338 crates deep and includes
`skia-bindings`, which downloads and builds Skia from its own build script, so
the rules that call cargo are tagged `local`, `no-sandbox` and
`requires-network` rather than being ported to rules_rust.

### The web loads a WebAssembly module

A browser has no `dart:ffi`, so `package:slint` reaches the same Rust code
through WebAssembly instead: `native/rust/wasm.rs` exposes the runtime to
JavaScript with `wasm-bindgen`, and
[`backend_web.dart`](./slint/lib/src/backend_web.dart) calls it over
`dart:js_interop`. Everything above that line — properties, callbacks, the
software renderer — is the code every other platform runs.

Build the module into the application's `web/` directory:

```sh
bazel run //scripts:build_wasm -- path/to/app/web
```

That writes `slint_dart.js` and `slint_dart_bg.wasm` (about 15 MB, roughly
4 MB over the wire once the server compresses it). Loading is asynchronous, so
`await initSlint()` before the first component:

```dart
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initSlint();
  runApp(const MyApp());
}
```

It returns immediately on every other platform, so call it unconditionally.
`initSlint(scriptUrl: './slint_dart.js')` points elsewhere; the argument is a
module specifier, so a relative path needs the leading `./`.

Two entry points have no meaning in a browser and throw `SlintException`:
`loadFile()`, because there is no filesystem — the untyped API must fetch the
`.slint` source and use `loadSource()` — and `run()`/`runEventLoop()`, because
the browser owns the event loop. Generated wrappers use `load()` and do not
need the original file. `SlintView` drives the frames instead.

Because the software renderer rasterizes every pixel itself, this build turns
on `i-slint-core`'s `image-decoders` and `svg` features, which a wasm build
normally leaves to the browser. Without them `std-widgets` icons panic with
"The image cannot be rendered".

## Two ways to show a UI

### Slint owns the window

`ComponentInstance.run()` opens a native window and runs Slint's event loop,
the way the Python and Node.js bindings do. Use this for a plain
`dart run` application.

**This does not work on macOS**, and not inside Flutter on any platform: the
Dart VM does not run `main()` on the process main thread, which is where a
native event loop has to live.
Both cargo profiles here unwind on panic, so the binding catches that and
reports it as a `SlintException` instead of aborting the host application.
A profile configured with `panic = "abort"` would lose that.
On Linux and Windows it works.

### Slint draws into a buffer you own

[`SlintSurface`](./slint/lib/src/embedded.dart) installs Slint's software renderer
and hands you the frame as pixels. There is no event loop and no thread
requirement, so it works everywhere — including inside Flutter, which is what
`slint_flutter` builds on:

```dart
import 'package:slint_flutter/slint_flutter.dart';
import 'package:my_app/ui/counter.slint.dart';

// Inside a widget tree:
SlintView(load: CounterWindow.load)
```

Generated `load()` does not read the `.slint` file, so this form is what
packaged Flutter apps use. The untyped API still compiles source at runtime:
`SlintView(load: () => loadFile('ui/todo.slint'))` on desktop, or
`loadSource` after fetching the text on the web.

Driving it yourself is the same three steps `SlintView` performs each frame:

```dart
final surface = SlintSurface()..resize(800, 600, scaleFactor: 2.0);
final app = loadFile('ui/todo.slint')..show();   // after the surface exists

surface.tick();                                  // advance timers, animations
final pixels = surface.render();                 // RGBA, premultiplied, or null
surface.dispatchPointer(PointerEventKind.moved, x: 10, y: 20);
```

## Values

Values cross the boundary as JSON, which means they arrive in Dart as ordinary
data:

| Slint | Dart |
| --- | --- |
| `int`, `float`, `length`, `duration`, `percent` | `num` |
| `string` | `String` |
| `bool` | `bool` |
| `[T]` (a model) | `List` |
| a struct | `Map<String, Object?>` |
| `color`, `brush` | `String`, CSS-style: `'#00c1e2'`, `'#00c1e2ff'` |
| `image` | `SlintImage` |
| an enum | `String`, the variant name |
| a callback with no return | the handler returns `null` |

Models are read and written whole. Dart owns the list, and assigning the
property publishes it:

```dart
final items = [...app['todo-model']! as List<Object?>];
items.add({'title': 'One more', 'checked': false});
app['todo-model'] = items;
```

Globals work the same way through `app.global('PrinterQueue')`.

Pass an image in with `SlintImage.fromPath`, `fromEncoded`, `fromSvg`, or
`fromRgba`. `@image-url` inside `.slint` still works. The untyped
`getProperty` returns `null`, a path string, or a map; generated getters wrap
that in `SlintImage`.

`initTranslations` installs a Dart function for `@tr(...)` strings. Create a
`SlintSurface` or a component first so the Slint platform exists:

```dart
initTranslations((string, {context, plural, n = 0}) => catalog[string] ?? string);
```

## Testing

```sh
cd native
cargo test -p slint-dart
```

The Dart tests need a backend that opens no window, which is what `SlintSurface`
installs: every suite creates one before loading a component, so the software
renderer is the platform and no native window is ever opened. That works on
every host, including macOS, where the Dart VM's worker thread cannot own a
native event loop.

```sh
cd native
cargo build -p slint-dart --no-default-features --features renderer-software
flutter pub get   # at the repo root, once
flutter pub run melos run test:native     # slint + slint_flutter
flutter pub run melos run test            # slint_generator (no native library)
```

Or run each package directly:

```sh
cd native
cargo build -p slint-dart --no-default-features --features renderer-software
cd ../slint
SLINT_DART_LIBRARY="$PWD/../native/target/debug/libslint_dart.dylib" dart test
cd ../slint_flutter
SLINT_DART_LIBRARY="$PWD/../native/target/debug/libslint_dart.dylib" flutter test
cd ../slint_generator
dart test    # a fake generator, so no native library is needed
```

Bazel Starlark tooling (buildifier, packaging rules, codegen smoke builds):

```sh
bazel test //tools/tests:tooling
```

Pinning `SLINT_DART_LIBRARY` is what keeps the suites on that debug build:
`dart test` also runs the build hook, which produces a default-feature release
library. Running the tests therefore needs the Rust toolchain either way.

There is no `SLINT_BACKEND=testing` here. It would need a `backend-testing`
feature, and the published `i-slint-backend-testing` cannot build the
`internal` feature that the backend selector turns on with it — the crate
embeds a font directory that only exists inside the Slint repository.

## Toolchain

The whole toolchain is pinned with [mise](https://mise.jdx.dev) in `.mise.toml`:
the Rust toolchain (`rust`), the Dart SDK (`dart`), the Flutter SDK
(`flutter`), [Bazel](https://bazel.build) (`bazel`), and
[buildifier](https://github.com/bazelbuild/buildtools) (`buildifier`) for
Starlark/Bazel files. Run `mise install` once to fetch the pinned versions;
with the mise shims on `PATH`, every command above is available as plain
`dart`, `flutter`, `cargo`, `rustc`, `bazel`, and `buildifier`.
`cbindgen` (for regenerating the FFI bindings) is a Cargo binary, installed with
`cargo install cbindgen`.

### Dart workspace (Melos)

Dart and Flutter packages are managed as a [pub workspace](https://dart.dev/tools/pub/workspaces)
with [Melos](https://melos.invertase.dev). Configuration lives in the root
[`pubspec.yaml`](./pubspec.yaml) under the `melos` key (Melos 7+ no longer uses
a separate `melos.yaml`). Melos is pinned there as a dev dependency — it is not
in the mise registry, so invoke it with `flutter pub run melos` from the repo root (or
`mise run melos --`).

```sh
flutter pub get              # once, installs the pinned melos version
flutter pub run melos bootstrap  # link workspace packages and fetch dependencies
flutter pub run melos run analyze
flutter pub run melos run test              # pure-Dart packages (slint_generator, …)
flutter pub run melos run test:native       # slint + slint_flutter (needs debug libslint_dart)
flutter pub run melos run test:flutter      # Flutter example tests
flutter pub run melos run format
flutter pub run melos run codegen           # build_runner in apps that depend on slint_generator
```

Core packages (`slint`, `slint_flutter`, `slint_generator`,
`examples/test_support`) and every Flutter example under `examples/*/flutter`
are workspace members. Examples stay `publish_to: none`; when the binding packages
are ready for pub.dev, remove `publish_to: none` from those three packages and
use `flutter pub run melos version` / `flutter pub run melos publish` (Melos skips packages
that still declare `publish_to: none`).

## Examples

- [`slint_flutter/example`](./slint_flutter/example) — the code-generation
  example, a Flutter application with a generated `CounterWindow` wrapper.
- [`examples/`](./examples) — Flutter demos (`todo`, `gallery`, `memory`,
  `carousel`, and others). Each app lives under `examples/<name>/flutter`
  and uses a generated typed wrapper via `Component.load()`. See
  [`examples/README.md`](./examples/README.md).

## Limitations

- One `SlintSurface` per isolate: the software renderer owns a single surface.
- Everything must be used from the main isolate, which is where the Slint
  platform lives. This matches the Python and Node.js bindings.
- There is no runtime `.slint` loader. Every UI is compiled ahead of time
  into `libslint_dart` (or the wasm module) and instantiated with generated
  `load()`.
- Dart web does not yet look up generated `slint_aot_*` wasm exports, so
  `AotHandle.create` throws there. The wasm module itself does not include
  the compiler.
- A Rust panic on the web aborts the module instead of unwinding, so the
  `catch_unwind` guards that turn a panic into a `SlintException` elsewhere do
  not apply there. The message still reaches the browser console.
