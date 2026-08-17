# Dart Code-Generation Example

Build the native Slint library from the repository root:

```sh
cargo build --release -p slint-dart
```

Fetch the Dart dependencies and generate the typed wrapper:

```sh
cd slint/example
fvm dart pub get
fvm dart run build_runner build --delete-conflicting-outputs
```

The generator creates `lib/ui/counter.slint.dart`. `CounterWindow.load()`
instantiates the component compiled into that wrapper; it does not read
`counter.slint` at runtime.
Run the example on Linux or Windows:

```sh
fvm dart run bin/main.dart
```

Use watch mode while editing `lib/ui/counter.slint`:

```sh
fvm dart run build_runner watch --delete-conflicting-outputs
```

The generated `CounterWindow` wrapper exposes `current-count` as `currentCount`,
`status_message` as `statusMessage`, `count-changed` as `onCountChanged()`,
and `reset_counter` as `invokeResetCounter()`.
