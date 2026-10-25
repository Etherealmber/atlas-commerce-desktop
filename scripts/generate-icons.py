#!/usr/bin/env python3
"""Regenerate the Atlas Commerce Desktop brand assets.

The brand is monochrome: a near-black rounded tile carrying the white
sunburst symbol, plus sunburst + “Atlas” wordmark artwork for documentation.
No teal, no saffron/orange.

Inputs (committed in the repository):
  * crates/ui/assets/branding/atlas-mark.mask     — sunburst coverage mask
  * crates/ui/assets/branding/atlas-wordmark.mask — full lockup coverage mask

A `.mask` file is a little-endian `u32` width, a little-endian `u32` height
and one 8-bit coverage byte per pixel. The desktop UI tints these masks at
runtime; this script turns the same coverage into static assets.

Outputs (paths are relative to the repository root):
  * packaging/icons/atlas-commerce-desktop.svg   — source vector
  * packaging/macos/icon-1024.png                — macOS master (sips scales it)
  * packaging/windows/atlas-commerce-desktop.ico — multi-size Windows icon
  * packaging/branding/atlas-wordmark-light.png  — README artwork (dark ink)
  * packaging/branding/atlas-wordmark-dark.png   — README artwork (white ink)

Run:  python3 scripts/generate-icons.py   (Pillow required for the rasters)
"""

import math
import struct
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BRANDING = ROOT / "crates/ui/assets/branding"

# 256-unit app-icon design: the same near-black tile as the previous mark,
# now carrying the white sunburst. Geometry mirrors the supplied brand PNG
# (measured on its 252 px symbol: outer radius 126, inner 77, stroke ~9.5,
# 15 rays every 22.5° with a horizontal dash replacing the 0° ray), scaled so
# the symbol occupies a 212 px square centred on the tile.
CANVAS = 256
TILE_RADIUS = 56
MARK_INSET = 22
CENTER = (128.0, 128.0)
OUTER_RADIUS = 106.0
INNER_RADIUS = 65.0
DASH_INNER_RADIUS = 17.0
STROKE_WIDTH = 8
BRAND = "#0A0A0A"
PAPER = "#FAFAFA"


def read_mask(name: str) -> tuple[Image.Image, int, int]:
    data = (BRANDING / name).read_bytes()
    width, height = struct.unpack("<II", data[:8])
    return Image.frombytes("L", (width, height), data[8:]), width, height


def svg_mark() -> str:
    cx, cy = CENTER
    elements = []
    # 15 rays, one every 22.5°; the 0° slot is reserved for the 3 o'clock dash.
    for step in range(1, 16):
        angle = math.radians(step * 22.5)
        cos_a, sin_a = math.cos(angle), math.sin(angle)
        x1 = cx + INNER_RADIUS * cos_a
        y1 = cy + INNER_RADIUS * sin_a
        x2 = cx + OUTER_RADIUS * cos_a
        y2 = cy + OUTER_RADIUS * sin_a
        elements.append(
            f'    <line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"/>'
        )
    elements.append(
        f'    <line x1="{cx + DASH_INNER_RADIUS:.2f}" y1="{cy:.2f}" '
        f'x2="{cx + OUTER_RADIUS:.2f}" y2="{cy:.2f}"/>'
    )
    lines = "\n".join(elements)
    return f"""<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Atlas Commerce Desktop">
  <title>Atlas Commerce Desktop</title>
  <rect width="256" height="256" rx="{TILE_RADIUS}" fill="{BRAND}"/>
  <g stroke="{PAPER}" stroke-width="{STROKE_WIDTH}" stroke-linecap="round">
{lines}
  </g>
</svg>
"""


def render_app_icon(size: int, mark: Image.Image) -> Image.Image:
    """Black rounded tile with the white sunburst coverage mask centred."""
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(
        (0, 0, size - 1, size - 1),
        radius=TILE_RADIUS * size / CANVAS,
        fill=(10, 10, 10, 255),
    )
    mark_size = round(size * (CANVAS - 2 * MARK_INSET) / CANVAS)
    mark_scaled = mark.resize((mark_size, mark_size), Image.LANCZOS)
    # The coverage mask becomes the alpha channel of a neutral white layer.
    layer = Image.new("RGBA", (mark_size, mark_size), (250, 250, 250, 0))
    layer.putalpha(mark_scaled)
    image.alpha_composite(layer, ((size - mark_size) // 2, (size - mark_size) // 2))
    return image


def render_wordmark(mask: Image.Image, dark_background: bool, out: Path) -> None:
    """Trimmed lockup with a calm margin, on its native background."""
    background = (0, 0, 0) if dark_background else (255, 255, 255)
    ink = (250, 250, 250) if dark_background else (10, 10, 10)
    pad = round(mask.height * 0.18)
    canvas = Image.new("RGB", (mask.width + 2 * pad, mask.height + 2 * pad), background)
    artwork = Image.new("RGBA", mask.size, (*ink, 0))
    artwork.putalpha(mask)
    canvas.paste(artwork, (pad, pad), artwork)
    canvas.save(out)
    print(f"wrote {out.relative_to(ROOT)}")


def main() -> None:
    mark, _, _ = read_mask("atlas-mark.mask")
    wordmark, _, _ = read_mask("atlas-wordmark.mask")

    svg_path = ROOT / "packaging/icons/atlas-commerce-desktop.svg"
    svg_path.write_text(svg_mark(), encoding="utf-8")
    print(f"wrote {svg_path.relative_to(ROOT)}")

    png_path = ROOT / "packaging/macos/icon-1024.png"
    render_app_icon(1024, mark).save(png_path)
    print(f"wrote {png_path.relative_to(ROOT)}")

    ico_path = ROOT / "packaging/windows/atlas-commerce-desktop.ico"
    master = render_app_icon(256, mark)
    master.save(
        ico_path,
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print(f"wrote {ico_path.relative_to(ROOT)}")

    branding_dir = ROOT / "packaging/branding"
    branding_dir.mkdir(parents=True, exist_ok=True)
    render_wordmark(wordmark, False, branding_dir / "atlas-wordmark-light.png")
    render_wordmark(wordmark, True, branding_dir / "atlas-wordmark-dark.png")


if __name__ == "__main__":
    main()
