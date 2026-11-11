# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "pillow"]
# ///
"""Ember app icon: a blue flame grown from domain-warped fractal noise.

Run: uv run res/icons/gen_icon.py   (writes PNGs next to this file)

The flame is a teardrop density field whose edge is pushed around by fBm value
noise that is itself warped by another fBm (Inigo Quilez-style domain warping),
stretched vertically so it reads as rising tongues. Intensity maps through a
blue color ramp; a blurred copy adds glow. Seeded, so output is reproducible.
"""
import math
import os

import numpy as np
from PIL import Image, ImageFilter

N = 1024  # render size, downsampled for each output size
SEED = 7
rng = np.random.default_rng(SEED)
LATTICE = rng.random((256, 256))


def smooth(t):
    return t * t * (3 - 2 * t)


def value_noise(x, y):
    xi, yi = np.floor(x).astype(int), np.floor(y).astype(int)
    xf, yf = smooth(x - xi), smooth(y - yi)
    xi, yi = xi & 255, yi & 255
    a = LATTICE[yi, xi]
    b = LATTICE[yi, (xi + 1) & 255]
    c = LATTICE[(yi + 1) & 255, xi]
    d = LATTICE[(yi + 1) & 255, (xi + 1) & 255]
    return (a + (b - a) * xf) + ((c + (d - c) * xf) - (a + (b - a) * xf)) * yf


def fbm(x, y, octaves=6):
    v, amp, freq = 0.0, 0.5, 1.0
    for i in range(octaves):
        # rotate each octave a little so lattice artifacts don't line up
        a = 0.5 * i
        xr, yr = x * math.cos(a) - y * math.sin(a), x * math.sin(a) + y * math.cos(a)
        v = v + amp * value_noise(xr * freq + 13.1 * i, yr * freq + 7.7 * i)
        amp *= 0.5
        freq *= 2.03
    return v


ys, xs = np.mgrid[0:N, 0:N].astype(np.float64)
u = (xs / N - 0.5) * 2.0          # -1 .. 1, left to right
v = 1.0 - ys / N                   # 0 bottom .. 1 top

# domain warp: noise sampled at noise-displaced coordinates, stretched upward
px, py = u * 2.2, v * 3.2
qx, qy = fbm(px, py), fbm(px + 5.2, py + 1.3)
rx, ry = fbm(px + 4.0 * qx + 1.7, py + 4.0 * qy + 9.2), fbm(px + 4.0 * qx + 8.3, py + 4.0 * qy + 2.8)
n = fbm(px + 3.5 * rx, py - 2.5 + 3.5 * ry)  # 0..~1

# flame body: narrow teardrop whose edge is eaten by noise more and more toward the
# top, so it splits into separate tongues instead of a smooth cone
base_y, top_y = 0.22, 0.90
t_raw = (v - base_y) / (top_y - base_y)
t = np.clip(t_raw, 0, 1)
profile = 0.34 * np.sqrt(np.clip(1 - t, 0, 1)) * (1 + 0.55 * np.exp(-((t - 0.18) / 0.2) ** 2))
# rounded bottom: below base_y the width closes like a circle
profile = profile * np.sqrt(np.clip(1 - (np.minimum(t_raw, 0) / 0.22) ** 2, 0, 1))
fine = fbm(u * 5.5 + 2.0 * rx, v * 3.0 - 1.7 + 2.0 * ry, octaves=5)  # tongue-scale detail
sway = 0.16 * (qx - 0.5) * t * 1.6
f = profile - np.abs(u - sway)
f = f + (fine - 0.5) * (0.10 + 0.55 * t) + (n - 0.5) * 0.12
f = f - 0.10 * t ** 3                                              # tips burn out
density = np.clip(f / 0.11, 0, 1)

# small hot core near the base
core = np.exp(-((u / 0.13) ** 2 + ((v - 0.24) / 0.13) ** 2))
streaks = fbm(u * 9.0 + 3.0 * qx, v * 1.4 - 0.6, octaves=4)   # vertical flicker lines
heat = density * (0.30 + 0.55 * (1 - t)) * (0.55 + 0.7 * streaks) * (0.8 + 0.4 * fine)
heat = np.clip(heat + 0.32 * core * density * (0.7 + 0.6 * streaks), 0, 1)

# blue ramp: transparent -> deep indigo -> electric blue -> cyan -> white-hot
stops = [(0.00, (8, 12, 40, 0)), (0.12, (16, 28, 110, 200)), (0.35, (30, 90, 255, 255)),
         (0.62, (60, 200, 255, 255)), (0.85, (190, 245, 255, 255)), (1.00, (255, 255, 255, 255))]
rgba = np.zeros((N, N, 4))
for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
    m = (heat >= t0) & (heat <= t1)
    k = ((heat - t0) / (t1 - t0))[..., None]
    rgba[m] = (np.array(c0) * (1 - k) + np.array(c1) * k)[m]
flame = Image.fromarray(rgba.astype(np.uint8), "RGBA")

# background: dark rounded square with a faint blue glow behind the flame
bg = np.zeros((N, N, 4))
glow = np.exp(-((u / 0.75) ** 2 + ((v - 0.42) / 0.6) ** 2))
bg[..., 0] = 10 + 10 * glow
bg[..., 1] = 12 + 22 * glow
bg[..., 2] = 24 + 60 * glow
bg[..., 3] = 255
back = Image.fromarray(bg.astype(np.uint8), "RGBA")

halo = flame.filter(ImageFilter.GaussianBlur(N * 0.025))
img = Image.alpha_composite(back, halo)
img = Image.alpha_composite(img, flame)

# squircle mask (superellipse, n=5) with margin, like modern app icons
m = 0.875
sq = (np.abs(u / m) ** 5 + np.abs((ys / N - 0.5) * 2 / m) ** 5) <= 1
mask = Image.fromarray((sq * 255).astype(np.uint8), "L").filter(ImageFilter.GaussianBlur(1.2))
out = Image.new("RGBA", (N, N), (0, 0, 0, 0))
out.paste(img, (0, 0), mask)

here = os.path.dirname(os.path.abspath(__file__))
for size in (512, 256, 128, 64, 48, 32, 24, 16):
    out.resize((size, size), Image.LANCZOS).save(os.path.join(here, f"ember-{size}.png"))
print("wrote ember-{512..16}.png")
