#!/usr/bin/env python3
"""Generate the 32x32 2-bit "Icons32" set as ASCII-art sources.

Each icon is described with a few vector primitives in a 32x32 unit space,
rasterized with 4x4 supersampling, and quantized to 4 coverage levels:

    '.' = 0 (transparent)   '-' = 1 (33%)   '+' = 2 (66%)   '#' = 3 (100% ink)

Output: v2/glyphs/icons32/NN_name.txt (one icon per file). The .txt files are the
source of truth consumed by v2/build.rs; this script is only a starting point and
the generated art may be hand-edited afterwards (re-running overwrites edits).

Usage: python3 tools/gen_icons.py
"""
import math
import os

N = 32
SS = 4
W = 2.6  # default stroke width


# ---------------------------------------------------------------- primitives
def seg_dist(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    l2 = dx * dx + dy * dy
    t = 0.0 if l2 == 0 else max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / l2))
    qx, qy = ax + t * dx, ay + t * dy
    return math.hypot(px - qx, py - qy)


def line(*pts, w=W):
    segs = list(zip(pts[0::2], pts[1::2]))
    def f(x, y):
        for (a, b) in zip(segs, segs[1:]):
            if seg_dist(x, y, a[0], a[1], b[0], b[1]) <= w / 2:
                return True
        return False
    return f


def poly(*pts):
    p = list(zip(pts[0::2], pts[1::2]))
    def f(x, y):
        inside = False
        j = len(p) - 1
        for i in range(len(p)):
            xi, yi = p[i]
            xj, yj = p[j]
            if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
                inside = not inside
            j = i
        return inside
    return f


def outline(*pts, w=W):
    return line(*(list(pts) + list(pts[:2])), w=w)


def disc(cx, cy, r):
    return lambda x, y: math.hypot(x - cx, y - cy) <= r


def ring(cx, cy, r, w=W, a0=None, a1=None):
    """Circle stroke; optional arc from a0..a1 degrees (0 = east, CCW, y up)."""
    def f(x, y):
        d = math.hypot(x - cx, y - cy)
        if abs(d - r) > w / 2:
            return False
        if a0 is None:
            return True
        a = math.degrees(math.atan2(-(y - cy), x - cx)) % 360
        lo, hi = a0 % 360, a1 % 360
        return lo <= a <= hi if lo <= hi else (a >= lo or a <= hi)
    return f


def rect(x0, y0, x1, y1, r=0.0):
    def f(x, y):
        if not (x0 <= x <= x1 and y0 <= y <= y1):
            return False
        if r <= 0:
            return True
        cx = min(max(x, x0 + r), x1 - r)
        cy = min(max(y, y0 + r), y1 - r)
        return math.hypot(x - cx, y - cy) <= r
    return f


def frame(x0, y0, x1, y1, r=0.0, w=W):
    outer = rect(x0, y0, x1, y1, r)
    inner = rect(x0 + w, y0 + w, x1 - w, y1 - w, max(0.0, r - w))
    return lambda x, y: outer(x, y) and not inner(x, y)


def minus(a, b):
    return lambda x, y: a(x, y) and not b(x, y)


# ---------------------------------------------------------------- icon set
def gear():
    def f(x, y):
        dx, dy = x - 16, y - 16
        d = math.hypot(dx, dy)
        a = math.atan2(dy, dx)
        tooth = 0.5 + 0.5 * math.cos(8 * a)
        r_out = 10.5 + (3.0 if tooth > 0.55 else 0.0)
        return 4.2 <= d <= r_out
    return [f]


