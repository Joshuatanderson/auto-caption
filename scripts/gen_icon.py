#!/usr/bin/env python3
"""
Renders the AutoCap app icon at 1024×1024 using the app's own caption style:
bold Noto Sans, "Auto" in primary (white), "Cap" in accent (cantaloupe mint),
thick black outline, soft mint halo behind. Two-line layout for legibility at
small sizes (32px).

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
BG_COLOR = "#FDF6EC"          # warm cream; reads cleanly in both light & dark docks
OUTLINE = "#0A0A0A"
PRIMARY = "#FFFFFF"
ACCENT = sys.argv[1] if len(sys.argv) > 1 else "#61C695"  # cantaloupe mint

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

    # Sharp layer: white "Auto" + accent "Cap", both with black outline.
    d = ImageDraw.Draw(img)
    d.text((auto_x, auto_y), "Auto", fill=PRIMARY, font=font,
           stroke_width=SHARP_STROKE, stroke_fill=OUTLINE)
    d.text((cap_x, cap_y), "Cap", fill=ACCENT, font=font,
           stroke_width=SHARP_STROKE, stroke_fill=OUTLINE)

    img.save(OUT_PATH)
    print(f"wrote {OUT_PATH} ({CANVAS}×{CANVAS})")


if __name__ == "__main__":
    main()
