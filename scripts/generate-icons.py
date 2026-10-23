#!/usr/bin/env python3
"""Regenerate the Atlas Commerce Desktop brand assets.

The brand mark is monochrome, matching the Atlas Commerce web and Android
design systems: a near-black rounded tile, a white rounded inset and the
"A" in near-black. No teal, no saffron/orange.

Outputs (paths are relative to the repository root):
  * packaging/icons/atlas-commerce-desktop.svg  — source vector
  * packaging/macos/icon-1024.png               — macOS master (sips scales it)
  * packaging/windows/atlas-commerce-desktop.ico — multi-size Windows icon

Run:  python3 scripts/generate-icons.py   (Pillow required for the rasters)
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent

# 256-unit design, shared with the SVG and the tray icon rasterizer.
CANVAS = 256
BRAND = (10, 10, 10, 255)  # #0A0A0A
PAPER = (250, 250, 250, 255)  # #FAFAFA
OUTER_RADIUS = 56
INSET = 44
INNER_RADIUS = 38
STROKE_WIDTH = 16
# A geometry: two legs meeting at the apex, then the crossbar.
APEX = (128, 76)
LEFT_FOOT = (86, 176)
RIGHT_FOOT = (170, 176)
BAR_LEFT = (100, 146)
BAR_RIGHT = (156, 146)

SVG = f"""<svg width="256" height="256" viewBox="0 0 256 256" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Atlas Commerce Desktop">
  <title>Atlas Commerce Desktop</title>
  <rect width="256" height="256" rx="{OUTER_RADIUS}" fill="#0A0A0A"/>
  <rect x="{INSET}" y="{INSET}" width="{CANVAS - 2 * INSET}" height="{CANVAS - 2 * INSET}" rx="{INNER_RADIUS}" fill="#FAFAFA"/>
  <path d="M{LEFT_FOOT[0]} {LEFT_FOOT[1]} L{APEX[0]} {APEX[1]} L{RIGHT_FOOT[0]} {RIGHT_FOOT[1]} M{BAR_LEFT[0]} {BAR_LEFT[1]} H{BAR_RIGHT[0]}" stroke="#0A0A0A" stroke-width="{STROKE_WIDTH}"
        stroke-linecap="round" stroke-linejoin="round"/>
</svg>
"""


def render(size: int) -> Image.Image:
    """Render the mark at an arbitrary square size."""
    scale = size / CANVAS
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    def box(inset: float) -> tuple[float, float, float, float]:
        return inset * scale, inset * scale, size - inset * scale, size - inset * scale

    draw.rounded_rectangle(box(0), radius=OUTER_RADIUS * scale, fill=BRAND)
    draw.rounded_rectangle(
        box(INSET), radius=INNER_RADIUS * scale, fill=PAPER
    )

    def point(vertex: tuple[int, int]) -> tuple[float, float]:
        return vertex[0] * scale, vertex[1] * scale

    width = max(1, round(STROKE_WIDTH * scale))
    draw.line(
        [point(LEFT_FOOT), point(APEX), point(RIGHT_FOOT)],
        fill=BRAND,
        width=width,
        joint="curve",
    )
    draw.line(
        [point(BAR_LEFT), point(BAR_RIGHT)],
        fill=BRAND,
        width=width,
        joint="curve",
    )
    # Round the leg feet and the apex, mirroring SVG stroke-linecap="round".
    cap = width / 2
    for vertex in (LEFT_FOOT, RIGHT_FOOT, BAR_LEFT, BAR_RIGHT):
        x, y = point(vertex)
        draw.ellipse((x - cap, y - cap, x + cap, y + cap), fill=BRAND)
    return image


def main() -> None:
    svg_path = ROOT / "packaging/icons/atlas-commerce-desktop.svg"
    svg_path.write_text(SVG, encoding="utf-8")
    print(f"wrote {svg_path.relative_to(ROOT)}")

    png_path = ROOT / "packaging/macos/icon-1024.png"
    render(1024).save(png_path)
    print(f"wrote {png_path.relative_to(ROOT)}")

    ico_path = ROOT / "packaging/windows/atlas-commerce-desktop.ico"
    master = render(256)
    master.save(
        ico_path,
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print(f"wrote {ico_path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
