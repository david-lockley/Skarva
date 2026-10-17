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

app_dir="target/$profile/JEV Studio.app"
mkdir -p "$app_dir/Contents/MacOS"
mkdir -p "$app_dir/Contents/Resources/licenses"
cp "target/$profile/jev-studio" "$app_dir/Contents/MacOS/jev-studio"
cp packaging/macos/Info.plist "$app_dir/Contents/Info.plist"
cp THIRD_PARTY_NOTICES.md "$app_dir/Contents/Resources/THIRD_PARTY_NOTICES.md"
cp licenses/Apache-2.0.txt "$app_dir/Contents/Resources/licenses/Apache-2.0.txt"
echo "$app_dir"
