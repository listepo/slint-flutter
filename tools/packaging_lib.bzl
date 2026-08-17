"""Pure helpers shared by packaging rules and their unit tests."""

ANDROID_MANIFEST = """\
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="dev.slint.slintdart">
    <uses-sdk android:minSdkVersion="21" />
</manifest>
"""

def framework_plist(platform, min_os, version):
    """Return an Info.plist body for one framework slice."""
    return """\
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
\t<key>CFBundleDevelopmentRegion</key><string>en</string>
\t<key>CFBundleExecutable</key><string>slint_dart</string>
\t<key>CFBundleIdentifier</key><string>dev.slint.slintdart</string>
\t<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
\t<key>CFBundleName</key><string>slint_dart</string>
\t<key>CFBundlePackageType</key><string>FMWK</string>
\t<key>CFBundleShortVersionString</key><string>{version}</string>
\t<key>CFBundleVersion</key><string>{version}</string>
\t<key>CFBundleSupportedPlatforms</key><array><string>{platform}</string></array>
\t<key>MinimumOSVersion</key><string>{min_os}</string>
</dict>
</plist>
""".format(platform = platform, min_os = min_os, version = version)

_SLICE_PLATFORMS = {
    "macos": "MacOSX",
    "ios": "iPhoneOS",
    "ios-simulator": "iPhoneSimulator",
}

_SLICE_MIN_OS = {
    "macos": "${MACOSX_DEPLOYMENT_TARGET:-11.0}",
    "ios": "${IPHONEOS_DEPLOYMENT_TARGET:-13.0}",
    "ios-simulator": "${IPHONEOS_DEPLOYMENT_TARGET:-13.0}",
}

def build_framework_shell(slice_name, inputs, version, stage):
    """Shell fragment that builds one framework slice under $stage."""
    quoted = []
    for path in inputs:
        quoted.append('"%s"' % path)
    input_paths = " ".join(quoted)
    plist = framework_plist(
        platform = _SLICE_PLATFORMS[slice_name],
        min_os = _SLICE_MIN_OS[slice_name],
        version = version,
    )

    if slice_name == "macos":
        return """
framework="$stage/{slice}/slint_dart.framework"
mkdir -p "$framework/Versions/A/Resources"
binary="$framework/Versions/A/slint_dart"
lipo -create -output "$binary" {inputs}
install_name_tool -id "@rpath/slint_dart.framework/Versions/A/slint_dart" "$binary"
cat > "$framework/Versions/A/Resources/Info.plist" << 'EOF'
{plist}EOF
ln -sf A "$framework/Versions/Current"
ln -sf "Versions/Current/slint_dart" "$framework/slint_dart"
ln -sf "Versions/Current/Resources" "$framework/Resources"
frameworks+=( -framework "$framework" )
""".format(slice = slice_name, inputs = input_paths, plist = plist)

    return """
framework="$stage/{slice}/slint_dart.framework"
mkdir -p "$framework"
binary="$framework/slint_dart"
lipo -create -output "$binary" {inputs}
install_name_tool -id "@rpath/slint_dart.framework/slint_dart" "$binary"
cat > "$framework/Info.plist" << 'EOF'
{plist}EOF
frameworks+=( -framework "$framework" )
""".format(slice = slice_name, inputs = input_paths, plist = plist)
