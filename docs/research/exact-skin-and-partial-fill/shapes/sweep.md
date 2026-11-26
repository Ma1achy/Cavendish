
**sphere** (icosphere of 5120 triangles, vertices on the sphere of radius 1.0 m; size 2 m; signal -293.6 rad at 1.5 m, -69.18 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces | (c★) analytic fill | (d★) analytic + second |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.5 | 8 | 68 | 12548 | 1.9 | 1.1e+01 / 2.9e+00 | 8.5e-01 / 2.4e-02 | 4.0e-02 / 1.4e-04 | 5.7e-02 / 3.7e-04 | 6.8e-13 / 3.7e-13 | 4.2e-02 / 1.5e-04 | 5.7e-02 / 3.7e-04 |
| 8 | 0.25 | 136 | 284 | 24148 | 2.9 | 1.4e+01 / 3.3e+00 | 1.3e-01 / 1.6e-02 | 1.7e-04 / 5.3e-06 | 4.8e-03 / 3.3e-05 | 1.3e-12 / 9.9e-14 | 4.4e-04 / 3.7e-06 | 4.8e-03 / 3.3e-05 |
| 16 | 0.125 | 1568 | 1172 | 57124 | 7.5 | 5.0e+00 / 1.2e+00 | 6.8e-02 / 8.1e-03 | 1.2e-04 / 7.5e-07 | 3.4e-04 / 2.4e-06 | 1.7e-12 / 6.1e-13 | 7.9e-05 / 5.0e-07 | 3.4e-04 / 2.4e-06 |
| 32 | 0.0625 | 14784 | 4772 | 163316 | 24.1 | 2.4e+00 / 5.5e-01 | 5.8e-03 / 6.5e-04 | 7.5e-06 / 6.5e-08 | 2.4e-05 / 1.6e-07 | 6.3e-12 / 8.2e-13 | 9.4e-06 / 7.3e-08 | 2.4e-05 / 1.6e-07 |
| 64 | 0.03125 | 127619 | 19244 | 535076 | 43.0 | 4.5e-01 / 1.1e-01 | 9.2e-04 / 2.4e-05 | 2.0e-07 / 1.1e-09 | 1.5e-06 / 1.0e-08 | 9.5e-12 / 9.4e-13 | 7.5e-07 / 7.0e-09 | 1.5e-06 / 1.0e-08 |
| 128 | 0.01562 | 1057445 | 77012 | 1913012 | 251.6 | 5.2e-02 / 1.3e-02 | 4.8e-04 / 6.2e-05 | 8.9e-08 / 8.9e-10 | 9.6e-08 / 6.7e-10 | 4.0e-12 / 1.2e-12 | 1.8e-07 / 1.5e-09 | 9.6e-08 / 6.7e-10 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.97; (b) sampled fill: 2.23; (c) fill at centroid: 3.10; (d) + second moment: 3.91; (e) clipped pieces: flat at rounding (max 9e-12 rad); (c★) analytic fill: 2.91; (d★) analytic + second: 3.91.

**torus** (64 × 32 quads split into 4096 triangles, vertices on the torus R = 1.0 m, r = 0.4 m; size 2.8 m; signal -212 rad at 1.5 m, -47.38 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces | (c★) analytic fill | (d★) analytic + second |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.7 | 0 | 40 | 9360 | 0.6 | 1.5e+02 / 3.5e+01 | 5.6e+00 / 1.5e-01 | 1.7e+00 / 7.7e-02 | 7.3e-02 / 4.4e-04 | 5.7e-13 / 7.0e-13 | 1.7e+00 / 7.8e-02 | 7.5e-02 / 4.5e-04 |
| 8 | 0.35 | 0 | 200 | 18544 | 1.3 | 2.5e+01 / 4.8e+00 | 4.8e-01 / 2.8e-02 | 1.3e-02 / 7.7e-04 | 8.9e-03 / 5.4e-05 | 5.4e-13 / 2.1e-13 | 1.5e-02 / 8.3e-04 | 9.0e-03 / 5.4e-05 |
| 16 | 0.175 | 256 | 760 | 40240 | 2.9 | 3.7e+00 / 6.7e-01 | 4.3e-02 / 2.6e-03 | 3.5e-03 / 3.2e-04 | 5.9e-04 / 3.8e-06 | 7.1e-13 / 1.9e-13 | 4.0e-03 / 3.3e-04 | 6.0e-04 / 3.8e-06 |
| 32 | 0.0875 | 3280 | 2936 | 106800 | 8.0 | 2.6e+00 / 5.5e-01 | 2.3e-02 / 1.9e-04 | 1.2e-03 / 7.9e-05 | 5.0e-05 / 3.2e-07 | 1.8e-12 / 9.4e-13 | 1.3e-03 / 8.1e-05 | 5.1e-05 / 3.2e-07 |
| 64 | 0.04375 | 31577 | 12184 | 349424 | 68.4 | 5.9e-01 / 1.3e-01 | 8.7e-03 / 2.8e-04 | 8.3e-05 / 5.5e-06 | 3.6e-06 / 2.3e-08 | 2.3e-13 / 7.4e-13 | 8.8e-05 / 5.7e-06 | 3.7e-06 / 2.3e-08 |
| 128 | 0.02187 | 276402 | 48014 | 1213780 | 185.4 | 1.6e-01 / 4.3e-02 | 5.4e-04 / 8.3e-05 | 1.1e-05 / 7.3e-07 | 2.4e-07 / 1.5e-09 | 1.3e-12 / 1.7e-12 | 1.4e-05 / 8.6e-07 | 2.4e-07 / 1.5e-09 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.73; (b) sampled fill: 2.19; (c) fill at centroid: 2.56; (d) + second moment: 3.77; (e) clipped pieces: flat at rounding (max 2e-12 rad); (c★) analytic fill: 2.57; (d★) analytic + second: 3.77.

**teapot** (Utah teapot (Newell 1975) as 6320 triangles: patch seams welded, giving 4 open parts (body open at the top, lid open at its rim, spout and handle open at both ends: 6 holes), each hole capped by a fan to its centroid, the closed parts united with manifold3d (spout and handle pass into the body; the lid sits on the rim); 6646 triangles, the original triangles kept; size 1 m; signal -9.135 rad at 1.5 m, -1.953 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 26 | 12254 | 1.9 | 3.6e+00 / 6.2e-01 | 1.7e-02 / 1.5e-04 | 7.4e-03 / 1.7e-04 | 2.7e-04 / 4.7e-07 | 4.5e-13 / 1.2e-14 |
| 8 | 0.125 | 12 | 110 | 18826 | 2.6 | 2.7e-01 / 5.8e-02 | 4.7e-03 / 7.2e-05 | 4.4e-04 / 8.1e-06 | 2.4e-05 / 3.5e-08 | 1.8e-13 / 2.9e-13 |
| 16 | 0.0625 | 220 | 420 | 35402 | 6.4 | 6.2e-02 / 2.4e-02 | 5.9e-04 / 2.6e-05 | 9.5e-05 / 2.5e-06 | 1.9e-06 / 2.7e-09 | 4.5e-13 / 1.9e-13 |
| 32 | 0.03125 | 2357 | 1749 | 85330 | 11.7 | 1.6e-01 / 3.6e-02 | 1.1e-03 / 5.0e-06 | 1.1e-05 / 3.1e-07 | 1.4e-07 / 2.6e-10 | 2.2e-12 / 5.9e-14 |
| 64 | 0.01562 | 22062 | 7274 | 248274 | 29.1 | 3.5e-03 / 9.8e-04 | 1.7e-04 / 1.0e-05 | 1.0e-06 / 3.1e-08 | 8.8e-09 / 1.1e-11 | 7.2e-13 / 3.4e-13 |
| 128 | 0.007812 | 189474 | 29378 | 816906 | 136.5 | 1.8e-02 / 3.8e-03 | 3.4e-04 / 1.3e-05 | 1.2e-07 / 3.4e-09 | 5.9e-10 / 8.9e-13 | 4.1e-13 / 2.5e-13 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.19; (b) sampled fill: 0.93; (c) fill at centroid: 3.01; (d) + second moment: 3.83; (e) clipped pieces: flat at rounding (max 2e-12 rad).

**suzanne** (Blender's Suzanne as 968 triangles: welded, 4 parts (head with two open eye sockets, two eyes open at the back, one two-sided sheet of 2 triangles dropped), 4 holes capped by fans to their centroids, the head and eyes united (manifold3d); 1136 triangles, original triangles kept; size 1 m; signal -11.98 rad at 1.5 m, -2.236 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 41 | 3964 | 0.5 | 4.6e+00 / 9.1e-01 | 3.3e-02 / 1.9e-03 | 5.6e-04 / 1.4e-05 | 3.5e-04 / 1.3e-06 | 5.5e-13 / 2.2e-13 |
| 8 | 0.125 | 12 | 139 | 8180 | 1.1 | 3.5e-02 / 1.1e-02 | 1.2e-02 / 1.9e-04 | 5.4e-04 / 1.5e-05 | 2.4e-05 / 5.7e-08 | 7.3e-14 / 2.2e-13 |
| 16 | 0.0625 | 222 | 527 | 20052 | 2.8 | 2.3e-01 / 4.0e-02 | 2.2e-03 / 5.1e-05 | 3.8e-05 / 1.4e-06 | 2.4e-06 / 5.2e-09 | 1.5e-13 / 1.3e-13 |
| 32 | 0.03125 | 2437 | 2305 | 67920 | 10.4 | 4.4e-03 / 1.1e-03 | 2.6e-04 / 1.3e-05 | 5.8e-06 / 1.9e-07 | 1.7e-07 / 3.8e-10 | 1.9e-12 / 2.2e-15 |
| 64 | 0.01562 | 23258 | 9507 | 243280 | 40.6 | 3.4e-03 / 9.4e-04 | 3.3e-05 / 1.8e-06 | 7.5e-07 / 2.4e-08 | 1.2e-08 / 2.6e-11 | 2.6e-13 / 5.1e-14 |
| 128 | 0.007812 | 203593 | 38399 | 909996 | 132.3 | 2.0e-03 / 4.2e-04 | 1.6e-04 / 4.7e-06 | 9.0e-08 / 3.0e-09 | 7.8e-10 / 1.8e-12 | 2.5e-13 / 3.0e-13 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.43; (b) sampled fill: 1.85; (c) fill at centroid: 3.08; (d) + second moment: 3.75; (e) clipped pieces: flat at rounding (max 2e-12 rad).

**Facet error** (mesh against analytic shape)

| shape | triangles | phase error at 1.5 m (rad) | at 5 m (rad) | mass error |
|---|---|---|---|---|
| sphere | 80 | +3.72e+01 | +8.77e+00 | -1.27e-01 |
| sphere | 320 | +9.96e+00 | +2.35e+00 | -3.38e-02 |
| sphere | 1280 | +2.53e+00 | +5.97e-01 | -8.61e-03 |
| sphere | 5120 | +6.36e-01 | +1.50e-01 | -2.16e-03 |
| sphere | 20480 | +1.59e-01 | +3.75e-02 | -5.41e-04 |
| sphere | 81920 | +3.98e-02 | +9.38e-03 | -1.35e-04 |
| torus | 256 | +2.71e+01 | +5.91e+00 | -1.23e-01 |
| torus | 1024 | +7.03e+00 | +1.53e+00 | -3.18e-02 |
| torus | 4096 | +1.77e+00 | +3.86e-01 | -8.01e-03 |
| torus | 16384 | +4.44e-01 | +9.68e-02 | -2.01e-03 |
| torus | 65536 | +1.11e-01 | +2.42e-02 | -5.02e-04 |
| torus | 262144 | +2.78e-02 | +6.05e-03 | -1.25e-04 |

**sphere: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces | (c★) analytic fill | (d★) analytic + second |
|---|---|---|---|---|---|---|---|
| 4 | 0.272 µs, 0.000224 MB | 0.646 µs, 0.000532 MB | 0.646 µs, 0.00162 MB | 1.22 µs, 0.00325 MB | 690 µs, 0.452 MB | 0.646 µs, 0.00162 MB | 1.22 µs, 0.00325 MB |
| 8 | 2.38 µs, 0.00196 MB | 3.57 µs, 0.00294 MB | 3.57 µs, 0.00748 MB | 5.98 µs, 0.0143 MB | 1.33e+03 µs, 0.872 MB | 3.57 µs, 0.00748 MB | 5.98 µs, 0.0143 MB |
| 16 | 18.5 µs, 0.0152 MB | 23.3 µs, 0.0192 MB | 23.3 µs, 0.0379 MB | 33.3 µs, 0.0661 MB | 3.16e+03 µs, 2.08 MB | 23.3 µs, 0.0379 MB | 33.3 µs, 0.0661 MB |
| 32 | 147 µs, 0.121 MB | 166 µs, 0.137 MB | 166 µs, 0.213 MB | 207 µs, 0.328 MB | 9.11e+03 µs, 6.02 MB | 166 µs, 0.213 MB | 207 µs, 0.328 MB |
| 64 | 1.16e+03 µs, 0.957 MB | 1.25e+03 µs, 1.03 MB | 1.25e+03 µs, 1.34 MB | 1.41e+03 µs, 1.8 MB | 3.05e+04 µs, 20.3 MB | 1.25e+03 µs, 1.34 MB | 1.41e+03 µs, 1.8 MB |
| 128 | 9.32e+03 µs, 7.67 MB | 9.64e+03 µs, 7.94 MB | 9.64e+03 µs, 9.17 MB | 1.03e+04 µs, 11 MB | 1.14e+05 µs, 76.8 MB | 9.66e+03 µs, 9.19 MB | 1.03e+04 µs, 11 MB |

**torus: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces | (c★) analytic fill | (d★) analytic + second |
|---|---|---|---|---|---|---|---|
| 4 | 0.136 µs, 0.000112 MB | 0.34 µs, 0.00028 MB | 0.34 µs, 0.00092 MB | 0.68 µs, 0.00188 MB | 515 µs, 0.337 MB | 0.34 µs, 0.00092 MB | 0.68 µs, 0.00188 MB |
| 8 | 0.68 µs, 0.00056 MB | 1.7 µs, 0.0014 MB | 1.7 µs, 0.0046 MB | 3.4 µs, 0.0094 MB | 1.02e+03 µs, 0.669 MB | 1.7 µs, 0.0046 MB | 3.4 µs, 0.0094 MB |
| 16 | 5.03 µs, 0.00414 MB | 8.64 µs, 0.00711 MB | 8.64 µs, 0.0193 MB | 15.1 µs, 0.0375 MB | 2.22e+03 µs, 1.46 MB | 8.64 µs, 0.0193 MB | 15.1 µs, 0.0375 MB |
| 32 | 40.2 µs, 0.0331 MB | 52.8 µs, 0.0435 MB | 52.8 µs, 0.0905 MB | 77.8 µs, 0.161 MB | 5.9e+03 µs, 3.89 MB | 53.8 µs, 0.091 MB | 78.6 µs, 0.161 MB |
| 64 | 319 µs, 0.263 MB | 372 µs, 0.306 MB | 372 µs, 0.501 MB | 476 µs, 0.794 MB | 1.95e+04 µs, 12.9 MB | 375 µs, 0.503 MB | 479 µs, 0.795 MB |
| 128 | 2.54e+03 µs, 2.09 MB | 2.76e+03 µs, 2.27 MB | 2.76e+03 µs, 3.04 MB | 3.17e+03 µs, 4.19 MB | 6.91e+04 µs, 46 MB | 2.78e+03 µs, 3.06 MB | 3.19e+03 µs, 4.21 MB |

**teapot: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.068 µs, 5.6e-05 MB | 0.221 µs, 0.000182 MB | 0.221 µs, 0.000598 MB | 0.442 µs, 0.00122 MB | 674 µs, 0.441 MB |
| 8 | 0.408 µs, 0.000336 MB | 1.04 µs, 0.000854 MB | 1.04 µs, 0.00261 MB | 1.97 µs, 0.00525 MB | 1.04e+03 µs, 0.679 MB |
| 16 | 3.43 µs, 0.00282 MB | 5.44 µs, 0.00448 MB | 5.44 µs, 0.0112 MB | 9.01 µs, 0.0213 MB | 1.95e+03 µs, 1.28 MB |
| 32 | 27.5 µs, 0.0227 MB | 34.9 µs, 0.0287 MB | 34.9 µs, 0.0567 MB | 49.8 µs, 0.0987 MB | 4.71e+03 µs, 3.1 MB |
| 64 | 216 µs, 0.178 MB | 249 µs, 0.205 MB | 249 µs, 0.322 MB | 311 µs, 0.496 MB | 1.38e+04 µs, 9.14 MB |
| 128 | 1.73e+03 µs, 1.42 MB | 1.86e+03 µs, 1.53 MB | 1.86e+03 µs, 2 MB | 2.11e+03 µs, 2.71 MB | 4.65e+04 µs, 30.9 MB |

**suzanne: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.034 µs, 2.8e-05 MB | 0.349 µs, 0.000287 MB | 0.349 µs, 0.000943 MB | 0.697 µs, 0.00193 MB | 218 µs, 0.143 MB |
| 8 | 0.459 µs, 0.000378 MB | 1.28 µs, 0.00106 MB | 1.28 µs, 0.00328 MB | 2.46 µs, 0.00662 MB | 450 µs, 0.296 MB |
| 16 | 3.76 µs, 0.00309 MB | 6.37 µs, 0.00524 MB | 6.37 µs, 0.0137 MB | 10.8 µs, 0.0263 MB | 1.1e+03 µs, 0.727 MB |
| 32 | 29.6 µs, 0.0243 MB | 40.3 µs, 0.0332 MB | 40.3 µs, 0.0701 MB | 59.9 µs, 0.125 MB | 3.76e+03 µs, 2.48 MB |
| 64 | 236 µs, 0.195 MB | 279 µs, 0.229 MB | 279 µs, 0.381 MB | 359 µs, 0.61 MB | 1.36e+04 µs, 8.99 MB |
| 128 | 1.89e+03 µs, 1.56 MB | 2.06e+03 µs, 1.69 MB | 2.06e+03 µs, 2.31 MB | 2.38e+03 µs, 3.23 MB | 5.18e+04 µs, 34.5 MB |

**Cheapest cost per evaluation point (µs, model) to bring the 1.5 m phase error below 1e-4 / 1e-6 / 1e-8 rad** (log–log interpolation between pitches; — never within size/h ≤ 128)

| technique | sphere | torus | teapot | suzanne |
|---|---|---|---|---|
| (a) cubes | — / — / — | — / — / — | — / — / — | — / — / — |
| (b) sampled fill | — / — / — | — / — / — | — / — / — | 100 / — / — |
| (c) fill at centroid | 26.9 / 510 / — | 324 / — / — | 5.14 / 259 / — | 3.56 / 211 / — |
| (d) + second moment | 77 / 1.89e+03 / — | 48.9 / 1.16e+03 / — | 0.808 / 13.7 / 286 | 1.26 / 19.2 / 407 |
| (e) clipped pieces | 690 / 690 / 690 | 515 / 515 / 515 | 674 / 674 / 674 | 218 / 218 / 218 |
| (c★) analytic fill | 18.1 / 996 / — | 343 / — / — | n/a | n/a |
| (d★) analytic + second | 77 / 1.89e+03 / — | 49.9 / 1.18e+03 / — | n/a | n/a |
| whole mesh, direct | 282 (exact) | 225 (exact) | 366 (exact) | 62.5 (exact) |

Facet error against triangle count (fitted slope of log|error| against log triangles): sphere: phase -0.99, mass -0.99; torus: phase -0.99, mass -0.99 (−1 per triangle count is order 2 in the edge length).
