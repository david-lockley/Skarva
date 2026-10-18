#!/bin/sh
set -eu

profile="${1:-release}"
if [ "$profile" = "release" ]; then
    cargo build --release
elif [ "$profile" = "debug" ]; then
    cargo build
else
    echo "Usage: scripts/package-macos.sh [release|debug]" >&2
    exit 2
fi

app_dir="target/$profile/Skarva.app"
mkdir -p "$app_dir/Contents/MacOS"
mkdir -p "$app_dir/Contents/Resources/licenses"
cp "target/$profile/skarva" "$app_dir/Contents/MacOS/skarva"
cp packaging/macos/Info.plist "$app_dir/Contents/Info.plist"
cp packaging/macos/AppIcon.icns "$app_dir/Contents/Resources/AppIcon.icns"
cp THIRD_PARTY_NOTICES.md "$app_dir/Contents/Resources/THIRD_PARTY_NOTICES.md"
cp licenses/Apache-2.0.txt "$app_dir/Contents/Resources/licenses/Apache-2.0.txt"
codesign --force --sign - "$app_dir"
echo "$app_dir"
