"""Developer-facing commands exposed as `bazel run` targets."""

def _dev_script_impl(ctx):
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(
        output = launcher,
        is_executable = True,
        content = ctx.attr.script,
    )
    return [DefaultInfo(
        executable = launcher,
        runfiles = ctx.runfiles(files = ctx.files.data),
    )]

dev_script = rule(
    implementation = _dev_script_impl,
    doc = "Runs a shell script from the repository root via `bazel run`.",
    executable = True,
    attrs = {
        "script": attr.string(mandatory = True),
        "data": attr.label_list(allow_files = True, default = []),
    },
)

_GENERATE_BINDINGS_SCRIPT = """\
#!/usr/bin/env bash
set -euo pipefail

check=false
for arg in "$@"; do
  if [ "$arg" = "--check" ]; then
    check=true
  fi
done

find_cbindgen() {
  if command -v cbindgen >/dev/null 2>&1; then
    command -v cbindgen
    return
  fi
  if [ -x "$HOME/.cargo/bin/cbindgen" ]; then
    echo "$HOME/.cargo/bin/cbindgen"
    return
  fi
  echo "cbindgen not found. Install it with: cargo install cbindgen" >&2
  exit 1
}

root="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "$0")/../.." && pwd)}"
package="$root/slint"
native="$root/native"
target_dir="${CARGO_TARGET_DIR:-$native/target}"
header="$target_dir/slint_dart.h"
generated="$package/lib/src/ffi.g.dart"

cbindgen="$(find_cbindgen)"
mkdir -p "$target_dir"
"$cbindgen" --config "$native/cbindgen.toml" --crate slint-dart \\
  --output "$header" --quiet "$native"

backup=""
if [ "$check" = true ]; then
  backup="$(mktemp)"
  cp "$generated" "$backup"
fi

cleanup() {
  if [ -n "$backup" ]; then
    cp "$backup" "$generated"
    rm -f "$backup"
  fi
}
trap cleanup EXIT

(
  cd "$package"
  dart run ffigen --config ffigen.yaml >/dev/null
)

if [ "$check" = true ]; then
  if ! diff -q "$backup" "$generated" >/dev/null; then
    diff -u "$backup" "$generated" >&2 || true
    echo >&2
    echo "ffi.g.dart is out of date with rust/." >&2
    echo "Regenerate it with: bazel run //scripts:generate_bindings" >&2
    exit 1
  fi
  echo "ffi.g.dart is up to date."
else
  echo "$generated"
fi
"""

_BUILD_WASM_SCRIPT = """\
#!/usr/bin/env bash
set -euo pipefail

destination=""
for arg in "$@"; do
  if [ -n "$destination" ]; then
  echo "unexpected argument: $arg" >&2
    exit 2
  fi
  destination="$arg"
done

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "wasm-pack is required: cargo install wasm-pack" >&2
  exit 1
fi

root="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "$0")/../.." && pwd)}"
native="$root/native"
target_dir="${CARGO_TARGET_DIR:-$native/target}"
stage="$target_dir/wasm-web"

features="${SLINT_DART_FEATURES:---no-default-features --features renderer-software}"
# shellcheck disable=SC2206
feature_args=($features)

if ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "adding the wasm32-unknown-unknown target" >&2
  rustup target add wasm32-unknown-unknown
fi

wasm-pack build "$native" --target web --out-dir "$stage" --out-name slint_dart \\
  --release --no-typescript --no-pack -- "${feature_args[@]}"

for name in .gitignore package.json README.md; do
  rm -f "$stage/$name"
done

if [ -n "$destination" ]; then
  mkdir -p "$destination"
  cp "$stage/slint_dart.js" "$stage/slint_dart_bg.wasm" "$destination/"
  echo "copied slint_dart.js and slint_dart_bg.wasm to $destination" >&2
else
  echo "built $stage/slint_dart.js and $stage/slint_dart_bg.wasm" >&2
fi
"""

def generate_bindings(name):
    dev_script(name = name, script = _GENERATE_BINDINGS_SCRIPT)

def build_wasm(name):
    dev_script(name = name, script = _BUILD_WASM_SCRIPT)
