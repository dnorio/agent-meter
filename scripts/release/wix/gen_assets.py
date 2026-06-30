#!/usr/bin/env python3
"""Generate branded installer assets (icon + wizard bitmaps) from the brand logo.

Outputs (under ./assets):
  - icon.ico    : product icon for ARP + shortcuts
  - banner.bmp  : 493x58 top banner (WixUIBannerBmp)
  - dialog.bmp  : 493x312 welcome/finish image (WixUIDialogBmp)

Colors come from crates/collector/ui/_static/favicon.svg gradient.
Requires Pillow.
"""
from PIL import Image, ImageDraw, ImageFont
import os

C1 = (124, 92, 255)   # #7c5cff
C2 = (25, 195, 167)   # #19c3a7
WHITE = (255, 255, 255)

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "assets")
os.makedirs(OUT, exist_ok=True)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def diagonal_gradient(w, h):
    img = Image.new("RGB", (w, h), C1)
    px = img.load()
    maxd = (w - 1) + (h - 1)
    for y in range(h):
        for x in range(w):
            px[x, y] = lerp(C1, C2, (x + y) / maxd)
    return img


def draw_logo_marks(draw, cx, cy, scale, color=WHITE):
    bars = [(9, 21, 13), (16, 21, 16), (23, 21, 9)]
    dots = [(9, 11), (16, 14), (23, 10)]

    def T(x, y):
        return (cx + (x - 16) * scale, cy + (y - 16) * scale)

    lw = max(2, int(2.5 * scale))
    for x, y0, y1 in bars:
        draw.line([T(x, y0), T(x, y1)], fill=color, width=lw)
    r = max(2, int(1.6 * scale))
    for x, y in dots:
        c = T(x, y)
        draw.ellipse([c[0] - r, c[1] - r, c[0] + r, c[1] + r], fill=color)


def make_icon():
    base = 256
    ic = Image.new("RGBA", (base, base), (0, 0, 0, 0))
    grad = diagonal_gradient(base, base).convert("RGBA")
    mask = Image.new("L", (base, base), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, base - 1, base - 1], radius=56, fill=255)
    ic.paste(grad, (0, 0), mask)
    draw_logo_marks(ImageDraw.Draw(ic), base / 2, base / 2, base / 32)
    ic.save(os.path.join(OUT, "icon.ico"),
            sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])


def make_banner():
    W, H = 493, 58
    ban = Image.new("RGB", (W, H), WHITE)
    ban.paste(diagonal_gradient(150, H), (0, 0))
    draw_logo_marks(ImageDraw.Draw(ban), 29, H / 2, H / 32 * 0.8)
    ban.save(os.path.join(OUT, "banner.bmp"))


def make_dialog():
    W, H = 493, 312
    dlg = diagonal_gradient(W, H)
    dd = ImageDraw.Draw(dlg)
    draw_logo_marks(dd, 120, 120, 6, WHITE)
    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 30)
        fsm = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 16)
    except Exception:
        font = ImageFont.load_default()
        fsm = ImageFont.load_default()
    dd.text((40, 210), "agent-meter", font=font, fill=WHITE)
    dd.text((42, 250), "proxy installer", font=fsm, fill=(235, 235, 255))
    dlg.save(os.path.join(OUT, "dialog.bmp"))


if __name__ == "__main__":
    make_icon()
    make_banner()
    make_dialog()
    print("assets written to", OUT)
