#!/usr/bin/env bash
set -euo pipefail

aar="$1"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

unzip -q "$aar" -d "$work"

test -f "$work/AndroidManifest.xml"
test -f "$work/classes.jar"
test -f "$work/jni/arm64-v8a/libslint_dart.so"
test -f "$work/jni/x86_64/libslint_dart.so"
test -f "$work/slint-dart.properties"
grep -q 'version=0.0.0-test' "$work/slint-dart.properties"
grep -q 'package="dev.slint.slintdart"' "$work/AndroidManifest.xml"
