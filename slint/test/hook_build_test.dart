import 'dart:ffi' show Abi;
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/src/test.dart' as hooks_test;
import 'package:native_toolchain_rust/native_toolchain_rust.dart';
import 'package:test/test.dart';

import '../hook/build.dart' as build_hook;

/// The host OS in code-asset terms, or null when the hook doesn't support it.
OS? hostOs(Abi abi) {
  return switch (abi) {
    Abi.macosArm64 || Abi.macosX64 => OS.macOS,
    Abi.linuxArm64 || Abi.linuxX64 => OS.linux,
    Abi.windowsArm64 || Abi.windowsX64 => OS.windows,
    _ => null,
  };
}

void main() {
  test('the cargo profile defaults to release and honors the user-define', () {
    expect(build_hook.cargoProfile(null), BuildMode.release);
    expect(build_hook.cargoProfile('debug'), BuildMode.debug);
    expect(build_hook.cargoProfile('release'), BuildMode.release);
    expect(
      () => build_hook.cargoProfile('fast'),
      throwsA(isA<FormatException>()),
    );
  });

  test('mobile builds the software renderer only', () {
    // Android and iOS draw through SlintSurface, and winit/Skia/FemtoVG do not
    // cross-compile to them anyway.
    expect(build_hook.isEmbeddedOnly(OS.android), isTrue);
    expect(build_hook.isEmbeddedOnly(OS.iOS), isTrue);
    // Desktop keeps the default features, because `run()` opens a window.
    expect(build_hook.isEmbeddedOnly(OS.macOS), isFalse);
    expect(build_hook.isEmbeddedOnly(OS.linux), isFalse);
    expect(build_hook.isEmbeddedOnly(OS.windows), isFalse);
  });

  test('leaves iOS to the xcframework: no asset, no build', () async {
    await hooks_test.testBuildHook(
      mainMethod: build_hook.main,
      extensions: [
        CodeAssetExtension(
          targetOS: OS.iOS,
          targetArchitecture: Architecture.arm64,
          linkModePreference: LinkModePreference.dynamic,
          iOS: IOSCodeConfig(targetSdk: IOSSdk.iPhoneOS, targetVersion: 13),
        ),
      ],
      check: (input, output) {
        expect(
            output.assets.encodedAssets.where((a) => a.isCodeAsset), isEmpty);
        expect(output.dependencies, isEmpty);
      },
    );
  });

  // A real cargo build of the default feature set — Skia included — into the
  // throwaway target directory `testBuildHook` hands out, so nothing is cached
  // from the repository's own `native/target`. Minutes, not seconds.
  test('builds libslint_dart and declares it as a bundled code asset',
      timeout: const Timeout(Duration(minutes: 30)), () async {
    final abi = Abi.current();
    final os = hostOs(abi);
    if (os == null ||
        !File('${Platform.environment['HOME']}/.cargo/bin/rustup')
            .existsSync()) {
      markTestSkipped(
        'The host (${Platform.operatingSystem}) is not supported, or rustup '
        'is unavailable.',
      );
      return;
    }

    await hooks_test.testBuildHook(
      mainMethod: build_hook.main,
      extensions: [
        CodeAssetExtension(
          targetOS: os,
          targetArchitecture: Architecture.fromAbi(abi),
          linkModePreference: LinkModePreference.dynamic,
          macOS: os == OS.macOS ? MacOSCodeConfig(targetVersion: 13) : null,
        ),
      ],
      check: (input, output) {
        final codeAssets = output.assets.encodedAssets
            .where((asset) => asset.isCodeAsset)
            .map((asset) => asset.asCodeAsset)
            .toList();
        expect(codeAssets, hasLength(1));
        final asset = codeAssets.single;
        expect(asset.id, 'package:slint/${build_hook.assetName}');
        expect(asset.linkMode, isA<DynamicLoadingBundled>());
        expect(asset.file, isNotNull);
        expect(File.fromUri(asset.file!).existsSync(), isTrue);
        expect(asset.file!.pathSegments.last, contains('slint_dart'));

        // The Rust sources are declared, so editing one re-runs cargo. These
        // come from cargo's own depfile, which lists each file that went into
        // the build — a directory dependency would not do: the hook runner
        // hashes a directory from its child names alone.
        final dependencies = output.dependencies.map((uri) => uri.path).toSet();
        expect(dependencies, isNotEmpty);
        expect(
          dependencies.where((path) => path.endsWith('rust/lib.rs')),
          isNotEmpty,
          reason: 'the crate sources must be watched',
        );
      },
    );
  });
}
