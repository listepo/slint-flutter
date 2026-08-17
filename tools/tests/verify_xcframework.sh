#!/usr/bin/env bash
set -euo pipefail

archive="$1"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

unzip -q "$archive" -d "$work"
test -d "$work/SlintDart.xcframework"
test -f "$work/SlintDart.xcframework/Info.plist"