ICONS = [
    ("home", [poly(16, 4, 3, 15, 6, 15, 6, 28, 13, 28, 13, 20, 19, 20, 19, 28, 26, 28, 26, 15, 29, 15)]),
    ("back", [line(27, 16, 6, 16), line(15, 7, 6, 16, 15, 25)]),
    ("forward", [line(5, 16, 26, 16), line(17, 7, 26, 16, 17, 25)]),
    ("up", [line(16, 27, 16, 6), line(7, 15, 16, 6, 25, 15)]),
    ("close", [line(7, 7, 25, 25, w=3.0), line(25, 7, 7, 25, w=3.0)]),
    ("check", [line(5, 17, 12, 24, 27, 8, w=3.2)]),
    ("menu", [line(5, 8, 27, 8, w=3.0), line(5, 16, 27, 16, w=3.0), line(5, 24, 27, 24, w=3.0)]),
    ("more", [disc(7, 16, 3), disc(16, 16, 3), disc(25, 16, 3)]),
    ("plus", [line(16, 5, 16, 27, w=3.2), line(5, 16, 27, 16, w=3.2)]),
    ("minus", [line(5, 16, 27, 16, w=3.2)]),
    ("search", [ring(13, 13, 8, w=3.0), line(19, 19, 27, 27, w=3.6)]),
    ("refresh", [ring(16, 16, 10, w=2.8, a0=110, a1=400), poly(22, 1, 29, 7, 20, 11)]),
    ("settings", gear()),
    ("trash", [line(5, 8, 27, 8), line(12, 8, 12, 4, 20, 4, 20, 8),
               outline(8, 8, 10, 28, 22, 28, 24, 8), line(13, 13, 13, 23, w=2.0),
               line(19, 13, 19, 23, w=2.0)]),
    ("edit", [poly(21, 4, 28, 11, 12, 27, 5, 27, 5, 20), ]),
    ("save", [minus(rect(4, 4, 28, 28, 2),
                    lambda x, y: (7 <= x <= 25 and 17 <= y <= 25)      # label
                    or (9 <= x <= 22 and 4 <= y <= 12 and not (17 <= x <= 20 and 6 <= y <= 10)))]),
    ("files", [frame(9, 3, 27, 23, 1.5, w=2.2), rect(4, 8, 22, 29, 1.5), rect(6, 10, 20, 27)]),
    ("folder", [poly(3, 7, 12, 7, 15, 10, 29, 10, 29, 27, 3, 27)]),
    ("document", [outline(7, 3, 19, 3, 25, 9, 25, 29, 7, 29), line(19, 3, 19, 9, 25, 9),
                  line(11, 15, 21, 15, w=2.0), line(11, 20, 21, 20, w=2.0), line(11, 25, 18, 25, w=2.0)]),
    ("image", [frame(3, 5, 29, 27, 2), disc(11, 12, 3), poly(5, 25, 13, 16, 18, 21, 22, 17, 27, 25)]),
    ("calculator", [frame(6, 3, 26, 29, 2.5), rect(10, 7, 22, 12, 0.5)]),
    ("games", [minus(rect(2, 9, 30, 25, 7),
                     lambda x, y: (abs(x - 10) <= 1.3 and 12.5 <= y <= 21.5)
                     or (abs(y - 17) <= 1.3 and 5.5 <= x <= 14.5)
                     or math.hypot(x - 21, y - 15) <= 2.0
                     or math.hypot(x - 25, y - 19) <= 2.0)]),
    ("system", [frame(3, 5, 29, 22, 1.5), line(16, 22, 16, 27), line(9, 27, 23, 27, w=2.8)]),
    ("clock", [ring(16, 16, 12, w=2.8), line(16, 8, 16, 16, 22, 19, w=2.6)]),
    ("play", [poly(9, 5, 26, 16, 9, 27)]),
    ("pause", [rect(7, 6, 13, 26, 1), rect(19, 6, 25, 26, 1)]),
    ("stop", [rect(7, 7, 25, 25, 2)]),
    ("speaker", [poly(4, 12, 10, 12, 17, 5, 17, 27, 10, 20, 4, 20),
                 ring(17, 16, 6, w=2.4, a0=-50, a1=50), ring(17, 16, 11, w=2.4, a0=-50, a1=50)]),
    ("mute", [poly(4, 12, 10, 12, 17, 5, 17, 27, 10, 20, 4, 20),
              line(21, 12, 29, 20, w=2.6), line(29, 12, 21, 20, w=2.6)]),
    ("info", [minus(disc(16, 16, 13), lambda x, y: math.hypot(x - 16, y - 9.5) <= 2.2
                    or (abs(x - 16) <= 1.6 and 14 <= y <= 24))]),
    ("warning", [minus(poly(16, 3, 30, 28, 2, 28),
                       lambda x, y: (abs(x - 16) <= 1.6 and 11 <= y <= 20)
                       or math.hypot(x - 16, y - 24) <= 1.9)]),
    ("lock", [rect(6, 14, 26, 29, 2), ring(16, 14, 6.5, w=3.0, a0=0, a1=180),
              line(9.5, 14, 9.5, 12), line(22.5, 14, 22.5, 12)]),
]


def calc_keys(x, y):
    # 3x3 key grid inside the calculator body (rows y=15..25)
    for r in range(3):
        for c in range(3):
            kx, ky = 10 + c * 4.5, 15 + r * 4.0
            if kx <= x <= kx + 3.0 and ky <= y <= ky + 2.5:
                return True
    return False


# Calculator: add the key grid (defined after ICONS for readability)
for i, (name, shapes) in enumerate(ICONS):
    if name == "calculator":
        ICONS[i] = (name, shapes + [calc_keys])


def render(shapes):
    rows = []
    for py in range(N):
        row = []
        for px in range(N):
            hit = 0
            for sy in range(SS):
                for sx in range(SS):
                    x = px + (sx + 0.5) / SS
                    y = py + (sy + 0.5) / SS
                    if any(s(x, y) for s in shapes):
                        hit += 1
            level = int(round(3 * hit / (SS * SS)))
            row.append(".-+#"[level])
        rows.append("".join(row))
    return rows


def main():
    root = os.path.join(os.path.dirname(__file__), "..", "v2", "glyphs", "icons32")
    os.makedirs(root, exist_ok=True)
    for f in os.listdir(root):
        if f.endswith(".txt"):
            os.remove(os.path.join(root, f))
    for i, (name, shapes) in enumerate(ICONS):
        rows = render(shapes)
        path = os.path.join(root, f"{i:02d}_{name}.txt")
        with open(path, "w") as fh:
            fh.write(f"# Icons32 #{i} {name} (U+{0xE000 + i:04X})\n")
            fh.write("# 32x32, 2 bits per pixel: '.'=0 '-'=1 '+'=2 '#'=3\n")
            fh.write("\n".join(rows) + "\n")
    print(f"wrote {len(ICONS)} icons to {os.path.normpath(root)}")


if __name__ == "__main__":
    main()
