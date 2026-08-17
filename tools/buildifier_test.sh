#!/usr/bin/env bash
set -eo pipefail

if ! command -v buildifier >/dev/null 2>&1; then
  echo "buildifier not found on PATH; install it with mise or https://github.com/bazelbuild/buildtools" >&2
  exit 1
fi

root="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

# shellcheck disable=SC2046
buildifier -mode=check $(find . \
  \( -name '*.bzl' -o -name 'BUILD.bazel' -o -name 'MODULE.bazel' \) \
  -not -path './bazel-*/*')
