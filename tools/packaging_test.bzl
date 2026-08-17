"""Unit tests for packaging helpers."""

load("@bazel_skylib//lib:unittest.bzl", "asserts", "unittest")
load(":packaging_lib.bzl", "ANDROID_MANIFEST", "build_framework_shell", "framework_plist")

def _contains(env, haystack, needle):
    asserts.true(
        env,
        needle in haystack,
        "expected %r in output" % needle,
    )

def _framework_plist_test_impl(ctx):
    env = unittest.begin(ctx)
    plist = framework_plist("MacOSX", "11.0", "1.2.3")
    _contains(env, plist, "<string>MacOSX</string>")
    _contains(env, plist, "<string>1.2.3</string>")
    _contains(env, plist, "<string>slint_dart</string>")
    return unittest.end(env)

framework_plist_test = unittest.make(_framework_plist_test_impl)

def _android_manifest_test_impl(ctx):
    env = unittest.begin(ctx)
    _contains(env, ANDROID_MANIFEST, 'package="dev.slint.slintdart"')
    _contains(env, ANDROID_MANIFEST, 'android:minSdkVersion="21"')
    return unittest.end(env)

android_manifest_test = unittest.make(_android_manifest_test_impl)

def _build_framework_shell_test_impl(ctx):
    env = unittest.begin(ctx)
    macos = build_framework_shell(
        slice_name = "macos",
        inputs = ["/tmp/a.dylib", "/tmp/b.dylib"],
        version = "9.9.9",
        stage = "$stage",
    )
    _contains(env, macos, "Versions/A/slint_dart")
    _contains(env, macos, "lipo -create")

    ios = build_framework_shell(
        slice_name = "ios",
        inputs = ["/tmp/a.dylib"],
        version = "9.9.9",
        stage = "$stage",
    )
    _contains(env, ios, "iPhoneOS")
    _contains(env, ios, 'framework="$stage/ios/slint_dart.framework"')
    return unittest.end(env)

build_framework_shell_test = unittest.make(_build_framework_shell_test_impl)
