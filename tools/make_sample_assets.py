#!/usr/bin/env python3
"""Generate the five bundled sample images.

Pure standard library (zlib + struct write the PNGs), fully deterministic, so
the repo has real image assets without vendoring third-party pictures.

All patterns are written in normalised coordinates (fractions of width/height),
so changing W/H keeps the same composition instead of clipping or blowing out
the vignette.

Run once:  python3 tools/make_sample_assets.py
"""
import math
import os
import struct
import zlib

W, H = 1280, 720
TAU = 2.0 * math.pi


def clamp8(v):
    v = int(v)
    return 0 if v < 0 else (255 if v > 255 else v)


def write_png(path, w, h, rows):
    raw = b"".join(b"\x00" + bytes(row) for row in rows)

    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data +
                struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", zlib.compress(raw, 9)))
        f.write(chunk(b"IEND", b""))


def aurora(w, h):
    rows = []
    for y in range(h):
        v = y / h
        row = bytearray()
        for x in range(w):
            u = x / w
            band = math.sin(u * 23.0 + v * 11.0) * math.sin(v * 7.0 + 0.7)
            glow = max(0.0, math.sin(u * 8.5 + 1.1 * math.sin(v * 6.9)))
            r = 22 + 40 * v + 150 * glow * max(0.0, band)
            g = 34 + 110 * v + 150 * glow * max(0.0, band + 0.25)
            b = 84 + 120 * v + 90 * glow
            row += bytes((clamp8(r), clamp8(g), clamp8(b)))
        rows.append(row)
    return rows


def rings(w, h):
    rows = []
    cx, cy = w * 0.42, h * 0.46
    for y in range(h):
        row = bytearray()
        for x in range(w):
            d = math.hypot(x - cx, y - cy) / h
            k = 0.5 + 0.5 * math.sin(d * TAU / 0.275)
            fade = 1.0 - 0.55 * max(0.0, (d - 0.48) / 0.33)
            fade = max(0.0, fade)
            r = (40 + 175 * k) * fade
            g = (55 + 130 * (1.0 - k)) * fade
            b = (95 + 150 * (0.5 + 0.5 * math.sin(d * TAU / 0.45 + 1.3))) * fade
            row += bytes((clamp8(r), clamp8(g), clamp8(b)))
        rows.append(row)
    return rows


def checker(w, h):
    palette = [
        (235, 88, 76), (247, 173, 63), (94, 194, 133), (76, 150, 214),
        (150, 111, 214), (232, 232, 240), (44, 52, 66), (214, 96, 160),
    ]
    cols, rws = 8, 6
    cw, ch = w / cols, h / rws
    rows = []
    for y in range(h):
        row = bytearray()
        cy = min(rws - 1, int(y // ch))
        for x in range(w):
            cx = min(cols - 1, int(x // cw))
            base = palette[(cx * 3 + cy * 5) % len(palette)]
            sheen = 0.85 + 0.15 * math.sin(((x / w) + (y / h)) * 7.0)
            row += bytes((clamp8(base[0] * sheen),
                          clamp8(base[1] * sheen),
                          clamp8(base[2] * sheen)))
        rows.append(row)
    return rows


def dunes(w, h):
    # (baseline as a fraction of height, wavelength as a fraction of height, phase)
    ridges = [(0.62, 0.115, 0.9), (0.74, 0.079, 2.1), (0.86, 0.054, 4.0)]
    rows = []
    for y in range(h):
        t = y / h
        row = bytearray()
        for x in range(w):
            u = x / h
            r = 30 + 90 * t
            g = 40 + 70 * t
            b = 80 + 60 * (1.0 - t)
            for index, (base, wave, phase) in enumerate(ridges):
                edge = base + 0.055 * math.sin(u / wave + phase)
                if t > edge:
                    light = 0.55 + 0.45 * math.sin(x / (w * 0.14) + index)
                    r = 150 + 80 * light - index * 20
                    g = 95 + 55 * light - index * 12
                    b = 55 + 40 * light
                    break
            row += bytes((clamp8(r), clamp8(g), clamp8(b)))
        rows.append(row)
    return rows


def orbits(w, h):
    bodies = [(0.30, 0.38, 0.25, (255, 190, 90)),
              (0.68, 0.55, 0.34, (90, 190, 255)),
              (0.46, 0.78, 0.20, (255, 110, 170))]
    rows = []
    for y in range(h):
        row = bytearray()
        for x in range(w):
            r = 12.0 + 14.0 * (y / h)
            g = 14.0 + 10.0 * (x / w)
            b = 34.0 + 26.0 * (y / h)
            for bx, by, radius, color in bodies:
                d = math.hypot(x - bx * w, y - by * h) / h
                falloff = max(0.0, 1.0 - d / radius) ** 2
                r += color[0] * falloff
                g += color[1] * falloff
                b += color[2] * falloff
            # Sparse stars, spaced by a fraction of the area so density is stable.
            if (x * 7 + y * 13) % (w * h // 1200) == 0:
                r += 120
                g += 120
                b += 120
            row += bytes((clamp8(r), clamp8(g), clamp8(b)))
        rows.append(row)
    return rows


def main():
    out_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets")
    os.makedirs(out_dir, exist_ok=True)
    scenes = [("01_aurora", aurora), ("02_rings", rings), ("03_checker", checker),
              ("04_dunes", dunes), ("05_orbits", orbits)]
    for name, fn in scenes:
        path = os.path.join(out_dir, name + ".png")
        write_png(path, W, H, fn(W, H))
        print(f"wrote {path} ({os.path.getsize(path)} bytes)")


if __name__ == "__main__":
    main()
