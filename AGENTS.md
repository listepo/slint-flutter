# AGENTS.md — slint-dart

This file provides guidance to AI coding assistants working in the Dart and
Flutter bindings for Slint. The UI is written in `.slint`, the logic in Dart.

This repository was extracted from the Slint repository's `api/flutter`
directory and now builds against released Slint crates from crates.io, so
nothing here needs a Slint checkout.

Five pieces live here:

| Piece | What it is |
| --- | --- |
| [`native/rust/`](./native/rust) | `slint-dart`, a Rust `cdylib` exposing a plain C ABI over `slint-interpreter`. The runtime; this is what an application ships. |
| [`native/codegen/`](./native/codegen) | `slint-dart-codegen` and its `slint-dart-generate` binary. Generate-time only: it holds the Slint compiler and emits Dart. Never linked into an application. |
| [`slint/`](./slint) | `package:slint`, the binding itself. Pure Dart over `dart:ffi`, no Flutter dependency. |
| [`slint_generator/`](./slint_generator) | The `build_runner` builder that turns a `.slint` file into a typed Dart API. Dev dependency only. |
| [`slint_flutter/`](./slint_flutter) | A `SlintView` widget that renders a Slint UI inside a Flutter app. |

**Code generation emits Dart and only Dart.** No Rust, no C, no C++, nothing an
application has to compile: a generated `.slint.dart` is ordinary Dart source
carrying the compiled `.slint` graph as data, and the same prebuilt
`libslint_dart` runs every application. If a change would make the generator
write another language, or make an application rebuild the runtime for its own
UI, it is the wrong change — see the Limitations section of the README for why
a compiler-free runtime is not available instead.

## Prerequisites

