#!/bin/sh
set -eu

profile="${1:-release}"
package_id="$(cargo pkgid --offline)"
version="${package_id##*@}"
numeric_version="${version%%-*}"
numeric_version="${numeric_version%%+*}"
if [ "$profile" = "release" ]; then
    cargo build --release --locked
elif [ "$profile" = "debug" ]; then
    cargo build --locked
else
    echo "Usage: scripts/package-macos.sh [release|debug]" >&2
    exit 2
fi

app_dir="${2:-target/$profile/Skarva.app}"
mkdir -p "$app_dir/Contents/MacOS"
mkdir -p "$app_dir/Contents/Resources/licenses"
cp "target/$profile/skarva" "$app_dir/Contents/MacOS/skarva"
cp packaging/macos/Info.plist "$app_dir/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $numeric_version" "$app_dir/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $numeric_version" "$app_dir/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :SkarvaVersion $version" "$app_dir/Contents/Info.plist"
cp packaging/macos/AppIcon.icns "$app_dir/Contents/Resources/AppIcon.icns"
cp LICENSE "$app_dir/Contents/Resources/LICENSE"
cp THIRD_PARTY_NOTICES.md "$app_dir/Contents/Resources/THIRD_PARTY_NOTICES.md"
cp -R licenses/. "$app_dir/Contents/Resources/licenses/"
codesign --force --sign - "$app_dir"
echo "$app_dir"
