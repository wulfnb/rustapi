#!/usr/bin/env python3
"""Generate multi-resolution icons for RustAPI (macOS .icns, Windows .ico, web PNGs).

Uses macOS `sips` and `iconutil` to produce standard Apple ICNS bundles and
packs PNG images into a standard Windows Vista+ multi-resolution ICO file.
"""
from pathlib import Path
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path("/Users/noufalbabu/.gemini/antigravity/brain/0e3387c2-1b2c-44be-be3c-3a28db06c76d/rustapi_icon_concept_2_hyperbolt_brackets_png_1793288507883.jpg")
ICONS_DIR = ROOT / "assets" / "icons"
SITE_DIR = ROOT / "site"


def create_ico(png_paths, output_ico_path):
    """Pack multiple PNG files into a valid Windows ICO file."""
    images_data = []
    for p in png_paths:
        data = p.read_bytes()
        # Read PNG width & height from IHDR chunk (bytes 16..24)
        w, h = struct.unpack(">II", data[16:24])
        images_data.append((w, h, data))

    # ICO Header: 2 bytes reserved (0), 2 bytes type (1 = ICO), 2 bytes count
    count = len(images_data)
    header = struct.pack("<HHH", 0, 1, count)
    
    # Directory entries: 16 bytes each
    # Offset of first image data starts after header + directory entries
    offset = 6 + (16 * count)
    entries = []
    payloads = []
    
    for w, h, data in images_data:
        size = len(data)
        # width & height: byte (0 means 256)
        bw = 0 if w >= 256 else w
        bh = 0 if h >= 256 else h
        entry = struct.pack("<BBBBHHII", bw, bh, 0, 0, 1, 32, size, offset)
        entries.append(entry)
        payloads.append(data)
        offset += size

    with open(output_ico_path, "wb") as f:
        f.write(header)
        for e in entries:
            f.write(e)
        for d in payloads:
            f.write(d)
    print(f"Created Windows ICO: {output_ico_path} ({count} resolutions)")


def main():
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    SITE_DIR.mkdir(parents=True, exist_ok=True)

    master_png = ICONS_DIR / "rustapi-icon.png"
    source_copy = ICONS_DIR / "rustapi-icon-source.png"

    # Convert JPG master to high-res PNG
    subprocess.run(["sips", "-s", "format", "png", str(SOURCE), "--out", str(master_png)], check=True)
    shutil.copy2(master_png, source_copy)

    # Generate individual resolution PNGs
    resolutions = [16, 24, 32, 48, 64, 128, 180, 256, 512, 1024]
    temp_dir = ICONS_DIR / "sizes"
    temp_dir.mkdir(exist_ok=True)

    for res in resolutions:
        out = temp_dir / f"icon_{res}x{res}.png"
        subprocess.run(["sips", "-z", str(res), str(res), str(master_png), "--out", str(out)], check=True)

    # 1. Primary exports
    shutil.copy2(temp_dir / "icon_256x256.png", ICONS_DIR / "rustapi-icon-256.png")
    shutil.copy2(temp_dir / "icon_64x64.png", SITE_DIR / "favicon.png")
    shutil.copy2(temp_dir / "icon_256x256.png", SITE_DIR / "logo.png")
    shutil.copy2(temp_dir / "icon_180x180.png", SITE_DIR / "apple-touch-icon.png")

    # 2. Build macOS ICNS bundle
    iconset_dir = ICONS_DIR / "RustAPI.iconset"
    if iconset_dir.exists():
        shutil.rmtree(iconset_dir)
    iconset_dir.mkdir()

    mac_sizes = [
        (16, 1, "icon_16x16.png"),
        (16, 2, "icon_16x16@2x.png"),
        (32, 1, "icon_32x32.png"),
        (32, 2, "icon_32x32@2x.png"),
        (128, 1, "icon_128x128.png"),
        (128, 2, "icon_128x128@2x.png"),
        (256, 1, "icon_256x256.png"),
        (256, 2, "icon_256x256@2x.png"),
        (512, 1, "icon_512x512.png"),
        (512, 2, "icon_512x512@2x.png"),
    ]

    for size, scale, name in mac_sizes:
        dim = size * scale
        target = iconset_dir / name
        subprocess.run(["sips", "-z", str(dim), str(dim), str(master_png), "--out", str(target)], check=True)

    icns_path = ICONS_DIR / "RustAPIIcon.icns"
    subprocess.run(["iconutil", "-c", "icns", str(iconset_dir), "-o", str(icns_path)], check=True)
    shutil.rmtree(iconset_dir)
    print(f"Created macOS ICNS: {icns_path}")

    # 3. Build Windows ICO
    ico_res = [16, 24, 32, 48, 64, 128, 256]
    ico_pngs = [temp_dir / f"icon_{r}x{r}.png" for r in ico_res]
    ico_path = ICONS_DIR / "RustAPIIcon.ico"
    create_ico(ico_pngs, ico_path)

    # Clean up temp size folder
    shutil.rmtree(temp_dir)
    print("Icon suite generated successfully!")


if __name__ == "__main__":
    main()
