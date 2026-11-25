"""Would a DAG dedupe nodes whose leaves carry a coverage count? (S5-1 of voxel-engines.md, on synthetic worlds.)

Question: the DAG papers (Kaempe 2013, SSVDAG, TSVDAG, HashDAG, Aokana) merge subtrees of BINARY occupancy.
Cavendish's leaf is a coverage count (0..k^3 samples inside the body) and a material. TSVDAG s2.3 says that putting
attributes in the voxels "significantly reduces" the number of equivalent subtrees. How much, on a known world?

Method (Kaempe's bottom-up merge, one pass): level 0 = cells; a node at level L covers 2^L cells a side; two nodes are
the same when their eight children are the same ids. Count, per level, the MIXED nodes (neither all empty nor all
full, because uniform nodes are free in Cavendish anyway: kind: uniform) and the distinct ones among them.
Content = (a) occupancy, count > 0 (what the papers store); (b) the full coverage count (what Cavendish stores).

Pass criteria, written before running:
  P1 (control, must fail to dedupe): a world of random coverage counts has distinct/total > 0.99 at every level.
  P2 (control, must dedupe): the cell-aligned town has the same distinct/total for (a) and (b), because every count is
     0 or k^3, so (b) carries no more information than (a).
  P3 (the question): the sub-cell-offset town and the terrain have distinct(b)/total much larger than distinct(a)/total
     at the boundary levels. The size of the gap is the result; no threshold is set in advance.
Worlds are synthetic and small (128 x 128 x 64 cells, k = 4 samples a side); they show direction and rough size only.
Run: python dag_dedup.py   (Python 3.14, NumPy; about 20 s)
"""
import sys
import numpy as np

sys.stdout.reconfigure(encoding="utf-8")
rng = np.random.default_rng(7)
NX = NY = 128
NZ = 64
K = 4                       # samples a side per cell; count in 0..K^3 = 0..64


def supersample(inside):
    """inside: bool array on the (K*NX, K*NY, K*NZ) sample grid -> coverage counts per cell, uint8."""
    s = inside.reshape(NX, K, NY, K, NZ, K).sum(axis=(1, 3, 5))
    return s.astype(np.uint8)


def sample_axes():
    x = (np.arange(K * NX) + 0.5) / K
    y = (np.arange(K * NY) + 0.5) / K
    z = (np.arange(K * NZ) + 0.5) / K
    return x, y, z


def town(offsets):
    """Boxes 16.0 x 16.0 cells, 4 height classes, on a 32-cell pitch. offsets=True: each at a random sub-cell shift
    (a fraction of a cell, continuous), so the boundary cells hold partial counts that differ building to building."""
    x, y, z = sample_axes()
    inside = np.zeros((K * NX, K * NY, K * NZ), bool)
    inside[:, :, z < 4.0] = True                                  # ground, cell-aligned
    for bx in range(8, NX - 24, 32):
        for by in range(8, NY - 24, 32):
            hgt = 6.0 + 4.0 * int(rng.integers(0, 4))
            dx, dy = (rng.uniform(0, 1), rng.uniform(0, 1)) if offsets else (0.0, 0.0)
            mx = (x >= bx + dx) & (x < bx + dx + 16.0)
            my = (y >= by + dy) & (y < by + dy + 16.0)
            mz = (z >= 4.0) & (z < 4.0 + hgt)
            inside |= mx[:, None, None] & my[None, :, None] & mz[None, None, :]
    return supersample(inside)


def terrain():
    x, y, z = sample_axes()
    xs, ys = np.meshgrid(x, y, indexing="ij")
    h = np.full(xs.shape, 20.0)
    for _ in range(5):
        f = 2 * np.pi / rng.uniform(30, 90)
        th = rng.uniform(0, 2 * np.pi)
        h += rng.uniform(1.0, 4.0) * np.sin(f * (xs * np.cos(th) + ys * np.sin(th)) + rng.uniform(0, 6.28))
    inside = z[None, None, :] < h[:, :, None]
    return supersample(inside)


def random_world():
    return rng.integers(0, K ** 3 + 1, size=(NX, NY, NZ)).astype(np.uint8)


