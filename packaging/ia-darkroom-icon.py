"""IA Darkroom's app icon: a dark squircle, the Infinite Arch in gold, a safelight glow inside.
Renders at 4x and downsamples. Writes 1024 and 512 PNGs plus an .icns."""
import math, os, sys
from PIL import Image, ImageDraw, ImageFilter, ImageChops

OUT = sys.argv[1]
S = 1024
K = 4           # supersample
W = S * K

# macOS icon grid: the shape is 824/1024 of the canvas, centered
side = int(824 * K)
off = (W - side) // 2

def squircle_mask(size, radius_frac=0.2237, n=5):
    """Apple-ish continuous-corner rounded square."""
    m = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(m)
    r = size * radius_frac
    # approximate superellipse corners by a polygon
    pts = []
    steps = 400
    cx = cy = size / 2
    a = size / 2
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s_ = math.cos(t), math.sin(t)
        x = cx + a * math.copysign(abs(c) ** (2 / n), c)
        y = cy + a * math.copysign(abs(s_) ** (2 / n), s_)
        pts.append((x, y))
    d.polygon(pts, fill=255)
    return m

mask = squircle_mask(side)

# background: charcoal with a soft vertical gradient and a faint vignette
bg = Image.new("RGB", (side, side), (0x13, 0x19, 0x17))
grad = Image.linear_gradient("L").resize((side, side))          # top 0 -> bottom 255
top = Image.new("RGB", (side, side), (0x1c, 0x23, 0x20))
bot = Image.new("RGB", (side, side), (0x0c, 0x10, 0x0f))
bg = Image.composite(bot, top, grad)

layer = bg.copy()

# geometry of the arch
cx = side / 2
arch_w = side * 0.50          # outer width of the arch
stroke = side * 0.078
outer_r = arch_w / 2
inner_r = outer_r - stroke
top_y = side * 0.235          # top of the outer arch
spring_y = top_y + outer_r    # where the curve meets the legs
base_y = side * 0.77          # bottom of the legs

# safelight glow inside the arch opening (warm amber-red), brightest at the top center, fading down
glow = Image.new("RGB", (side, side), (0, 0, 0))
gd = ImageDraw.Draw(glow)
# the opening: inner semicircle + rectangle down to base
opening = Image.new("L", (side, side), 0)
od = ImageDraw.Draw(opening)
od.pieslice([cx - inner_r, top_y + stroke, cx + inner_r, top_y + stroke + 2 * inner_r], 180, 360, fill=255)
od.rectangle([cx - inner_r, spring_y, cx + inner_r, base_y], fill=255)
# radial glow centred a bit above the spring line
gl = Image.new("L", (side, side), 0)
gld = ImageDraw.Draw(gl)
gcx, gcy = cx, top_y + stroke + inner_r * 0.55
for i in range(60, 0, -1):
    r = inner_r * 1.35 * i / 60
    v = int(255 * (1 - i / 60) ** 1.1)
    gld.ellipse([gcx - r, gcy - r * 1.25, gcx + r, gcy + r * 1.25], fill=v)
gl = gl.filter(ImageFilter.GaussianBlur(side * 0.03))
# vertical fade so the bottom of the opening goes dark
fade = Image.linear_gradient("L").resize((side, side)).point(lambda p: max(0, 255 - int(max(0, p - 60) * 1.25)))
gl = ImageChops.multiply(gl, fade)
gl = ImageChops.multiply(gl, opening)
amber = Image.new("RGB", (side, side), (0xd8, 0x4a, 0x2a))   # safelight
layer = Image.composite(amber, layer, gl)
# a brighter core
core = gl.point(lambda p: int((p / 255) ** 2.2 * 255))
hot = Image.new("RGB", (side, side), (0xff, 0x9a, 0x4a))
layer = Image.composite(hot, layer, core.point(lambda p: int(p * 0.8)))

# a faint warm bloom spilling past the arch onto the dark
bloom = gl.filter(ImageFilter.GaussianBlur(side * 0.06)).point(lambda p: int(p * 0.35))
layer = Image.composite(amber, layer, bloom)

# the arch itself in gold, with a slight light-from-above
arch = Image.new("L", (side, side), 0)
ad = ImageDraw.Draw(arch)
ad.pieslice([cx - outer_r, top_y, cx + outer_r, top_y + 2 * outer_r], 180, 360, fill=255)
ad.pieslice([cx - inner_r, top_y + stroke, cx + inner_r, top_y + stroke + 2 * inner_r], 180, 360, fill=0)
ad.rectangle([cx - outer_r, spring_y - 1, cx - inner_r, base_y], fill=255)
ad.rectangle([cx + inner_r, spring_y - 1, cx + outer_r, base_y], fill=255)
# plinths
pl_h = stroke * 0.55
pl_w = stroke * 1.35
ad.rectangle([cx - outer_r - (pl_w - stroke) / 2, base_y, cx - outer_r - (pl_w - stroke) / 2 + pl_w, base_y + pl_h], fill=255)
ad.rectangle([cx + inner_r - (pl_w - stroke) / 2, base_y, cx + inner_r - (pl_w - stroke) / 2 + pl_w, base_y + pl_h], fill=255)

gold_top = Image.new("RGB", (side, side), (0xe9, 0xcf, 0x86))
gold_bot = Image.new("RGB", (side, side), (0xb8, 0x97, 0x4c))
gold = Image.composite(gold_bot, gold_top, grad)
# soft drop shadow under the arch
shadow = arch.filter(ImageFilter.GaussianBlur(side * 0.012))
shadow_img = Image.new("RGB", (side, side), (0, 0, 0))
layer = Image.composite(shadow_img, layer, shadow.point(lambda p: int(p * 0.55)))
layer = Image.composite(gold, layer, arch)

# a thin inner highlight line along the top edge of the squircle
edge = Image.new("L", (side, side), 0)
ed = ImageDraw.Draw(edge)
inner = squircle_mask(side - 2 * int(3 * K)).resize((side - 2 * int(3 * K), side - 2 * int(3 * K)))
ring = ImageChops.subtract(mask, Image.new("L", (side, side), 0))
ring_in = Image.new("L", (side, side), 0)
ring_in.paste(inner, (int(3 * K), int(3 * K)))
ring = ImageChops.subtract(mask, ring_in)
ring = ImageChops.multiply(ring, grad.point(lambda p: 255 - p))
hl = Image.new("RGB", (side, side), (0xff, 0xf6, 0xe0))
layer = Image.composite(hl, layer, ring.point(lambda p: int(p * 0.35)))

# assemble on transparent canvas with the macOS drop shadow
canvas = Image.new("RGBA", (W, W), (0, 0, 0, 0))
sh = Image.new("L", (W, W), 0)
sh.paste(mask, (off, off + int(10 * K)))
sh = sh.filter(ImageFilter.GaussianBlur(14 * K)).point(lambda p: int(p * 0.5))
canvas.paste((0, 0, 0, 255), (0, 0), sh)
rgba = layer.convert("RGBA")
rgba.putalpha(mask)
canvas.alpha_composite(rgba, (off, off))

os.makedirs(OUT, exist_ok=True)
final = canvas.resize((S, S), Image.LANCZOS)
final.save(os.path.join(OUT, "ia-darkroom-1024.png"))
final.resize((512, 512), Image.LANCZOS).save(os.path.join(OUT, "ia-darkroom-512.png"))
final.resize((256, 256), Image.LANCZOS).save(os.path.join(OUT, "ia-darkroom-256.png"))
# a 1024 without the outer shadow/margins for the window icon on Linux/Windows (full-bleed look is wrong there; keep the same)
print("ok")
