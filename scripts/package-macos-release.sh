#!/bin/sh
set -eu

package_id="$(cargo pkgid --offline)"
version="${package_id##*@}"
case "$(uname -m)" in
    arm64) architecture=arm64 ;;
    x86_64) architecture=x64 ;;
    *) echo "Unsupported macOS architecture" >&2; exit 2 ;;
esac
name="skarva-$version-macos-$architecture"
app_dir="target/release/$name/Skarva.app"
sh scripts/package-macos.sh release "$app_dir"
codesign --verify --deep --strict "$app_dir"
ditto -c -k --sequesterRsrc --keepParent "$app_dir" "target/release/$name.zip"
dmg_stage="$(mktemp -d "${TMPDIR:-/tmp}/skarva-dmg.XXXXXX")"
ditto "$app_dir" "$dmg_stage/Skarva.app"
ln -s /Applications "$dmg_stage/Applications"
hdiutil create -volname Skarva -srcfolder "$dmg_stage" -format UDZO -ov "target/release/$name.dmg"
echo "target/release/$name.zip"
echo "target/release/$name.dmg"