def levels(cells, full):
    """Yield (level, total_mixed, distinct_mixed) bottom-up. cells: uint8 counts (or 0/1 occupancy with full=1)."""
    ids = cells.astype(np.int64)
    n_ids = int(ids.max()) + 1
    empty = 0
    fullid = full
    level = 0
    while min(ids.shape) > 1:
        level += 1
        a, b, c = (s // 2 for s in ids.shape)
        kids = ids.reshape(a, 2, b, 2, c, 2).transpose(0, 2, 4, 1, 3, 5).reshape(a * b * c, 8)
        uniq, inv = np.unique(kids, axis=0, return_inverse=True)
        inv = inv.reshape(-1)
        is_empty = (uniq == empty).all(axis=1)
        is_full = (uniq == fullid).all(axis=1)
        mixed_u = ~(is_empty | is_full)
        total_mixed = int(mixed_u[inv].sum())
        distinct_mixed = int(mixed_u.sum())
        yield level, total_mixed, distinct_mixed
        # new ids: keep 0 for empty and `fullid` for full so the uniform tests keep working one level up
        new = np.empty(len(uniq), np.int64)
        nxt = max(fullid, 0) + 1
        for i in range(len(uniq)):
            if is_empty[i]:
                new[i] = empty
            elif is_full[i]:
                new[i] = fullid
            else:
                new[i] = nxt
                nxt += 1
        ids = new[inv].reshape(a, b, c)


def report(name, counts):
    occ = (counts > 0).astype(np.uint8)
    # the full-coverage id stays K^3 for (b) and 1 for (a), so uniform-full nodes are recognised in both
    ra = list(levels(occ, 1))
    rb = list(levels(counts, K ** 3))
    print(f"\n{name}   (cells {counts.shape}, share of cells with 0 < count < {K**3}: "
          f"{100 * np.mean((counts > 0) & (counts < K ** 3)):.1f} %)")
    print(f"{'level':>5} | {'mixed a':>8} {'distinct a':>10} {'ratio a':>8} | {'mixed b':>8} {'distinct b':>10} {'ratio b':>8} | distinct b / distinct a")
    out = []
    sa = [0, 0]
    sb = [0, 0]
    for (la, ta, da), (lb, tb, db) in zip(ra, rb):
        if tb == 0 and ta == 0:
            continue
        out.append((la, ta, da, tb, db))
        ra_ = da / ta if ta else float("nan")
        rb_ = db / tb if tb else float("nan")
        sa[0] += ta; sa[1] += da; sb[0] += tb; sb[1] += db
        print(f"{la:>5} | {ta:>8} {da:>10} {ra_:>8.3f} | {tb:>8} {db:>10} {rb_:>8.3f} | {db / da if da else float('nan'):>8.1f}x")
    print(f"{'all':>5} | {sa[0]:>8} {sa[1]:>10} {sa[1] / sa[0]:>8.3f} | {sb[0]:>8} {sb[1]:>10} {sb[1] / sb[0]:>8.3f} | {sb[1] / sa[1]:>8.1f}x")
    return out


def main():
    worlds = {
        "random counts (control P1)": random_world(),
        "town, cell-aligned (control P2)": town(False),
        "town, sub-cell offsets": town(True),
        "terrain (smooth height field)": terrain(),
    }
    res = {k: report(k, v) for k, v in worlds.items()}
    # Checks of the pass criteria
    r = res["random counts (control P1)"]
    p1 = all(db / tb > 0.99 for (_, _, _, tb, db) in r if tb > 1000)
    t = res["town, cell-aligned (control P2)"]
    p2 = all(abs(da / ta - db / tb) < 1e-12 for (_, ta, da, tb, db) in t if ta and tb)
    print(f"\nP1 random counts distinct/total > 0.99 where there are > 1000 nodes: {'PASS' if p1 else 'FAIL'}")
    print(f"P2 aligned town: occupancy and coverage give identical ratios: {'PASS' if p2 else 'FAIL'}")
    # control for the control: P2 must FAIL if coverage differs, i.e. the offset town must not satisfy it
    t2 = res["town, sub-cell offsets"]
    p2c = all(abs(da / ta - db / tb) < 1e-12 for (_, ta, da, tb, db) in t2 if ta and tb)
    print(f"P2 control arm (offset town must NOT give identical ratios): {'PASS' if not p2c else 'FAIL'}")


if __name__ == "__main__":
    main()
