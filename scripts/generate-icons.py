#!/usr/bin/env python3
"""
Generate Hermes Agent icons from the caduceus SVG.
Outputs PNG, ICO, and ICNS files for the Tauri app.

Usage: python scripts/generate-icons.py

Requires: cairosvg or rsvg-convert for SVG->PNG conversion.
If neither is available, falls back to a simple colored placeholder.
"""

import subprocess
import sys
from pathlib import Path

SVG_SOURCE = Path.home() / "hermes-agent" / "acp_registry" / "icon.svg"
ICONS_DIR = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"

REQUIRED_SIZES = [
    (32, "32x32.png"),
    (128, "128x128.png"),
    (256, "128x128@2x.png"),
    (256, "icon.png"),
    (30, "Square30x30Logo.png"),
    (44, "Square44x44Logo.png"),
    (71, "Square71x71Logo.png"),
    (89, "Square89x89Logo.png"),
    (107, "Square107x107Logo.png"),
    (142, "Square142x142Logo.png"),
    (150, "Square150x150Logo.png"),
    (284, "Square284x284Logo.png"),
    (310, "Square310x310Logo.png"),
    (50, "StoreLogo.png"),
]


def find_converter():
    """Find an SVG-to-PNG converter."""
    # Try rsvg-convert (most reliable on macOS with brew)
    try:
        subprocess.run(["rsvg-convert", "--version"], capture_output=True, check=True)
        return ("rsvg", None)
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass

    # Try cairosvg
    try:
        import cairosvg
        return ("cairosvg", cairosvg)
    except (ImportError, OSError):
        pass

    # Try Inkscape
    try:
        subprocess.run(["inkscape", "--version"], capture_output=True, check=True)
        return ("inkscape", None)
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass

    return None


def convert_cairosvg(cairosvg_mod, svg_path, png_path, size):
    cairosvg_mod.svg2png(url=str(svg_path), write_to=str(png_path),
                         output_width=size, output_height=size)
    print(f"  -> {png_path.name} ({size}x{size})")


def convert_rsvg(svg_path, png_path, size):
    subprocess.run([
        "rsvg-convert",
        "-w", str(size),
        "-h", str(size),
        "-o", str(png_path),
        str(svg_path),
    ], check=True, capture_output=True)
    print(f"  -> {png_path.name} ({size}x{size})")


def convert_inkscape(svg_path, png_path, size):
    subprocess.run([
        "inkscape",
        "-o", str(png_path),
        "-w", str(size),
        "-h", str(size),
        str(svg_path),
    ], check=True, capture_output=True)
    print(f"  -> {png_path.name} ({size}x{size})")


def generate_ico(icons_dir):
    """Generate ICO from 32x32 and 256x256 PNGs."""
    try:
        from PIL import Image

        png32 = icons_dir / "32x32.png"
        png256 = icons_dir / "icon.png"
        ico_path = icons_dir / "icon.ico"

        if png32.exists() and png256.exists():
            img32 = Image.open(png32)
            img256 = Image.open(png256)
            img256.save(ico_path, format="ICO", sizes=[(256, 256), (32, 32)])
            print(f"  -> icon.ico generated")
        elif png32.exists():
            img32 = Image.open(png32)
            img32.save(ico_path, format="ICO")
            print(f"  -> icon.ico generated (32x32 only)")
    except ImportError:
        print("  SKIP icon.ico (Pillow not installed)")


def generate_icns(icons_dir):
    """Generate ICNS from 256x256 PNG."""
    try:
        from PIL import Image

        png256 = icons_dir / "icon.png"
        icns_path = icons_dir / "icon.icns"

        if png256.exists():
            img = Image.open(png256)
            img.save(icns_path, format="ICNS")
            print(f"  -> icon.icns generated")
    except ImportError:
        print("  SKIP icon.icns (Pillow not installed)")


def main():
    svg_path = SVG_SOURCE
    if not svg_path.exists():
        print(f"SVG icon not found at {svg_path}")
        print(f"Using fallback: generating solid-color icons...")
        generate_fallback_icons()
        return

    converter = find_converter()
    if not converter:
        print("No SVG converter found. Install one of:")
        print("  pip install cairosvg")
        print("  brew install librsvg  (for rsvg-convert)")
        print("  brew install inkscape")
        print("\nFalling back to solid-color placeholder icons...")
        generate_fallback_icons()
        return

    print(f"Using converter: {converter[0]}")
    print(f"Source: {svg_path}")
    print(f"Output: {ICONS_DIR}")

    ICONS_DIR.mkdir(parents=True, exist_ok=True)

    for size, name in REQUIRED_SIZES:
        png_path = ICONS_DIR / name
        if converter[0] == "cairosvg":
            convert_cairosvg(converter[1], svg_path, png_path, size)
        elif converter[0] == "rsvg":
            convert_rsvg(svg_path, png_path, size)
        elif converter[0] == "inkscape":
            convert_inkscape(svg_path, png_path, size)

    generate_ico(ICONS_DIR)
    generate_icns(ICONS_DIR)

    print("\nIcons generated successfully!")


def generate_fallback_icons():
    """Generate simple colored placeholder icons."""
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    try:
        from PIL import Image, ImageDraw, ImageFont

        for size, name in REQUIRED_SIZES:
            img = Image.new("RGBA", (size, size), (4, 28, 28, 255))  # #041c1c
            draw = ImageDraw.Draw(img)

            # Draw a simple caduceus-like symbol
            cx, cy = size // 2, size // 2
            r = size // 4

            # Circle (orb)
            draw.ellipse([cx - r//2, cy - r, cx + r//2, cy], fill=(255, 215, 0, 255))

            try:
                font_size = max(8, size // 4)
                font = ImageFont.truetype("/System/Library/Fonts/Supplemental/Arial.ttf", font_size)
                draw.text((cx - font_size//2, cy - font_size//2), "☤",
                         fill=(255, 215, 0, 255), font=font)
            except Exception:
                pass  # Just use the circle if font loads fail

            png_path = ICONS_DIR / name
            img.save(png_path, "PNG")

        generate_ico(ICONS_DIR)
        generate_icns(ICONS_DIR)
        print(f"Placeholder icons generated in {ICONS_DIR}")
    except ImportError:
        print("Pillow not installed. Cannot generate icons.")
        print(f"Manual action: copy icons from a previous build or use the default Tauri icons.")
        print(f"Target directory: {ICONS_DIR}")


if __name__ == "__main__":
    main()
