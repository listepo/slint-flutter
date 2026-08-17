"""Rules that assemble mobile release artifacts from per-triple cargo outputs."""

load(":packaging_lib.bzl", "ANDROID_MANIFEST", "build_framework_shell")

PACKAGING_TAGS = [
    "local",
    "no-sandbox",
]

def _android_aar_impl(ctx):
    output = ctx.outputs.out
    libs = []
    for abi, lib in ctx.attr.libs.items():
        libs.append((abi, lib.files.to_list()[0]))

    copy_cmds = []
    for abi, lib in libs:
        copy_cmds.append(
            'mkdir -p "$stage/jni/{abi}" && cp "{src}" "$stage/jni/{abi}/libslint_dart.so"'.format(
                abi = abi,
                src = lib.path,
            ),
        )

    ctx.actions.run_shell(
        outputs = [output],
        inputs = [lib for _, lib in libs],
        command = """
set -euo pipefail
root="$(pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

cat > "$stage/AndroidManifest.xml" << 'EOF'
{manifest}EOF

mkdir -p "$stage/META-INF"
echo "Manifest-Version: 1.0" > "$stage/META-INF/MANIFEST.MF"
( cd "$stage" && zip -q classes.jar META-INF/MANIFEST.MF )

{copy_cmds}

echo "version={version}" > "$stage/slint-dart.properties"

mkdir -p "$(dirname "$root/{output}")"
( cd "$stage" && zip -r -q "$root/{output}" . )
echo "wrote {output}" >&2
""".format(
            manifest = ANDROID_MANIFEST,
            copy_cmds = "\n".join(copy_cmds),
            version = ctx.attr.version,
            output = output.path,
        ),
        mnemonic = "AndroidAar",
        progress_message = "Assembling %s" % output.short_path,
        execution_requirements = {tag: "1" for tag in PACKAGING_TAGS},
    )

    return [DefaultInfo(files = depset([output]))]

android_aar = rule(
    implementation = _android_aar_impl,
    doc = "Assembles an AAR from per-ABI shared libraries.",
    attrs = {
        "version": attr.string(mandatory = True),
        "libs": attr.string_keyed_label_dict(
            allow_files = True,
            doc = "ABI name to `.so` file, e.g. `arm64-v8a` → `:android_arm64`.",
        ),
        "out": attr.output(mandatory = True),
    },
)

def _apple_xcframework_impl(ctx):
    output = ctx.outputs.out
    inputs = []
    framework_cmds = []
    for slice_name, libs in ctx.attr.slices.items():
        lib_files = [lib.files.to_list()[0] for lib in libs]
        inputs.extend(lib_files)
        framework_cmds.append(build_framework_shell(
            slice_name = slice_name,
            inputs = [f.path for f in lib_files],
            version = ctx.attr.version,
            stage = "$stage",
        ))

    ctx.actions.run_shell(
        outputs = [output],
        inputs = inputs,
        command = """
set -euo pipefail
root="$(pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
frameworks=()

{framework_cmds}

bundle="$stage/SlintDart.xcframework"
xcodebuild -create-xcframework "${{frameworks[@]}}" -output "$bundle"

mkdir -p "$(dirname "$root/{output}")"
( cd "$stage" && zip -r -q "$root/{output}" SlintDart.xcframework )
echo "wrote {output}" >&2
""".format(
            framework_cmds = "\n".join(framework_cmds),
            output = output.path,
        ),
        mnemonic = "AppleXcframework",
        progress_message = "Assembling %s" % output.short_path,
        execution_requirements = {tag: "1" for tag in PACKAGING_TAGS},
        use_default_shell_env = True,
    )

    return [DefaultInfo(files = depset([output]))]

apple_xcframework = rule(
    implementation = _apple_xcframework_impl,
    doc = "Assembles SlintDart.xcframework.zip from per-triple dylibs.",
    attrs = {
        "version": attr.string(mandatory = True),
        "slices": attr.label_list_dict(
            allow_files = True,
            doc = "Slice name to dylib targets, e.g. `macos` → [`:macos_arm64`, `:macos_x64`].",
        ),
        "out": attr.output(mandatory = True),
    },
)
