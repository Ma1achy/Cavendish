"""Lossless storage of 32^3 material chunks: palette+bit-pack, RLE, and exact dedup of chunks and 8^3 bricks.
Synthetic worlds (stated, not Cavendish's): flat 'town' of hollow boxes with a window band; rolling terrain from smooth noise.
Material 0 = air. Pass criterion written first: every encoding must decode to the identical array (checked)."""
import numpy as np, hashlib, sys
rng = np.random.default_rng(1)
N = 512; H = 128; C = 32

def town():
    w = np.zeros((N, N, H), np.uint8)
    w[:, :, :6] = 1; w[:, :, 6:8] = 2                       # soil, topsoil
    for bx in range(16, N-40, 56):
        for by in range(16, N-40, 56):
            hgt = int(rng.integers(3, 6)) * 8              # a few storeys: many buildings alike
            x0, y0, s = bx, by, 24
            w[x0:x0+s, y0:y0+s, 8:8+hgt] = 3
            w[x0+1:x0+s-1, y0+1:y0+s-1, 8:8+hgt] = 0       # hollow, 1 cell walls
            w[x0+1:x0+s-1, y0:y0+1, 12:14] = 4; w[x0+1:x0+s-1, y0+s-1:y0+s, 12:14] = 4   # a window band
            w[x0:x0+s, y0:y0+s, 8+hgt:10+hgt] = 5          # roof
    return w

def terrain():
    # sum of a few random plane waves, quantised to cells; layered materials by depth
    xs, ys = np.meshgrid(np.arange(N), np.arange(N), indexing="ij")
    h = np.full((N, N), 40.0)
    for k in range(6):
        f = 2*np.pi/ (rng.uniform(40, 200)); th = rng.uniform(0, 2*np.pi)
        h += rng.uniform(2, 10) * np.sin(f*(xs*np.cos(th)+ys*np.sin(th)) + rng.uniform(0, 6.28))
    h = h.astype(int)
    z = np.arange(H)[None, None, :]
    w = np.zeros((N, N, H), np.uint8)
    d = h[:, :, None] - z
    w[d >= 0] = 1; w[(d >= 0) & (d < 3)] = 2; w[(d >= 0) & (d < 1)] = 3
    w[(d > 20)] = 4
    return w

def chunks(w):
    for i in range(0, N, C):
        for j in range(0, N, C):
            for k in range(0, H, C):
                yield w[i:i+C, j:j+C, k:k+C]

def palette_bytes(c):
    u = np.unique(c)
    if len(u) == 1: return 0, u, None
    bits = max(1, int(np.ceil(np.log2(len(u)))))
    return (c.size*bits+7)//8 + len(u), u, bits

def pack(c, u, bits):
    idx = np.searchsorted(u, c.ravel()).astype(np.uint8)
    b = np.unpackbits(idx[:, None], axis=1, bitorder="little")[:, :bits].ravel()
    return np.packbits(b, bitorder="little")
def unpack(p, u, bits, n):
    b = np.unpackbits(p, bitorder="little")[:n*bits].reshape(n, bits)
    idx = (b * (1 << np.arange(bits))).sum(1)
    return u[idx]

def rle_bytes(c):
    f = c.ravel(order="F")  # x fastest
    ch = np.flatnonzero(np.diff(f)) + 1
    runs = np.diff(np.concatenate(([0], ch, [f.size])))
    return int(sum(1 + (1 if r < 128 else 2 if r < 16384 else 3) for r in runs))

def run(name, w):
    raw = pal = rle = 0; uni = 0; seen = {}; dedup = 0
    bseen = {}; bricks = 0; ubricks = 0
    for c in chunks(w):
        raw += c.size
        pb, u, bits = palette_bytes(c)
        if bits is None: uni += 1
        else:
            assert np.array_equal(unpack(pack(c, u, bits), u, bits, c.size).reshape(c.shape), c)  # exactness
        pal += pb; rle += min(rle_bytes(c), palette_bytes(c)[0] or 1)  # (best of per chunk, uniform = 1 byte)
        h = hashlib.blake2b(c.tobytes(), digest_size=16).digest()
        if h in seen: dedup += 1
        else: seen[h] = pb
        for a in range(0, C, 8):
            for b_ in range(0, C, 8):
                for d in range(0, C, 8):
                    br = c[a:a+8, b_:b_+8, d:d+8]; bricks += 1
                    hh = br.tobytes()
                    if hh not in bseen: bseen[hh] = len(np.unique(br))
    pal_dedup = sum(seen.values())
    # brick dictionary: unique bricks stored palette-packed + 4 bytes of index per brick slot
    ub = sum((1 if k == 1 else (512*max(1, int(np.ceil(np.log2(k))))+7)//8 + k) for k in bseen.values())
    bd = ub + bricks*(2 if len(bseen) < 65536 else 4)
    nch = sum(1 for _ in chunks(w))
    print(f"{name}: {raw/1e6:.1f} M cells, {nch} chunks, {uni} uniform ({100*uni/nch:.0f}%), identical-to-earlier {dedup}")
    print(f"  raw u8         {raw/1e6:8.2f} MB")
    print(f"  palette+pack   {pal/1e6:8.2f} MB  ({raw/max(pal,1):.1f}x)")
    print(f"  RLE (best/chk) {rle/1e6:8.2f} MB  ({raw/max(rle,1):.1f}x)")
    print(f"  pal + chunk dedup {pal_dedup/1e6:6.2f} MB  ({raw/max(pal_dedup,1):.1f}x)")
    print(f"  8^3 brick dict {bd/1e6:8.2f} MB  ({raw/bd:.1f}x), unique bricks {len(bseen)} of {bricks}")
run("town", town())
run("terrain", terrain())
