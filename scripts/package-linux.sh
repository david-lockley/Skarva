#!/bin/sh
set -eu

if [ "$(uname -s)" != "Linux" ]; then
    echo "Run this script on Linux." >&2
    exit 2
fi
cargo build --release --locked
architecture="$(uname -m)"
case "$architecture" in
    x86_64) architecture=x64; deb_architecture=amd64 ;;
    aarch64) architecture=arm64; deb_architecture=arm64 ;;
    *) echo "Unsupported Linux architecture: $architecture" >&2; exit 2 ;;
esac
package_id="$(cargo pkgid --offline)"
version="${package_id##*@}"
package="skarva-$version-linux-$architecture"
stage="target/release/$package"
mkdir -p "$stage/licenses" "$stage/share/applications" "$stage/share/icons/hicolor/256x256/apps"
install -m 755 target/release/skarva "$stage/skarva"
cp LICENSE README.md THIRD_PARTY_NOTICES.md "$stage/"
cp -R licenses/. "$stage/licenses/"
cp packaging/linux/dev.skarva.app.desktop "$stage/share/applications/"
cp assets/app-icon-window.png "$stage/share/icons/hicolor/256x256/apps/dev.skarva.app.png"
tar -czf "target/release/$package.tar.gz" -C target/release "$package"
echo "target/release/$package.tar.gz"

if command -v dpkg-deb >/dev/null 2>&1; then
    deb_root="target/release/deb-$version-$deb_architecture"
    mkdir -p "$deb_root/DEBIAN" "$deb_root/usr/bin" "$deb_root/usr/share/doc/skarva/licenses" \
        "$deb_root/usr/share/applications" "$deb_root/usr/share/icons/hicolor/256x256/apps"
    install -m 755 target/release/skarva "$deb_root/usr/bin/skarva"
    cp packaging/linux/dev.skarva.app.desktop "$deb_root/usr/share/applications/"
    cp assets/app-icon-window.png "$deb_root/usr/share/icons/hicolor/256x256/apps/dev.skarva.app.png"
    cp LICENSE README.md THIRD_PARTY_NOTICES.md "$deb_root/usr/share/doc/skarva/"
    cp -R licenses/. "$deb_root/usr/share/doc/skarva/licenses/"
    debian_version="$(printf '%s' "$version" | sed 's/-/~/')"
    sed -e "s/@VERSION@/$debian_version/" -e "s/@ARCHITECTURE@/$deb_architecture/" \
        packaging/linux/control.in > "$deb_root/DEBIAN/control"
    dpkg-deb --root-owner-group --build "$deb_root" "target/release/$package.deb"
else
    echo "Install dpkg-deb to also create a Debian/Ubuntu package." >&2
fi
