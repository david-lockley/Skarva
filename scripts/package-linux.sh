#!/bin/sh
set -eu

if [ "$(uname -s)" != "Linux" ]; then
    echo "Run this script on Linux." >&2
    exit 2
fi
cargo build --release --locked
architecture="$(uname -m)"
package="skarva-linux-$architecture"
stage="target/release/$package"
mkdir -p "$stage/licenses" "$stage/share/applications" "$stage/share/icons/hicolor/256x256/apps"
install -m 755 target/release/skarva "$stage/skarva"
cp LICENSE README.md THIRD_PARTY_NOTICES.md "$stage/"
cp -R licenses/. "$stage/licenses/"
cp packaging/linux/dev.skarva.app.desktop "$stage/share/applications/"
cp assets/app-icon-window.png "$stage/share/icons/hicolor/256x256/apps/dev.skarva.app.png"
tar -czf "target/release/$package.tar.gz" -C target/release "$package"
echo "target/release/$package.tar.gz"
