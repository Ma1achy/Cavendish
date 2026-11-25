"""Edge logarithm near its edge: L = ln((a+b+l)/(a+b-l)) for a point at distance d from the
middle of an edge of length l = 1/64 m, placed at a non-dyadic position, edge along a skew
direction. Reference: exact L = 2 asinh((l/2)/d) for the midpoint. Compares
  naive : log1p(2l/(a+b-l)) with a+b-l computed directly
  hk    : a+b-l = r0^2/(a - l1) + r0^2/(b + l2) (Holstein & Ketteridge's r-|l| = r0^2/(r+|l|))
Prints the relative error of L, and of d*L (what g carries)."""

import math


def main() -> None:
    l = 1 / 64
    e = (0.48, -0.6, 0.64)
    ne = math.sqrt(sum(x * x for x in e))
    e = tuple(x / ne for x in e)
    perp = (0.8, 0.6, 0.0)  # orthogonal to e: 0.384 - 0.36 + 0 = 0.024 -> fix below
    dp = sum(p * q for p, q in zip(perp, e))
    perp = tuple(p - dp * q for p, q in zip(perp, e))
    npp = math.sqrt(sum(x * x for x in perp))
    perp = tuple(x / npp for x in perp)
    v1 = (0.1234567, -0.3141593, 0.2718282)
    v2 = tuple(v1[i] + l * e[i] for i in range(3))
    mid = tuple(0.5 * (v1[i] + v2[i]) for i in range(3))
    print("d          L exact      naive rel    hk rel")
    for k in range(2, 15, 2):
        d = 10.0**-k
        p = tuple(mid[i] + d * perp[i] for i in range(3))
        r1 = tuple(v1[i] - p[i] for i in range(3))
        r2 = tuple(v2[i] - p[i] for i in range(3))
        a = math.sqrt(sum(x * x for x in r1))
        b = math.sqrt(sum(x * x for x in r2))
        el = math.sqrt(sum((v2[i] - v1[i]) ** 2 for i in range(3)))
        ehat = tuple((v2[i] - v1[i]) / el for i in range(3))
        exact = 2 * math.asinh((el / 2) / d)
        naive_gap = a + b - el
        naive = math.log1p(2 * el / naive_gap) if naive_gap > 0 else math.inf  # inf: division by zero
        l1 = sum(r1[i] * ehat[i] for i in range(3))
        l2 = sum(r2[i] * ehat[i] for i in range(3))
        c = (r1[1] * ehat[2] - r1[2] * ehat[1], r1[2] * ehat[0] - r1[0] * ehat[2], r1[0] * ehat[1] - r1[1] * ehat[0])
        r0sq = sum(x * x for x in c)
        if l1 <= 0 <= l2:
            gap = r0sq / (a - l1) + r0sq / (b + l2)
        else:
            gap = a + b - el
        hk = math.log1p(2 * el / gap)
        print(f"{d:8.0e}  {exact:10.4f}  {abs(naive / exact - 1):10.1e}  {abs(hk / exact - 1):10.1e}")


if __name__ == "__main__":
    main()
