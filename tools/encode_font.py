#!/usr/bin/env python3
"""Encode FiveSeven with the fixed-width orbit/fold model (no entropy coding).

Column word (8 bits):
    bits 0..5  orbit id (36 shapes under invert + reverse)
    bit  6     polarity  (0 = prototype, 1 = inverted)
    bit  7     direction (0 = top-to-bottom, 1 = bottom-to-top)

Glyph record:
    2 bits mode, payload, 2 bits dy

    mode 0 raw       5 column words
    mode 1 self      1 bit fold (0=left-right, 1=180) + 3 column words
    mode 2 reference 10-bit glyph index + 2-bit fold (same/lr/tb/180)
    mode 3 template  9-bit 3x3 quadrant + 2-bit row extrusion

Usage: python3 tools/encode_font.py [--dump out.bin]
"""
import argparse
import glob
import os
import sys

ROOT = os.path.join(os.path.dirname(__file__), "..")
GLYPH_DIR = os.path.join(ROOT, "v2", "glyphs", "fiveseven")

MAGIC = 0x46
VERSION = 2


def rev7(v):
    return int(format(v, "07b")[::-1], 2)


def parse():
    glyphs = []
    for path in sorted(glob.glob(os.path.join(GLYPH_DIR, "*.txt"))):
        lines = open(path, encoding="utf-8").read().splitlines()
        i = 0
        while i < len(lines):
            line = lines[i]
            if not line.startswith("@U+"):
                i += 1
                continue
            cp = int(line[3:].split()[0], 16)
            dy = int(line.split("dy=")[1].split()[0]) if "dy=" in line else 0
            rows = lines[i + 1 : i + 8]
            cols = []
            for c in range(5):
                v = 0
                for r in range(7):
                    if rows[r][c] == "#":
                        v |= 1 << (6 - r)
                cols.append(v)
            glyphs.append({"cp": cp, "dy": dy, "rows": rows, "cols": cols})
            i += 8
    glyphs.sort(key=lambda g: g["cp"])
    return glyphs


class BitWriter:
    def __init__(self):
        self.bits = []

    def put(self, value, n):
        for i in range(n - 1, -1, -1):
            self.bits.append((value >> i) & 1)

    def nbits(self):
        return len(self.bits)

    def as_bytes(self):
        out = bytearray()
        for i in range(0, len(self.bits), 8):
            chunk = self.bits[i : i + 8]
            byte = 0
            for b in chunk:
                byte = (byte << 1) | b
            byte <<= 8 - len(chunk)
            out.append(byte)
        return bytes(out)


