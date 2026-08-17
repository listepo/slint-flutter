"""Rules that drive cargo for one Rust target triple.

Every rule here shells out to cargo, so none of them can run in the sandbox:
cargo needs the network for the crate registry, and `skia-bindings` needs the
host C++ toolchain. They are tagged accordingly. What Bazel provides is the fan
out — one action per target triple, run in parallel — and caching keyed on the
crate sources, so an unchanged slice is not rebuilt.
"""

# Tags shared by every action that calls cargo.
CARGO_TAGS = [
    "local",
    "no-sandbox",
    "requires-network",
]

def _cargo_build_command(manifest, triple, profile, package, artifact, output, feature_cmds):
    """Shell that runs one cargo build and copies the artifact Bazel asked for."""
    return """
set -euo pipefail
manifest="{manifest}"
triple="{triple}"
profile="{profile}"
package="{package}"
artifact="{artifact}"
output="{output}"

find_cargo() {{
  if command -v cargo >/dev/null 2>&1; then
    command -v cargo
    return
  fi
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    echo "$HOME/.cargo/bin/cargo"
    return
  fi
  echo "cargo not found. Install the Rust toolchain (https://rustup.rs) or" >&2
  echo 'run bazel with --action_env=PATH="$PATH".' >&2
  exit 1
}}

cargo="$(find_cargo)"
export PATH="$(dirname "$cargo"):$PATH"

if [[ "$triple" == *-linux-android ]]; then
  api="${{SLINT_DART_ANDROID_API:-21}}"
  ndk="${{ANDROID_NDK_HOME:-${{ANDROID_NDK_ROOT:-}}}}"
  if [ -z "$ndk" ]; then
    sdk="${{ANDROID_HOME:-${{ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}}}"
    if [ -d "$sdk/ndk" ]; then
      ndk="$(ls -1 "$sdk/ndk" | sort -V | tail -1)"
      ndk="$sdk/ndk/$ndk"
    fi
  fi
  if [ -z "$ndk" ] || [ ! -d "$ndk" ]; then
    echo "Android NDK not found. Install it, or set ANDROID_NDK_HOME." >&2
    exit 1
  fi
  host_tag="darwin-x86_64"
  if [ "$(uname -s)" != "Darwin" ]; then
    host_tag="linux-x86_64"
  fi
  bin_dir="$ndk/toolchains/llvm/prebuilt/$host_tag/bin"
  ndk_triple="$triple"
  if [ "$triple" = "armv7-linux-androideabi" ]; then
    ndk_triple="armv7a-linux-androideabi"
  fi
  clang="$bin_dir/${{ndk_triple}}${{api}}-clang"
  if [ ! -f "$clang" ]; then
    echo "No $clang; is the installed NDK older than 27?" >&2
    exit 1
  fi
  underscored="${{triple//-/_}}"
  upper="$(printf '%s' "$underscored" | tr '[:lower:]' '[:upper:]')"
  export "CARGO_TARGET_${{upper}}_LINKER=$clang"
  export "CC_${{underscored}}=$clang"
  export "AR_${{underscored}}=$bin_dir/llvm-ar"
  export "CXX_${{underscored}}=$bin_dir/${{ndk_triple}}${{api}}-clang++"
fi

args=(build --manifest-path "$manifest" --package "$package")
if [ "$profile" = "release" ]; then
  args+=(--release)
fi
if [ "$triple" != "host" ]; then
  args+=(--target "$triple")
fi
{feature_cmds}

echo "==> cargo ${{args[*]}}" >&2
"$cargo" "${{args[@]}}"

target_dir="$("$cargo" metadata --format-version 1 --no-deps --manifest-path "$manifest" \\
  | sed -n 's/.*"target_directory"[[:space:]]*:[[:space:]]*"\\([^"]*\\)".*/\\1/p' | head -1)"
if [ -z "$target_dir" ]; then
  echo "could not read target_directory from cargo metadata" >&2
  exit 1
fi

if [ "$triple" = "host" ]; then
  built="$target_dir/$profile/$artifact"
else
  built="$target_dir/$triple/$profile/$artifact"
fi

if [ ! -f "$built" ]; then
  echo "cargo did not produce $built" >&2
  exit 1
fi

mkdir -p "$(dirname "$output")"
cp "$built" "$output"
""".format(
        manifest = manifest,
        triple = triple,
        profile = profile,
        package = package,
        artifact = artifact,
        output = output,
        feature_cmds = feature_cmds,
    )

def _feature_shell_args(features):
    lines = []
    for arg in features:
        lines.append('args+=("%s")' % arg)
    return "\n".join(lines)

def _cargo_library_impl(ctx):
    output = ctx.actions.declare_file(
        "{}/lib{}.{}".format(ctx.attr.target_triple, ctx.attr.crate_name, ctx.attr.extension),
    )

    features = []
    if not ctx.attr.default_features:
        features.append("--no-default-features")
    if ctx.attr.crate_features:
        features += ["--features", ",".join(ctx.attr.crate_features)]

    ctx.actions.run_shell(
        outputs = [output],
        inputs = ctx.files.srcs,
        command = _cargo_build_command(
            manifest = ctx.file.manifest.path,
            triple = ctx.attr.target_triple,
            profile = ctx.attr.profile,
            package = ctx.attr.package,
            artifact = "lib{}.{}".format(ctx.attr.crate_name, ctx.attr.extension),
            output = output.path,
            feature_cmds = _feature_shell_args(features),
        ),
        mnemonic = "CargoBuild",
        progress_message = "Building %s for %s" % (ctx.attr.package, ctx.attr.target_triple),
        execution_requirements = {tag: "1" for tag in CARGO_TAGS},
        use_default_shell_env = True,
    )

    return [DefaultInfo(files = depset([output]))]

cargo_library = rule(
    implementation = _cargo_library_impl,
    doc = "Builds one cdylib for one Rust target triple.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "Everything that should invalidate the build.",
        ),
        "manifest": attr.label(allow_single_file = True, mandatory = True),
        "package": attr.string(mandatory = True),
        "crate_name": attr.string(mandatory = True),
        "target_triple": attr.string(mandatory = True),
        "profile": attr.string(default = "release"),
        "extension": attr.string(default = "dylib"),
        "default_features": attr.bool(default = True),
        "crate_features": attr.string_list(default = []),
    },
)

def _cargo_binary_impl(ctx):
    output = ctx.actions.declare_file(ctx.attr.binary_name)

    ctx.actions.run_shell(
        outputs = [output],
        inputs = ctx.files.srcs,
        command = _cargo_build_command(
            manifest = ctx.file.manifest.path,
            triple = "host",
            profile = ctx.attr.profile,
            package = ctx.attr.package,
            artifact = ctx.attr.binary_name,
            output = output.path,
            feature_cmds = "",
        ),
        mnemonic = "CargoBuildBinary",
        progress_message = "Building %s" % ctx.attr.binary_name,
        execution_requirements = {tag: "1" for tag in CARGO_TAGS},
        use_default_shell_env = True,
    )

    return [DefaultInfo(
        files = depset([output]),
        executable = output,
    )]

cargo_binary = rule(
    implementation = _cargo_binary_impl,
    doc = "Builds one host binary — the Slint→Dart generator.",
    executable = True,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "manifest": attr.label(allow_single_file = True, mandatory = True),
        "package": attr.string(mandatory = True),
        "binary_name": attr.string(mandatory = True),
        "profile": attr.string(default = "release"),
    },
)
