/// The compiler behind the code generator.
///
/// `package:slint_generator` calls this from its `build_runner` builder;
/// applications use the generated wrappers instead.
///
/// The compiler is not in the runtime library. It lives in the
/// `slint-dart-generate` binary from the `slint-dart-codegen` crate, so
/// `libslint_dart` ships without it and generation never needs the library to
/// be built first. The binary writes one `.slint.dart` per input and nothing
/// else — no Rust, no C, nothing an application has to compile.
library;

import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;

/// Compile the `.slint` file at [inputPath] into Dart source for [outputPath],
/// configured by [optionsJson].
///
/// Returns the compiler's `source`, its `dependencies`, and any `diagnostics`
/// or generation `error`, all as plain JSON values.
Map<String, Object?> generate(
  String inputPath,
  String outputPath,
  String optionsJson,
) {
  final result = Process.runSync(
    findGenerator(),
    [inputPath, outputPath, optionsJson],
  );
  final output = (result.stdout as String).trim();
  if (output.isEmpty) {
    throw StateError(
      'slint-dart-generate produced no output '
      '(exit code ${result.exitCode}).\n${result.stderr}',
    );
  }
  final decoded = jsonDecode(output);
  if (decoded is! Map<String, dynamic>) {
    throw StateError('slint-dart-generate returned an invalid response.');
  }
  return decoded.cast<String, Object?>();
}

/// The name of the generator binary on this platform.
String get generatorFileName =>
    Platform.isWindows ? 'slint-dart-generate.exe' : 'slint-dart-generate';

/// Locate `slint-dart-generate`, building it once if this is a source
/// checkout.
///
/// `SLINT_DART_GENERATE` wins when set. Otherwise this looks through the Cargo
/// output directories above the current directory, the running script, and the
/// `slint` package root — the same places [SlintFfi] looks for the library —
/// and falls back to `PATH`.
String findGenerator() {
  final explicit = Platform.environment['SLINT_DART_GENERATE'];
  if (explicit != null && explicit.isNotEmpty) {
    return p.normalize(p.absolute(explicit));
  }
  final found = _findInCargoTarget() ?? _findOnPath();
  if (found != null) return found;

  final manifest = _codegenManifest();
  if (manifest == null) {
    throw StateError(
      'Cannot find $generatorFileName. Build it with\n'
      '    cd native && cargo build --release -p slint-dart-codegen\n'
      'or point SLINT_DART_GENERATE at the binary.',
    );
  }
  stderr.writeln('Building $generatorFileName (first run only)...');
  final build = Process.runSync(
    'cargo',
    [
      'build',
      '--release',
      '--manifest-path',
      manifest,
      '--bin',
      'slint-dart-generate'
    ],
  );
  if (build.exitCode != 0) {
    throw StateError(
      'cargo build -p slint-dart-codegen failed with exit code '
      '${build.exitCode}:\n${build.stderr}',
    );
  }
  final built = _findInCargoTarget();
  if (built == null) {
    throw StateError(
      'cargo built slint-dart-codegen but $generatorFileName was not found '
      'under target/. Point SLINT_DART_GENERATE at it.',
    );
  }
  return built;
}

/// `native/codegen/Cargo.toml` next to the crate that owns this package, or null when
/// this is not a source checkout (a published copy has no Rust sources).
String? _codegenManifest() {
  for (final root in _searchRoots()) {
    for (var dir = Directory(root);; dir = dir.parent) {
      for (final candidate in [
        File(p.join(dir.path, 'native', 'codegen', 'Cargo.toml')),
        File(p.join(dir.path, 'codegen', 'Cargo.toml')),
      ]) {
        if (candidate.existsSync()) return candidate.path;
      }
      if (dir.parent.path == dir.path) break;
    }
  }
  return null;
}

String? _findInCargoTarget() {
  for (final root in _searchRoots()) {
    for (var dir = Directory(root);; dir = dir.parent) {
      for (final targetDir in _cargoTargetDirs(dir.path)) {
        for (final profile in const ['release', 'debug']) {
          final candidate =
              File(p.join(targetDir, profile, generatorFileName));
          if (candidate.existsSync()) return candidate.path;
        }
      }
      if (dir.parent.path == dir.path) break;
    }
  }
  return null;
}

Iterable<String> _cargoTargetDirs(String root) {
  return [
    p.join(root, 'target'),
    p.join(root, 'native', 'target'),
  ];
}

String? _findOnPath() {
  final result = Process.runSync(
    Platform.isWindows ? 'where' : 'which',
    [generatorFileName],
  );
  if (result.exitCode != 0) return null;
  final first = (result.stdout as String).split('\n').first.trim();
  return first.isEmpty ? null : first;
}

/// Where to start walking up from: the working directory, the running script,
/// and the root of the linked `slint` package. The last one is what makes this
/// work from an application's `build_runner` run, whose working directory is
/// the application, not this checkout.
Iterable<String> _searchRoots() {
  return <String>{
    Directory.current.path,
    if (Platform.script.scheme == 'file')
      File.fromUri(Platform.script).parent.path,
    ..._linkedPackageRoots(),
  };
}

Iterable<String> _linkedPackageRoots() {
  final config = Platform.packageConfig;
  if (config == null) return const [];
  try {
    final configFile = File.fromUri(Uri.parse(config));
    final packages = (jsonDecode(configFile.readAsStringSync())
        as Map<String, dynamic>)['packages'] as List<Object?>;
    return [
      for (final entry in packages.whereType<Map<String, dynamic>>())
        if (entry['name'] == 'slint') _packageRoot(entry, configFile),
    ].whereType<String>();
  } on Object {
    return const [];
  }
}

String? _packageRoot(Map<String, dynamic> entry, File configFile) {
  final rootUri = entry['rootUri'];
  if (rootUri is! String) return null;
  final uri = Uri.tryParse(rootUri);
  if (uri == null) return null;
  final resolved = uri.isAbsolute ? uri : configFile.parent.uri.resolveUri(uri);
  if (resolved.scheme != 'file') return null;
  return File.fromUri(resolved).path;
}
