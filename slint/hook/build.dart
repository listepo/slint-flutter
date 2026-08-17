// cSpell: ignore rustup

/// The Dart build hook: every `flutter build`/`run` and `dart run`/`test` that
/// depends on `slint` runs it, and it produces `libslint_dart` as a code asset.
///
/// The cargo plumbing — target triples, the Android NDK toolchain, which files
/// to watch — belongs to `native_toolchain_rust`, which reads cargo's own
/// depfile so the hook re-runs exactly when the Rust sources change.
library;

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';
import 'package:native_toolchain_rust/native_toolchain_rust.dart';

/// The name of the code asset: the cdylib built by the `slint-dart` crate.
const assetName = 'libslint_dart';

/// The crate directory, relative to the `slint` package root.
const cratePath = '../native';

/// The cargo profile to build: `release` by default, or the value of the
/// `cargo_profile` user-define. Debug builds are faster to produce; release is
/// what the README documents.
BuildMode cargoProfile(Object? value) {
  return switch (value) {
    null || 'release' => BuildMode.release,
    'debug' => BuildMode.debug,
    _ => throw const FormatException(
        "hooks.user_defines.slint.cargo_profile must be 'debug' or 'release'",
      ),
  };
}

/// Whether [os] draws through the embedded `SlintSurface` only.
///
/// Mobile never opens a Slint-owned window, so winit, Skia and FemtoVG would be
/// dead weight there — and they do not cross-compile to these targets anyway.
/// The desktop default feature set keeps them, because `run()` needs a real
/// window. This mirrors what the xcframework and AAR scripts build.
bool isEmbeddedOnly(OS os) => os == OS.android || os == OS.iOS;

void main(List<String> arguments) async {
  await build(arguments, (input, output) async {
    if (!input.config.buildCodeAssets) return;

    final code = input.config.code;

    // iOS arrives as SlintDart.xcframework instead of being built here: an
    // embedded framework is what an iOS application can load, and it is built
    // once with `scripts/build_apple_frameworks.dart` and embedded in the
    // Runner target. `package:slint` opens it from the app bundle at runtime.
    if (code.targetOS == OS.iOS) return;

    if (code.linkModePreference == LinkModePreference.static) {
      throw UnsupportedError(
        'Slint only ships a dynamic library; static linking is not supported.',
      );
    }

    final embeddedOnly = isEmbeddedOnly(code.targetOS);
    await RustBuilder(
      assetName: assetName,
      cratePath: cratePath,
      buildMode: cargoProfile(input.userDefines['cargo_profile']),
      enableDefaultFeatures: !embeddedOnly,
      features: embeddedOnly ? const ['renderer-software'] : const [],
    ).run(input: input, output: output);
  });
}
