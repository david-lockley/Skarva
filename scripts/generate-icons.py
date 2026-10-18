#!/usr/bin/env python3
"""Generate desktop icon assets from the original Figma PNG exports.

Requires Pillow. Run from any directory with: python3 scripts/generate-icons.py
Generated assets are checked in; normal Rust builds do not need Python.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"


def rounded(image: Image.Image, radius: int) -> Image.Image:
    mask = Image.new("L", image.size)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, image.width - 1, image.height - 1), radius=radius, fill=255
    )
    result = image.copy()
    result.putalpha(mask)
    return result


def main() -> None:
    master = Image.open(ASSETS / "app-icon.png").convert("RGBA")
    if master.size != (1024, 1024):
        raise ValueError("app-icon.png must be the 1024 × 1024 square master")

    # A legacy macOS .icns needs its own silhouette and transparent margins.
    # The source export stays untouched so it can still be edited in Figma.
    tile = rounded(master.resize((832, 832), Image.Resampling.LANCZOS), 186)
    shadow = Image.new("RGBA", (1024, 1024))
    shadow_mask = Image.new("L", (1024, 1024))
    shadow_mask.paste(tile.getchannel("A"), (96, 108))
    shadow_mask = shadow_mask.filter(ImageFilter.GaussianBlur(18))
    shadow.putalpha(shadow_mask.point(lambda alpha: alpha * 0.3))
    shadow.alpha_composite(tile, (96, 96))
    shadow.save(ASSETS / "app-icon-macos.png")

    windows = rounded(master, 220)
    windows.resize((256, 256), Image.Resampling.LANCZOS).save(
        ASSETS / "app-icon-window.png"
    )
    windows_directory = ROOT / "packaging" / "windows"
    windows_directory.mkdir(parents=True, exist_ok=True)
    windows.save(
        windows_directory / "app-icon.ico",
        sizes=[(n, n) for n in (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)],
    )

    mark = Image.open(ASSETS / "logo-mark.png").convert("RGBA")
    bounds = mark.getchannel("A").getbbox()
    if bounds is None:
        raise ValueError("logo-mark.png has no visible artwork")
    mark = mark.crop(bounds)
    mark.thumbnail((192, 192), Image.Resampling.LANCZOS)
    mark.save(ASSETS / "logo-mark-ui.png")

    shadow.save(ROOT / "packaging" / "macos" / "AppIcon.icns", format="ICNS")
    print("Desktop icons and transparent in-app mark generated.")


if __name__ == "__main__":
    main()
