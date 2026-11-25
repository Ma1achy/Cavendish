"""Far-field rounding of the Werner-Scheeres face form: which sub-expression sets the error.

Pure Python (f64). Cube of side s, axis-aligned, centre at a non-dyadic point; field points along
a fixed non-axis direction at R/s = 10 .. 3e5. Reference: monopole + l=4 series (the l=6 term is
(s/R)^6 relative, below 1e-12 for R/s >= 100; rows below that compare against the series' own
truncation). Variants:
  A  triple = r1.(r2 x r3) (as skin_bench.py), L = log1p
  B  triple = h * |(v2-v1) x (v3-v1)| (edge vectors from the vertices), L = log1p
  C  as B but L = log((a+b+l)/(a+b-l)) (the naive ratio)
  E  as A with the cube at the origin and the field point rounded to integers (the selftest's setup)
"""

import math


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def cube_tris(c, s):
    hs = s / 2
    out = []
    for ax in range(3):
        for sg in (-1.0, 1.0):
            u, v = (ax + 1) % 3, (ax + 2) % 3
            q = []
            for p, w in [(-1, -1), (1, -1), (1, 1), (-1, 1)]:
                x = [0.0, 0.0, 0.0]
                x[ax], x[u], x[v] = sg * hs, p * hs, w * hs
                q.append(tuple(x[i] + c[i] for i in range(3)))
            if cross(sub(q[1], q[0]), sub(q[2], q[0]))[ax] * sg < 0:
                q = q[::-1]
            out += [(q[0], q[1], q[2]), (q[0], q[2], q[3])]
    return out


def field(tris, p, triple_from_edges, naive_log):
    u = 0.0
    g = [0.0, 0.0, 0.0]
    for t in tris:
        nn = cross(sub(t[1], t[0]), sub(t[2], t[0]))
        area2 = math.sqrt(dot(nn, nn))
        n = tuple(x / area2 for x in nn)
        r = [sub(x, p) for x in t]
        ln = [math.sqrt(dot(x, x)) for x in r]
        h = dot(n, r[0])
        trip = h * area2 if triple_from_edges else dot(r[0], cross(r[1], r[2]))
        den = ln[0] * ln[1] * ln[2] + ln[0] * dot(r[1], r[2]) + ln[1] * dot(r[2], r[0]) + ln[2] * dot(r[0], r[1])
        om = 2.0 * math.atan2(trip, den)
        s = 0.0
        for i in range(3):
            j = (i + 1) % 3
            e = sub(t[j], t[i])
            el = math.sqrt(dot(e, e))
            ne = tuple(x / el for x in cross(e, n))
            ab = ln[i] + ln[j]
            big = math.log((ab + el) / (ab - el)) if naive_log else math.log1p(2.0 * el / (ab - el))
            s += dot(ne, r[i]) * big
        br = s - h * om
        u += 0.5 * h * br
        for k in range(3):
            g[k] += -br * n[k]
    return u, g


def series(c, s, p):
    d = sub(p, c)
    rr = math.sqrt(dot(d, d))
    cc = s**3 * (s / 2) ** 4 * (-7.0 / 60.0)
    s4 = sum(x**4 for x in d)
    u = s**3 / rr + cc * (5 * s4 / rr**9 - 3 / rr**5)
    g = [-(s**3) * x / rr**3 + cc * (20 * x**3 / rr**9 - 45 * s4 * x / rr**11 + 15 * x / rr**7) for x in d]
    return u, g


def main() -> None:
    s = 1 / 64
    d0 = (0.613, 0.291, -0.734)
    nd = math.sqrt(dot(d0, d0))
    direction = tuple(x / nd for x in d0)
    odd = (0.1234567, -0.3141593, 0.2718282)
    cases = {"A": (odd, False, False), "B": (odd, True, False), "C": (odd, True, True), "E": ((0.0, 0.0, 0.0), False, False)}
    print("R/s       " + "".join(f"{k}:U rel    {k}:g rel    " for k in cases))
    for k in range(1, 6):
        for m in (1.0, 3.0):
            ratio = m * 10**k
            row = f"{ratio:8.0e}  "
            for name, (c, tfe, nl) in cases.items():
                p = tuple(c[i] + direction[i] * ratio * s for i in range(3))
                if name == "E":
                    p = tuple(float(round(x)) for x in p)
                    if dot(p, p) == 0:
                        row += f"{'-':>9}    {'-':>9}    "
                        continue
                u, g = field(cube_tris(c, s), p, tfe, nl)
                us, gs = series(c, s, p)
                dg = sub(g, gs)
                row += f"{abs(u / us - 1):9.1e}    {math.sqrt(dot(dg, dg) / dot(gs, gs)):9.1e}    "
            print(row)


if __name__ == "__main__":
    main()