The Dart and Flutter SDK is pinned with [FVM](https://fvm.app) in `.fvmrc`
(`"flutter": "stable"`), so every command below is available as `fvm dart …` and
`fvm flutter …`. Run `fvm install` once to fetch the pinned version.

The Dart tests need a native library that `cargo` builds, so the Rust toolchain
(`cargo`, `rustc`, and `cbindgen` for regenerating bindings) is also required.

## Build Commands

The `slint-dart` crate lives in [`native/`](./native): `Cargo.toml` sits there
and its sources are in `native/rust/`.

```sh
cd native
cargo build --release -p slint-dart      # the native library the bindings load
cd ../slint && fvm dart pub get
```

The `cdylib` is named `libslint_dart` (`libslint_dart.dylib` on macOS,
`libslint_dart.so` on Linux, `slint_dart.dll` on Windows) and lands in
`native/target/release/`.

`package:slint` finds the library by reading `SLINT_DART_LIBRARY` first, then
walking up from the working directory, the running executable, the running
script, and the linked package root for a `native/target/release` or
`native/target/debug` copy, and finally asking the platform loader (see
`slint/lib/src/ffi.dart`). Point `SLINT_DART_LIBRARY` at a built library to
override discovery.

## Testing

```sh
cd native
cargo test --workspace --no-default-features --features renderer-software
```

The Dart tests must open no window, which is what `SlintSurface` arranges: each
suite creates one before the first component, so the software renderer is the
Slint platform. There is deliberately no `backend-testing` feature — see the
comment in `native/Cargo.toml` for why the published crate cannot provide one.

```sh
cd native
cargo build -p slint-dart --no-default-features --features renderer-software
cd ../slint
SLINT_DART_LIBRARY="$PWD/../native/target/debug/libslint_dart.dylib" fvm dart test
cd ../slint_flutter
SLINT_DART_LIBRARY="$PWD/../native/target/debug/libslint_dart.dylib" fvm flutter test
```

The `slint_generator` builder tests use a fake generator and never load the
native library, so they need no `SLINT_DART_LIBRARY`:

```sh
cd slint_generator && fvm dart test && fvm dart analyze
```

`dart test` also runs the build hook, which produces a default-feature release
library — that is why the commands above pin `SLINT_DART_LIBRARY` to the debug
build they just made.

## The FFI bindings are generated

The C entry points are not declared by hand on both sides. cbindgen writes a C
header from `native/rust/`, and ffigen turns that into `slint/lib/src/ffi.g.dart`:

```sh
cargo install cbindgen        # once
./scripts/generate_slint_dart_bindings.bash
```

`ffi.g.dart` is committed, so building the package needs neither tool. Run the
script with `--check` in CI: it regenerates into a temporary copy and fails if
the result differs, which stops a changed Rust signature from silently
disagreeing with Dart.

If you change a signature in `native/rust/`, or add or remove an entry point, you must
regenerate `ffi.g.dart` (and `native/target/slint_dart.h`) with
`./scripts/generate_slint_dart_bindings.bash` and commit the result. The
`ffigen.yaml` rename map and the hand-written conversions in `ffi.dart` are
documented in the README.

## Architecture

### The Rust ABI (`native/rust/`)

- `native/rust/lib.rs` — the C ABI over `slint-interpreter`: compiler, instance,
  callbacks, timers, and the JSON envelope. `#[unsafe(no_mangle)]` exports are
  FFI; the handle types (`SlintCompiler`, `SlintComponentDefinition`, …) cross
  the ABI only behind opaque pointers declared in `native/cbindgen.toml`.
- `native/rust/embedded.rs` — embedded mode: Slint renders into a caller-owned buffer
  instead of opening a native window. This is what Flutter uses, because the
  Dart VM does not run `main()` on the process main thread and a second native
  window would not compose with the widget tree.
- `native/rust/compiled.rs` — decodes the module a generated wrapper carries and
  instantiates it through the interpreter, once per process, from memory. The
  encoder is `native/codegen/src/bundle.rs`; `MODULE_VERSION` must match on both sides.

The runtime has no `generate` entry point. Adding one would put the compiler
back in the shipped library.

### The generate-time compiler (`native/codegen/`)

- `native/codegen/src/lib.rs` — compiles one `.slint` and returns the Dart source,
  the files to watch, and any diagnostics. `OutputFormat::Llr` configures the
  compiler; the Rust, C++ and Python output formats are deliberately unused,
  and the crate does not enable their features.
- `native/codegen/src/dart.rs` — the `.slint` → `.slint.dart` generator. Its siblings
  live inside `i-slint-compiler`; this one lives here so the binding builds
  against a released Slint. It reads the compiler's LLR, all public API.
- `native/codegen/src/bundle.rs` — packs the `.slint` graph into the gzip+base64 blob
  the wrapper embeds, with imports rewritten to a virtual root and `@image-url`
  inlined as data URIs, so nothing is read from disk at `load()`.
- `native/codegen/src/main.rs` — the `slint-dart-generate` binary. Prints the JSON
  envelope the Dart builder reads; `--write` also writes the file, which
  `build_runner` must not use because it owns its outputs.

### The binding (`slint/`)

- `slint/lib/slint.dart` — the public API users see: `loadFile`/`loadSource`,
  `instantiateCompiled`, `ComponentInstance`, `SlintGlobal`, `runEventLoop`,
  `SlintTimer`, `initTranslations`, and the callback dispatch over
  `NativeCallable.isolateLocal`. It re-exports `SlintImage` from `image.dart`.
- `slint/lib/src/ffi.dart` — `SlintFfi` adds only how the library is found and
  the JSON-envelope helpers (`takeEnvelope`, `takeString`, `withNativeString`);
  everything else lives in the generated `ffi.g.dart`. These three are the only
  place that casts between `Pointer<Char>` and `package:ffi`'s `Pointer<Utf8>`.
- `slint/lib/src/diagnostics.dart` — `Diagnostic` and `SlintException`.
- `slint/lib/src/embedded.dart` — `SlintSurface` and the input enums, mirroring
  `native/rust/embedded.rs`.
- `slint/hook/build.dart` — the Dart build hook: every `flutter build`/`run`
  that depends on `slint` runs it, invokes `cargo build -p slint-dart` in
  `native/`, and bundles the `cdylib` into the application (as a framework on
  macOS). iOS builds nothing here; it uses the xcframework instead. Android
  cross-compiles each ABI with `cargo-ndk` against the Android NDK.

### The generator (`slint_generator/`)

- `slint_generator/lib/builder.dart` — the `slintBuilder` factory used by
  `build.yaml`.
- `slint_generator/lib/src/builder.dart` — `SlintBuilder`, the `build_runner`
  builder. Reads each `.slint` file, calls `package:slint/compiler.dart`'s
  `generate` (which runs `slint-dart-generate`), writes the `.slint.dart`
  wrapper, and registers compiler dependencies.
- `slint/lib/compiler.dart` locates that binary — `SLINT_DART_GENERATE`, then
  `native/target/`, then `PATH` — and builds it once from
  `native/codegen/Cargo.toml` when this is a source checkout. It never loads
  `libslint_dart`, so generating a wrapper does not require the runtime to be
  built.
- The builder's `buildExtensions` getter is dynamic: the default emits
  `.slint.dart` next to the source, while an `output_dir` option relocates
  outputs into a custom folder via a capture group. `build_to: source` only
  allows writing to `allowedOutputs`, which derives from the instance's
  `buildExtensions` (authoritative over `build.yaml`'s static value).
- `options` split: `include_paths` and `style` are passed to the native
  compiler; `output_dir` is a build_runner concern and is not.

### The widget (`slint_flutter/`)

- `slint_flutter/lib/slint_flutter.dart` — `SlintView`, a widget that drives a
  `SlintSurface` each frame and dispatches pointer/key input to it.

### Examples

- [`slint_flutter/example`](./slint_flutter/example) — the code-generation
  example, a Flutter application whose `CounterWindow` wrapper is generated
  from `lib/ui/counter.slint` by `build_runner`.
- [`examples/todo/flutter`](./examples/todo/flutter) — the todo demo, a Flutter
  application with a generated `MainWindow` wrapper. Its `android/`, `ios/`,
  `macos/` and `web/` platform runners are committed (standard Flutter
  convention); the wasm build (`web/slint_dart.js`, `web/slint_dart_bg.wasm`)
  is not.

## Key Patterns

- Everything must be used from the main isolate, where the Slint event loop
  lives. This matches the Python and Node.js bindings.
- `package:slint` is pure Dart; it depends only on `dart:ffi`, `ffi`, `path`,
  and the build-hook packages. It must not depend on Flutter. Flutter-only code
  goes in `slint_flutter`.
- Values cross the boundary as JSON: `num`, `String`, `bool`, `List`, and
  `Map<String, Object?>` for structs. Colors and brushes are CSS-style strings.
- FFI modules and generated files follow the existing conventions — match the
  surrounding code, and keep `ffi.g.dart` in sync with `native/rust/`.
- Generated code is excluded from the analyzer (via each package's
  `analysis_options.yaml` `analyzer.exclude`) and is not reformatted:
  `ffi.g.dart` comes from ffigen and the `.slint.dart` wrappers come from the
  `build_runner` generator. The `.slint.dart` files are gitignored, so they
  must never be edited by hand. Don't run `dart format` on generated files.
- Code style is enforced in CI: `dart format`/analyzer for Dart, `rustfmt` for
  Rust.

## Version Control (Git)

- Default branch is `main`; prefer linear history (rebase or squash).

## Deep Dive Documentation

- [`README.md`](./README.md) — the authoritative user-facing guide: building,
  testing, packaging (build hook, iOS xcframework, wasm), and the two ways to
  show a UI (native window vs. `SlintSurface`).