class BitReader:
    def __init__(self, data):
        self.data = data
        self.pos = 0

    def get(self, n):
        v = 0
        for _ in range(n):
            byte = self.data[self.pos // 8]
            bit = (byte >> (7 - (self.pos % 8))) & 1
            v = (v << 1) | bit
            self.pos += 1
        return v


# ------------------------------------------------------------- column orbits

def orbit_of(v):
    return {v, rev7(v), v ^ 0x7F, rev7(v) ^ 0x7F}


PROTOS = sorted({min(orbit_of(v)) for v in range(128)})


def column_word(v):
    """Smallest (orbit, polarity, direction) tuple that decodes to v."""
    for i, p in enumerate(PROTOS):
        for pol in (0, 1):
            for d in (0, 1):
                t = rev7(p) if d else p
                if pol:
                    t ^= 0x7F
                if t == v:
                    return i | (pol << 6) | (d << 7)
    raise ValueError(v)


def column_from_word(w):
    p = PROTOS[w & 0x3F]
    if w & 0x80:
        p = rev7(p)
    if w & 0x40:
        p ^= 0x7F
    return p


# --------------------------------------------------------------- glyph folds

def fold(cols, kind):
    if kind == 0:
        return list(cols)
    if kind == 1:
        return list(reversed(cols))
    if kind == 2:
        return [rev7(c) for c in cols]
    if kind == 3:
        return [rev7(c) for c in reversed(cols)]
    raise ValueError(kind)


def rot90x5(m):
    return ["".join(m[4 - r][c] for r in range(5)) for c in range(5)]


def rot90x3(q):
    return ["".join(q[2 - c][r] for c in range(3)) for r in range(3)]


def template_of(rows):
    """Return (9-bit quadrant, 2-bit extrusion) or None."""
    core = rows[1:6]
    if rot90x5(core) != core:
        return None
    q = [row[0:3] for row in core[0:3]]
    r1 = rot90x3(q)
    r2 = rot90x3(r1)
    r3 = rot90x3(r2)
    grid = [["."] * 5 for _ in range(5)]

    def place(t, r0, c0):
        for r in range(3):
            for c in range(3):
                grid[r0 + r][c0 + c] = t[r][c]

    place(q, 0, 0)
    place(r1, 0, 2)
    place(r2, 2, 2)
    place(r3, 2, 0)
    if ["".join(row) for row in grid] != core:
        return None
    top = 1 if rows[0] == rows[1] else (0 if rows[0] == "....." else None)
    bot = 1 if rows[6] == rows[5] else (0 if rows[6] == "....." else None)
    if top is None or bot is None:
        return None
    bits = 0
    for r in range(3):
        for c in range(3):
            if q[r][c] == "#":
                bits |= 1 << (8 - (r * 3 + c))
    return bits, top | (bot << 1)


def template_rows(qbits, extrude):
    q = [[("." if not (qbits & (1 << (8 - (r * 3 + c)))) else "#") for c in range(3)] for r in range(3)]
    q = ["".join(row) for row in q]
    r1 = rot90x3(q)
    r2 = rot90x3(r1)
    r3 = rot90x3(r2)
    grid = [["."] * 5 for _ in range(5)]

    def place(t, r0, c0):
        for r in range(3):
            for c in range(3):
                grid[r0 + r][c0 + c] = t[r][c]

    place(q, 0, 0)
    place(r1, 0, 2)
    place(r2, 2, 2)
    place(r3, 2, 0)
    core = ["".join(row) for row in grid]
    top = core[0] if extrude & 1 else "....."
    bot = core[4] if extrude & 2 else "....."
    return [top] + core + [bot]


# ---------------------------------------------------------------------- main

MODE_RAW, MODE_SELF, MODE_REF, MODE_TPL = range(4)
MODE_NAMES = ["raw", "self", "ref", "template"]
FOLD_NAMES = ["same", "lr", "tb", "180"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dump", metavar="PATH")
    args = ap.parse_args()

    glyphs = parse()
    n = len(glyphs)
    print(f"glyphs: {n}")
    print(f"orbits: {len(PROTOS)}")

    choice = []
    counts = {k: 0 for k in MODE_NAMES}
    tpl_count = 0
    for i, g in enumerate(glyphs):
        c = g["cols"]
        candidates = []

        candidates.append((2 + 5 * 8 + 2, MODE_RAW, None))

        if c == list(reversed(c)):
            candidates.append((2 + 1 + 3 * 8 + 2, MODE_SELF, 0))
        if all(c[k] == rev7(c[4 - k]) for k in range(5)):
            candidates.append((2 + 1 + 3 * 8 + 2, MODE_SELF, 1))

        for j in range(i):
            prev = glyphs[j]["cols"]
            for kind in (0, 1, 2, 3):
                if fold(prev, kind) == c:
                    candidates.append((2 + 10 + 2 + 2, MODE_REF, (j, kind)))
                    break

        tpl = template_of(g["rows"])
        if tpl is not None:
            candidates.append((2 + 9 + 2 + 2, MODE_TPL, tpl))

        # deterministic: cheapest, then template, ref, self, raw
        order = {MODE_TPL: 0, MODE_REF: 1, MODE_SELF: 2, MODE_RAW: 3}
        candidates.sort(key=lambda x: (x[0], order[x[1]]))
        cost, mode, payload = candidates[0]
        choice.append((cost, mode, payload))
        counts[MODE_NAMES[mode]] += 1
        if mode == MODE_TPL:
            tpl_count += 1

    print("modes:", counts)
    print(f"template glyphs: {tpl_count}")

    w = BitWriter()
    w.put(MAGIC, 8)
    w.put(VERSION, 8)
    w.put(n, 16)
    w.put(len(PROTOS), 8)
    for p in PROTOS:
        w.put(p, 7)
    for i, (_, mode, payload) in enumerate(choice):
        g = glyphs[i]
        w.put(mode, 2)
        if mode == MODE_RAW:
            for col in g["cols"]:
                w.put(column_word(col), 8)
        elif mode == MODE_SELF:
            w.put(payload, 1)
            for col in g["cols"][:3]:
                w.put(column_word(col), 8)
        elif mode == MODE_REF:
            j, kind = payload
            w.put(j, 10)
            w.put(kind, 2)
        else:
            qbits, extrude = payload
            w.put(qbits, 9)
            w.put(extrude, 2)
        w.put(g["dy"], 2)
    data = w.as_bytes()

    header_bits = 8 + 8 + 16 + 8 + 7 * len(PROTOS)
    total_bits = w.nbits()
    print(f"\nheader+orbits: {header_bits} bits ({header_bits / 8:.1f} B)")
    print(f"glyph records:  {total_bits - header_bits} bits ({(total_bits - header_bits) / 8:.1f} B)")
    print(f"TOTAL:          {total_bits} bits ({total_bits / 8:.1f} B, {len(data)} B serialised)")
    print(f"packed table:   {n * 5} B  -> delta {len(data) - n * 5:+d} B")

    # ------------------------------------------------------------------ verify
    r = BitReader(data)
    assert r.get(8) == MAGIC
    assert r.get(8) == VERSION
    cnt = r.get(16)
    no = r.get(8)
    assert cnt == n and no == len(PROTOS)
    protos = [r.get(7) for _ in range(no)]

    def cword(wv):
        p = protos[wv & 0x3F]
        if wv & 0x80:
            p = rev7(p)
        if wv & 0x40:
            p ^= 0x7F
        return p

    decoded = []
    for i in range(n):
        mode = r.get(2)
        if mode == MODE_RAW:
            cols = [cword(r.get(8)) for _ in range(5)]
            rows = None
        elif mode == MODE_SELF:
            kind = r.get(1)
            head = [cword(r.get(8)) for _ in range(3)]
            if kind == 0:
                cols = [head[0], head[1], head[2], head[1], head[0]]
            else:
                cols = [head[0], head[1], head[2], rev7(head[1]), rev7(head[0])]
            rows = None
        elif mode == MODE_REF:
            j = r.get(10)
            kind = r.get(2)
            d = r.get(2)
            decoded.append([fold(decoded[j][0], kind), d, None])
            continue
        else:
            qbits = r.get(9)
            extrude = r.get(2)
            rows = template_rows(qbits, extrude)
            cols = None
        d = r.get(2)
        decoded.append([cols, d, rows])

    bad = 0
    for i, g in enumerate(glyphs):
        cols, d, rows = decoded[i]
        if rows is None:
            if cols != g["cols"]:
                bad += 1
                if bad <= 3:
                    print(f"MISMATCH U+{g['cp']:04X} cols {cols} != {g['cols']}")
        else:
            if rows != g["rows"]:
                bad += 1
                if bad <= 3:
                    print(f"MISMATCH U+{g['cp']:04X} rows {rows} != {g['rows']}")
        if d != g["dy"]:
            bad += 1
    print(f"verify: {n - bad}/{n} glyphs match" + ("  OK" if bad == 0 else "  FAILED"))

    if args.dump:
        with open(args.dump, "wb") as fh:
            fh.write(data)
        print(f"wrote {args.dump}")

    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
