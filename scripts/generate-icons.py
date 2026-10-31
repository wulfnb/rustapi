#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=10"]
# ///
"""Generate the RustAPI icon suite for macOS, Windows, Linux, Docker and the web.

Run with `uv run scripts/generate-icons.py`. On macOS, `iconutil` also builds
the `.icns`; elsewhere the existing `.icns` is left untouched.

The master artwork is a flattened render: the rounded tile sits on a baked-in
dark background with no alpha channel. Every output therefore cuts the tile
out with a superellipse mask first. Without that cut, the background shows up
as a dark frame in the Dock, Spotlight, the taskbar and launchers.
"""
from pathlib import Path
import io
import math
import shutil
import struct
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
ICONS_DIR = ROOT / "assets" / "icons"
SITE_DIR = ROOT / "site"
SOURCE = ICONS_DIR / "rustapi-icon-source.png"

# Outer edge of the tile's metal bevel in the 1024 px source, and the
# superellipse exponent that matches its continuous-curvature corners.
TILE_BOX = (185, 185, 838, 838)
TILE_EXPONENT = 5.0
# Inset that drops the bevel so tiny sizes show a bigger glyph.
SMALL_INSET = 34
SMALL_SIZES = {16, 24, 32}

# Apple's Big Sur icon grid: an 824 px tile centred on a 1024 px canvas.
MACOS_TILE_RATIO = 824 / 1024
# Windows and Linux do not mask icons, so the tile nearly fills the canvas.
DESKTOP_TILE_RATIO = 0.94
HICOLOR_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def superellipse_mask(size, exponent=TILE_EXPONENT, supersample=4):
    """Anti-aliased superellipse alpha mask, rendered large and downsampled."""
    big = size * supersample
    half = big / 2
    steps = 720
    points = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = half + (half - 0.5) * math.copysign(abs(c) ** (2 / exponent), c)
        y = half + (half - 0.5) * math.copysign(abs(s) ** (2 / exponent), s)
        points.append((x, y))
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).polygon(points, fill=255)
    return mask.resize((size, size), Image.Resampling.LANCZOS)


def cut_tile(source, size, small=False):
    """The tile scaled to `size` px with everything outside it transparent."""
    left, top, right, bottom = TILE_BOX
    if small:
        left, top, right, bottom = left + SMALL_INSET, top + SMALL_INSET, right - SMALL_INSET, bottom - SMALL_INSET
    tile = source.crop((left, top, right, bottom)).resize((size, size), Image.Resampling.LANCZOS)
    tile = tile.convert("RGBA")
    tile.putalpha(superellipse_mask(size))
    return tile


def place(source, canvas, ratio, shadow=False):
    """The tile centred on a transparent `canvas` px square at `ratio` of its width."""
    tile_size = round(canvas * ratio)
    tile = cut_tile(source, tile_size, small=canvas in SMALL_SIZES)
    out = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    offset = (canvas - tile_size) // 2
    if shadow:
        # Soft drop shadow like Apple's template: ~1% offset, ~3% blur.
        alpha = tile.getchannel("A").point(lambda a: a * 0.45)
        blurred = Image.new("L", (canvas, canvas), 0)
        blurred.paste(alpha, (offset, offset + max(1, round(canvas * 0.012))))
        blurred = blurred.filter(ImageFilter.GaussianBlur(max(1, canvas * 0.028)))
        out.putalpha(blurred)
    out.alpha_composite(tile, (offset, offset))
    return out


def save_png(image, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, optimize=True)


def create_ico(images, output_ico_path):
    """Pack RGBA images into a Windows Vista+ ICO file with PNG payloads."""
    payloads = []
    for image in images:
        buffer = io.BytesIO()
        image.save(buffer, format="PNG", optimize=True)
        payloads.append((image.width, image.height, buffer.getvalue()))

    header = struct.pack("<HHH", 0, 1, len(payloads))
    offset = 6 + 16 * len(payloads)
    entries = []
    for w, h, data in payloads:
        # A width/height byte of 0 means 256.
        entries.append(struct.pack("<BBBBHHII", w % 256, h % 256, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    with open(output_ico_path, "wb") as f:
        f.write(header)
        f.writelines(entries)
        f.writelines(data for _, _, data in payloads)
    print(f"Created Windows ICO: {output_ico_path} ({len(payloads)} resolutions)")


def create_icns(source):
    if not shutil.which("iconutil"):
        print("iconutil not found (macOS only); keeping the existing RustAPIIcon.icns")
        return
    iconset_dir = ICONS_DIR / "RustAPI.iconset"
    if iconset_dir.exists():
        shutil.rmtree(iconset_dir)
    iconset_dir.mkdir()
    for size in [16, 32, 128, 256, 512]:
        for scale in [1, 2]:
            suffix = "@2x" if scale == 2 else ""
            save_png(place(source, size * scale, MACOS_TILE_RATIO, shadow=True),
                     iconset_dir / f"icon_{size}x{size}{suffix}.png")
    icns_path = ICONS_DIR / "RustAPIIcon.icns"
    subprocess.run(["iconutil", "-c", "icns", str(iconset_dir), "-o", str(icns_path)], check=True)
    shutil.rmtree(iconset_dir)
    print(f"Created macOS ICNS: {icns_path}")


def main():
    source = Image.open(SOURCE).convert("RGB")
    if source.size != (1024, 1024):
        sys.exit(f"{SOURCE} must be 1024x1024; TILE_BOX is measured against that size")

    # Masters: transparent full tile, and the padded macOS variant.
    save_png(place(source, 1024, DESKTOP_TILE_RATIO), ICONS_DIR / "rustapi-icon.png")
    save_png(place(source, 1024, MACOS_TILE_RATIO, shadow=True), ICONS_DIR / "rustapi-icon-macos.png")

    # Runtime window icons embedded by src/main.rs.
    save_png(place(source, 256, DESKTOP_TILE_RATIO), ICONS_DIR / "rustapi-icon-256.png")
    save_png(place(source, 512, MACOS_TILE_RATIO, shadow=True), ICONS_DIR / "rustapi-icon-macos-512.png")

    # Linux hicolor theme icons (packaged in the tarball and the Docker image).
    for size in HICOLOR_SIZES:
        save_png(place(source, size, DESKTOP_TILE_RATIO),
                 ICONS_DIR / "hicolor" / f"{size}x{size}" / "apps" / "rustapi.png")

    create_ico([place(source, s, DESKTOP_TILE_RATIO) for s in ICO_SIZES], ICONS_DIR / "RustAPIIcon.ico")
    create_icns(source)

    # Web: transparent favicon and logo; iOS masks the touch icon itself, so it
    # must be an opaque full-bleed square filled with the tile's carbon face.
    save_png(place(source, 64, DESKTOP_TILE_RATIO), SITE_DIR / "favicon.png")
    save_png(place(source, 256, DESKTOP_TILE_RATIO), SITE_DIR / "logo.png")
    face = source.getpixel((260, 260))
    touch = Image.new("RGBA", (180, 180), face + (255,))
    touch.alpha_composite(cut_tile(source, 180))
    save_png(touch.convert("RGB"), SITE_DIR / "apple-touch-icon.png")

    print("Icon suite generated successfully!")


if __name__ == "__main__":
    main()
