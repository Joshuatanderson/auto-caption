#!/usr/bin/env python3
"""
Renders the AutoCap app icon at 1024×1024 using the app's own caption style,
in the Happycampr brand palette: bold Noto Sans, "Auto" in burnt, "Cap" in
graham (the brand accent), burnt outline, soft graham halo behind, on a
marshmallow ground. Two-line layout for legibility at small sizes (32px).

Run: python3 scripts/gen_icon.py [optional_accent_hex]
Outputs: icon-source.png in the repo root. Feed that into `bun run tauri icon`.
"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
FONT_PATH = ROOT / "static" / "fonts" / "NotoSans-Bold.ttf"
OUT_PATH = ROOT / "icon-source.png"

CANVAS = 1024
BG_COLOR = "#F5F1E8"          # marshmallow; Happycampr light surface, reads in both docks
OUTLINE = "#2B1810"           # burnt
PRIMARY = "#2B1810"           # burnt — "Auto"
ACCENT = sys.argv[1] if len(sys.argv) > 1 else "#946334"  # graham — Happycampr brand accent

CORNER_RADIUS = 180
FONT_SIZE = 360
SHARP_STROKE = 14
GLOW_STROKE = 28
GLOW_BLUR = 22


def main():
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    ImageDraw.Draw(img).rounded_rectangle(
        (0, 0, CANVAS, CANVAS), radius=CORNER_RADIUS, fill=BG_COLOR
    )

    font = ImageFont.truetype(str(FONT_PATH), FONT_SIZE)

    def place(text: str) -> tuple[int, int, int]:
        l, t, r, b = font.getbbox(text)
        return r - l, -l, -t  # width, x-offset to origin, y-offset to origin

    auto_w, auto_dx, auto_dy = place("Auto")
    cap_w, cap_dx, cap_dy = place("Cap")

    line_h = int(FONT_SIZE * 1.02)
    block_h = line_h * 2
    top = (CANVAS - block_h) // 2

    auto_x = (CANVAS - auto_w) // 2 + auto_dx
    auto_y = top + auto_dy
    cap_x = (CANVAS - cap_w) // 2 + cap_dx
    cap_y = top + line_h + cap_dy

    # Glow layer: thick accent-colored pass, gaussian-blurred.
    glow = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.text((auto_x, auto_y), "Auto", fill=ACCENT, font=font,
            stroke_width=GLOW_STROKE, stroke_fill=ACCENT)
    gd.text((cap_x, cap_y), "Cap", fill=ACCENT, font=font,
            stroke_width=GLOW_STROKE, stroke_fill=ACCENT)
    glow = glow.filter(ImageFilter.GaussianBlur(radius=GLOW_BLUR))

    img = Image.alpha_composite(img, glow)

    # Sharp layer: burnt "Auto" + graham "Cap", both with burnt outline.
    d = ImageDraw.Draw(img)
    d.text((auto_x, auto_y), "Auto", fill=PRIMARY, font=font,
           stroke_width=SHARP_STROKE, stroke_fill=OUTLINE)
    d.text((cap_x, cap_y), "Cap", fill=ACCENT, font=font,
           stroke_width=SHARP_STROKE, stroke_fill=OUTLINE)

    img.save(OUT_PATH)
    print(f"wrote {OUT_PATH} ({CANVAS}×{CANVAS})")


if __name__ == "__main__":
    main()
