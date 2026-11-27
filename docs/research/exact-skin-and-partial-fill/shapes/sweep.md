
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

**bunny** (Stanford Bunny (Turk & Levoy 1994, Stanford 3D Scanning Repository, bun_zipper.ply): welded, 1 part, 5 holes capped by ear clipping; 69664 triangles, the original triangles kept; size 1 m; signal -17.35 rad at 1.5 m, -3.97 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 1 | 43 | 91164 | 4.8 | 2.0e+00 / 3.4e-01 | 1.7e-01 / 8.0e-03 | 3.9e-04 / 1.0e-04 | 2.2e-04 / 5.9e-07 | 9.7e-13 / 9.3e-13 |
| 8 | 0.125 | 28 | 195 | 117024 | 5.9 | 8.6e-01 / 2.4e-01 | 4.5e-02 / 2.1e-03 | 8.3e-05 / 1.5e-05 | 3.4e-05 / 4.8e-08 | 6.6e-13 / 6.9e-13 |
| 16 | 0.0625 | 470 | 836 | 175832 | 9.4 | 2.2e-01 / 5.6e-02 | 1.4e-02 / 3.7e-04 | 4.5e-05 / 3.1e-06 | 3.4e-06 / 5.2e-09 | 8.9e-13 / 6.0e-14 |
| 32 | 0.03125 | 4939 | 3465 | 323780 | 17.9 | 2.6e-02 / 1.6e-02 | 1.3e-03 / 5.2e-05 | 2.2e-06 / 3.4e-07 | 2.3e-07 / 3.6e-10 | 4.3e-12 / 8.1e-14 |
| 64 | 0.01562 | 45637 | 14180 | 739600 | 58.7 | 5.3e-03 / 4.6e-03 | 3.5e-04 / 1.7e-05 | 3.3e-07 / 3.6e-08 | 1.6e-08 / 2.9e-11 | 1.2e-12 / 3.0e-13 |
| 128 | 0.007812 | 391420 | 56987 | 2040788 | 127.8 | 3.1e-04 / 2.8e-04 | 2.5e-06 / 2.8e-06 | 4.0e-08 / 4.1e-09 | 1.1e-09 / 1.9e-12 | 1.6e-12 / 7.6e-14 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 2.82; (b) sampled fill: 3.36; (c) fill at centroid: 2.91; (d) + second moment: 3.76; (e) clipped pieces: flat at rounding (max 4e-12 rad).

**spot** (Spot (Keenan Crane, CC0 1.0, spot_triangulated.obj): welded, 1 part, 0 holes capped; 5856 triangles, the original triangles kept; size 1 m; signal -13.83 rad at 1.5 m, -2.966 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 32 | 10792 | 0.6 | 2.0e+00 / 8.7e-01 | 4.1e-02 / 2.3e-03 | 1.5e-02 / 3.7e-04 | 4.0e-04 / 1.2e-06 | 5.2e-13 / 1.7e-13 |
| 8 | 0.125 | 16 | 152 | 19344 | 1.0 | 1.3e-01 / 1.2e-01 | 3.3e-02 / 1.1e-03 | 1.4e-03 / 4.1e-05 | 3.6e-05 / 1.2e-07 | 3.1e-13 / 1.2e-13 |
| 16 | 0.0625 | 298 | 668 | 41204 | 2.3 | 2.0e-01 / 4.0e-02 | 2.3e-03 / 8.4e-05 | 1.3e-04 / 3.2e-06 | 2.3e-06 / 3.5e-09 | 2.5e-13 / 9.9e-14 |
| 32 | 0.03125 | 3368 | 2779 | 109104 | 6.2 | 7.6e-02 / 1.1e-02 | 9.1e-04 / 2.5e-05 | 1.1e-05 / 3.1e-07 | 2.0e-07 / 3.2e-10 | 1.7e-12 / 1.4e-13 |
| 64 | 0.01562 | 31786 | 11242 | 337420 | 28.8 | 1.8e-02 / 4.1e-03 | 1.6e-04 / 9.3e-06 | 1.5e-06 / 4.5e-08 | 1.4e-08 / 2.4e-11 | 6.4e-13 / 2.9e-13 |
| 128 | 0.007812 | 275051 | 44973 | 1163700 | 92.0 | 3.1e-03 / 4.4e-04 | 3.4e-05 / 1.7e-07 | 2.1e-07 / 6.1e-09 | 9.1e-10 / 1.5e-12 | 1.6e-13 / 1.7e-13 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.43; (b) sampled fill: 2.37; (c) fill at centroid: 3.19; (d) + second moment: 3.79; (e) clipped pieces: flat at rounding (max 2e-12 rad).

**dragon** (Stanford Dragon (Curless & Levoy 1996, Stanford 3D Scanning Repository, dragon_vrip.ply): decimated (quadric error, self-crossings cut out and closed) from 871414 to 49138 triangles, welded, 1 part, 0 holes capped; 49138 triangles, the decimated triangles kept; size 1 m; signal -5.13 rad at 1.5 m, -1.123 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 30 | 68010 | 6.6 | 1.7e+00 / 4.8e-01 | 1.2e-02 / 1.9e-03 | 3.8e-03 / 1.5e-04 | 7.0e-05 / 5.4e-07 | 2.8e-13 / 6.3e-13 |
| 8 | 0.125 | 0 | 130 | 88554 | 8.3 | 5.6e-01 / 1.7e-01 | 3.9e-02 / 2.1e-03 | 2.7e-04 / 6.6e-06 | 1.1e-05 / 3.7e-08 | 4.2e-13 / 4.2e-13 |
| 16 | 0.0625 | 30 | 569 | 131634 | 13.4 | 2.9e-01 / 7.0e-02 | 1.1e-03 / 6.8e-05 | 2.6e-05 / 6.8e-07 | 4.9e-07 / 1.0e-09 | 3.1e-13 / 2.7e-13 |
| 32 | 0.03125 | 794 | 2466 | 242758 | 26.2 | 1.3e-02 / 5.3e-04 | 1.1e-03 / 5.6e-05 | 4.8e-06 / 9.3e-08 | 6.9e-08 / 1.0e-10 | 8.3e-13 / 4.8e-13 |
| 64 | 0.01562 | 9883 | 10261 | 552958 | 49.1 | 2.3e-02 / 5.6e-03 | 6.5e-06 / 9.2e-06 | 8.9e-07 / 1.7e-08 | 5.6e-09 / 8.1e-12 | 5.3e-13 / 3.8e-15 |
| 128 | 0.007812 | 96327 | 41751 | 1523478 | 112.0 | 8.7e-04 / 4.6e-04 | 6.8e-05 / 5.1e-06 | 1.1e-07 / 2.0e-09 | 3.9e-10 / 3.9e-13 | 1.2e-13 / 8.5e-14 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 2.23; (b) sampled fill: 2.58; (c) fill at centroid: 2.75; (d) + second moment: 3.59; (e) clipped pieces: flat at rounding (max 8e-13 rad).

**armadillo** (Armadillo (Krishnamurthy & Levoy 1996, Stanford 3D Scanning Repository, Armadillo.ply): decimated (quadric error, self-crossings cut out and closed) from 345944 to 59996 triangles, welded, 1 part, 0 holes capped; 59996 triangles, the decimated triangles kept; size 1 m; signal -7.936 rad at 1.5 m, -1.452 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 43 | 78688 | 3.8 | 7.3e+00 / 1.3e+00 | 1.2e-03 / 1.0e-03 | 3.6e-03 / 9.3e-05 | 2.9e-04 / 1.8e-06 | 2.6e-13 / 5.8e-14 |
| 8 | 0.125 | 2 | 153 | 101192 | 5.1 | 4.5e-01 / 8.3e-02 | 1.5e-03 / 1.5e-04 | 1.1e-04 / 4.9e-06 | 1.6e-05 / 3.6e-08 | 2.0e-13 / 3.0e-14 |
| 16 | 0.06251 | 78 | 584 | 147404 | 7.8 | 1.3e-02 / 9.8e-03 | 4.6e-03 / 1.8e-04 | 7.3e-05 / 1.9e-06 | 9.9e-07 / 2.5e-09 | 3.4e-13 / 8.0e-14 |
| 32 | 0.03125 | 1217 | 2437 | 266776 | 14.8 | 3.4e-02 / 6.9e-03 | 9.1e-04 / 3.5e-05 | 5.6e-06 / 1.5e-07 | 9.1e-08 / 1.9e-10 | 1.1e-12 / 9.7e-14 |
| 64 | 0.01563 | 13408 | 9898 | 585636 | 32.5 | 9.4e-03 / 2.5e-03 | 9.4e-04 / 4.6e-05 | 7.8e-07 / 1.8e-08 | 6.2e-09 / 1.5e-11 | 0.0e+00 / 1.3e-13 |
| 128 | 0.007814 | 124555 | 40314 | 1564772 | 88.7 | 1.7e-03 / 1.5e-04 | 5.0e-05 / 3.3e-07 | 8.4e-08 / 2.1e-09 | 4.2e-10 / 1.2e-12 | 8.5e-13 / 2.4e-14 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.67; (b) sampled fill: 1.21; (c) fill at centroid: 2.72; (d) + second moment: 3.77; (e) clipped pieces: flat at rounding (max 1e-12 rad).

**buddha** (Happy Buddha (Curless & Levoy 1996, Stanford 3D Scanning Repository, happy_vrip.ply): decimated (quadric error, self-crossings cut out and closed) from 1087716 to 47974 triangles, welded, 1 part, 0 holes capped; 47974 triangles, the decimated triangles kept; size 1 m; signal -4.191 rad at 1.5 m, -0.9477 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 17 | 59662 | 2.8 | 8.8e-01 / 2.8e-01 | 2.2e-01 / 5.6e-03 | 8.1e-03 / 1.7e-04 | 1.8e-04 / 1.1e-06 | 2.9e-13 / 5.4e-13 |
| 8 | 0.125 | 0 | 89 | 78610 | 4.0 | 1.2e+00 / 3.4e-01 | 2.2e-02 / 8.4e-04 | 6.8e-04 / 1.2e-05 | 1.8e-06 / 2.4e-08 | 5.9e-13 / 9.6e-14 |
| 16 | 0.06251 | 44 | 419 | 117894 | 6.1 | 6.2e-03 / 2.5e-02 | 1.2e-02 / 5.7e-04 | 4.9e-05 / 7.0e-08 | 2.8e-08 / 1.4e-09 | 8.6e-13 / 6.1e-14 |
| 32 | 0.03126 | 735 | 1792 | 206530 | 11.4 | 5.8e-02 / 1.5e-02 | 1.2e-03 / 1.7e-05 | 7.1e-06 / 4.9e-08 | 9.2e-08 / 3.2e-10 | 3.0e-13 / 2.6e-15 |
| 64 | 0.01563 | 8381 | 7756 | 459186 | 25.3 | 2.3e-02 / 5.0e-03 | 1.4e-03 / 4.5e-05 | 9.6e-07 / 3.2e-09 | 3.4e-09 / 5.2e-12 | 5.4e-14 / 1.2e-13 |
| 128 | 0.007814 | 80087 | 32552 | 1247146 | 70.6 | 2.9e-04 / 2.0e-04 | 1.0e-04 / 2.6e-06 | 1.3e-07 / 9.1e-10 | 2.3e-10 / 4.0e-13 | 3.5e-14 / 1.1e-13 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 2.20; (b) sampled fill: 1.87; (c) fill at centroid: 3.03; (d) + second moment: 2.90; (e) clipped pieces: flat at rounding (max 9e-13 rad).

**bracket** (mk7 ball bearing extruder drive by jag (Thingiverse 13727, Thingi10K 44950, public domain): welded, 1 part, 0 holes capped; 5280 triangles, the original triangles kept; size 1 m; signal -6.955 rad at 1.5 m, -1.399 rad at 5 m)

| size/h | h (m) | interior | boundary | piece triangles | clip time (s) | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|---|---|---|---|---|
| 4 | 0.25 | 0 | 26 | 10856 | 0.6 | 3.1e+00 / 7.9e-01 | 4.6e-02 / 9.1e-03 | 1.1e-02 / 3.7e-04 | 1.4e-04 / 8.9e-07 | 1.2e-12 / 1.9e-13 |
| 8 | 0.125 | 4 | 130 | 20779 | 1.1 | 1.6e+00 / 3.0e-01 | 8.4e-03 / 8.2e-04 | 1.2e-03 / 3.0e-05 | 2.2e-05 / 8.5e-08 | 5.2e-14 / 1.2e-13 |
| 16 | 0.0625 | 76 | 534 | 42688 | 2.1 | 3.7e-02 / 1.0e-02 | 2.1e-02 / 1.7e-03 | 2.2e-04 / 5.2e-06 | 4.1e-07 / 7.4e-10 | 5.4e-13 / 5.3e-13 |
| 32 | 0.03125 | 1263 | 2245 | 106240 | 5.6 | 3.7e-01 / 7.4e-02 | 8.4e-03 / 6.7e-04 | 6.2e-06 / 6.4e-08 | 9.6e-09 / 6.0e-11 | 6.8e-13 / 3.2e-13 |
| 64 | 0.01562 | 14055 | 9190 | 310916 | 17.4 | 1.5e-02 / 1.5e-03 | 3.8e-03 / 4.3e-04 | 1.5e-06 / 2.5e-08 | 1.1e-09 / 2.9e-12 | 3.2e-14 / 2.6e-13 |
| 128 | 0.007812 | 127342 | 37374 | 1034892 | 60.0 | 8.6e-03 / 1.1e-03 | 2.7e-03 / 2.2e-04 | 1.5e-07 / 2.9e-09 | 7.3e-11 / 1.8e-14 | 6.5e-13 / 4.2e-13 |

Fitted order in h (phase at 1.5 m, size/h ≥ 8): (a) cubes: 1.63; (b) sampled fill: 0.58; (c) fill at centroid: 3.29; (d) + second moment: 4.49; (e) clipped pieces: flat at rounding (max 1e-12 rad).

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

Facet error against triangle count (fitted slope of log|error| against log triangles): sphere: phase -0.99, mass -0.99; torus: phase -0.99, mass -0.99 (−1 per triangle count is order 2 in the edge length).

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

**bunny: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.102 µs, 8.4e-05 MB | 0.374 µs, 0.000308 MB | 0.374 µs, 0.000996 MB | 0.74 µs, 0.00203 MB | 5.01e+03 µs, 3.28 MB |
| 8 | 0.926 µs, 0.000763 MB | 1.9 µs, 0.00156 MB | 1.9 µs, 0.00468 MB | 3.55 µs, 0.00936 MB | 6.44e+03 µs, 4.21 MB |
| 16 | 7.06 µs, 0.00582 MB | 11.1 µs, 0.00914 MB | 11.1 µs, 0.0225 MB | 18.2 µs, 0.0426 MB | 9.67e+03 µs, 6.34 MB |
| 32 | 56 µs, 0.0461 MB | 71.4 µs, 0.0588 MB | 71.4 µs, 0.114 MB | 101 µs, 0.197 MB | 1.78e+04 µs, 11.7 MB |
| 64 | 445 µs, 0.367 MB | 508 µs, 0.419 MB | 508 µs, 0.646 MB | 629 µs, 0.986 MB | 4.11e+04 µs, 27 MB |
| 128 | 3.57e+03 µs, 2.94 MB | 3.81e+03 µs, 3.14 MB | 3.81e+03 µs, 4.05 MB | 4.3e+03 µs, 5.42 MB | 1.16e+05 µs, 76.6 MB |

**spot: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.102 µs, 8.4e-05 MB | 0.272 µs, 0.000224 MB | 0.272 µs, 0.000736 MB | 0.544 µs, 0.0015 MB | 594 µs, 0.389 MB |
| 8 | 0.646 µs, 0.000532 MB | 1.43 µs, 0.00118 MB | 1.43 µs, 0.00361 MB | 2.72 µs, 0.00726 MB | 1.06e+03 µs, 0.698 MB |
| 16 | 5 µs, 0.00412 MB | 8.21 µs, 0.00676 MB | 8.21 µs, 0.0175 MB | 13.9 µs, 0.0335 MB | 2.27e+03 µs, 1.49 MB |
| 32 | 39.3 µs, 0.0324 MB | 52.2 µs, 0.043 MB | 52.2 µs, 0.0875 MB | 75.9 µs, 0.154 MB | 6.03e+03 µs, 3.97 MB |
| 64 | 315 µs, 0.26 MB | 366 µs, 0.301 MB | 366 µs, 0.481 MB | 461 µs, 0.751 MB | 1.88e+04 µs, 12.4 MB |
| 128 | 2.53e+03 µs, 2.08 MB | 2.72e+03 µs, 2.24 MB | 2.72e+03 µs, 2.96 MB | 3.1e+03 µs, 4.04 MB | 6.63e+04 µs, 44.1 MB |

**dragon: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.017 µs, 1.4e-05 MB | 0.255 µs, 0.00021 MB | 0.255 µs, 0.00069 MB | 0.51 µs, 0.00141 MB | 3.74e+03 µs, 2.45 MB |
| 8 | 0.281 µs, 0.000231 MB | 1.1 µs, 0.00091 MB | 1.1 µs, 0.00299 MB | 2.21 µs, 0.00611 MB | 4.87e+03 µs, 3.19 MB |
| 16 | 1.81 µs, 0.00149 MB | 5.09 µs, 0.00419 MB | 5.09 µs, 0.0133 MB | 9.93 µs, 0.027 MB | 7.24e+03 µs, 4.74 MB |
| 32 | 15.5 µs, 0.0128 MB | 27.7 µs, 0.0228 MB | 27.7 µs, 0.0623 MB | 48.7 µs, 0.121 MB | 1.34e+04 µs, 8.76 MB |
| 64 | 123 µs, 0.101 MB | 171 µs, 0.141 MB | 171 µs, 0.305 MB | 258 µs, 0.551 MB | 3.05e+04 µs, 20 MB |
| 128 | 991 µs, 0.816 MB | 1.17e+03 µs, 0.967 MB | 1.17e+03 µs, 1.63 MB | 1.53e+03 µs, 2.64 MB | 8.46e+04 µs, 55.8 MB |

**armadillo: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.068 µs, 5.6e-05 MB | 0.365 µs, 0.000301 MB | 0.365 µs, 0.000989 MB | 0.731 µs, 0.00202 MB | 4.33e+03 µs, 2.83 MB |
| 8 | 0.281 µs, 0.000231 MB | 1.32 µs, 0.00109 MB | 1.32 µs, 0.00353 MB | 2.62 µs, 0.0072 MB | 5.57e+03 µs, 3.64 MB |
| 16 | 2.41 µs, 0.00198 MB | 5.63 µs, 0.00463 MB | 5.63 µs, 0.014 MB | 10.6 µs, 0.028 MB | 8.11e+03 µs, 5.31 MB |
| 32 | 19.2 µs, 0.0158 MB | 31.1 µs, 0.0256 MB | 31.1 µs, 0.0646 MB | 51.8 µs, 0.123 MB | 1.47e+04 µs, 9.63 MB |
| 64 | 153 µs, 0.126 MB | 198 µs, 0.163 MB | 198 µs, 0.322 MB | 282 µs, 0.559 MB | 3.23e+04 µs, 21.2 MB |
| 128 | 1.22e+03 µs, 1.01 MB | 1.4e+03 µs, 1.15 MB | 1.4e+03 µs, 1.8 MB | 1.74e+03 µs, 2.77 MB | 8.71e+04 µs, 57.5 MB |

**buddha: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.034 µs, 2.8e-05 MB | 0.144 µs, 0.000119 MB | 0.144 µs, 0.000391 MB | 0.289 µs, 0.000799 MB | 3.28e+03 µs, 2.15 MB |
| 8 | 0.272 µs, 0.000224 MB | 0.757 µs, 0.000623 MB | 0.757 µs, 0.00205 MB | 1.51 µs, 0.00418 MB | 4.32e+03 µs, 2.83 MB |
| 16 | 1.54 µs, 0.00127 MB | 3.94 µs, 0.00324 MB | 3.94 µs, 0.00994 MB | 7.5 µs, 0.02 MB | 6.48e+03 µs, 4.25 MB |
| 32 | 12.9 µs, 0.0106 MB | 21.5 µs, 0.0177 MB | 21.5 µs, 0.0464 MB | 36.7 µs, 0.0894 MB | 1.14e+04 µs, 7.45 MB |
| 64 | 102 µs, 0.084 MB | 137 µs, 0.113 MB | 137 µs, 0.237 MB | 203 µs, 0.423 MB | 2.53e+04 µs, 16.6 MB |
| 128 | 812 µs, 0.669 MB | 957 µs, 0.788 MB | 957 µs, 1.31 MB | 1.23e+03 µs, 2.09 MB | 6.93e+04 µs, 45.7 MB |

**bracket: cost per evaluation point (µs, model) and memory (MB, model)**

| size/h | (a) cubes | (b) sampled fill | (c) fill at centroid | (d) + second moment | (e) clipped pieces |
|---|---|---|---|---|---|
| 4 | 0.0595 µs, 4.9e-05 MB | 0.221 µs, 0.000182 MB | 0.221 µs, 0.000598 MB | 0.442 µs, 0.00122 MB | 597 µs, 0.391 MB |
| 8 | 0.238 µs, 0.000196 MB | 1.14 µs, 0.000938 MB | 1.14 µs, 0.00302 MB | 2.24 µs, 0.00614 MB | 1.14e+03 µs, 0.749 MB |
| 16 | 2.38 µs, 0.00196 MB | 5.19 µs, 0.00427 MB | 5.19 µs, 0.0128 MB | 9.72 µs, 0.0256 MB | 2.35e+03 µs, 1.54 MB |
| 32 | 20 µs, 0.0165 MB | 29.8 µs, 0.0246 MB | 29.8 µs, 0.0605 MB | 48.9 µs, 0.114 MB | 5.85e+03 µs, 3.85 MB |
| 64 | 152 µs, 0.125 MB | 198 µs, 0.163 MB | 198 µs, 0.31 MB | 276 µs, 0.53 MB | 1.72e+04 µs, 11.4 MB |
| 128 | 1.22e+03 µs, 1 MB | 1.4e+03 µs, 1.15 MB | 1.4e+03 µs, 1.75 MB | 1.72e+03 µs, 2.65 MB | 5.8e+04 µs, 38.4 MB |

**Cheapest cost per evaluation point (µs, model) to bring the 1.5 m phase error below 1e-4 / 1e-6 / 1e-8 rad** (log–log interpolation between pitches; — never within size/h ≤ 128)

| technique | sphere | torus | teapot | suzanne | bunny | spot | dragon | armadillo | buddha | bracket |
|---|---|---|---|---|---|---|---|---|---|---|
| (a) cubes | — / — / — | — / — / — | — / — / — | — / — / — | — / — / — | — / — / — | — / — / — | — / — / — | — / — / — | — / — / — |
| (b) sampled fill | — / — / — | — / — / — | — / — / — | 100 / — / — | 844 / — / — | 673 / — / — | 65.4 / — / — | 883 / — / — | — / — / — | — / — / — |
| (c) fill at centroid | 26.9 / 510 / — | 324 / — / — | 5.14 / 259 / — | 3.56 / 211 / — | 1.56 / 160 / — | 9.88 / 555 / — | 2.11 / 151 / — | 1.81 / 157 / — | 2.52 / 133 / — | 7.58 / 280 / — |
| (d) + second moment | 77 / 1.89e+03 / — | 48.9 / 1.16e+03 / — | 0.808 / 13.7 / 286 | 1.26 / 19.2 / 407 | 1.43 / 39.9 / 893 | 1.37 / 25 / 582 | 0.51 / 7 / 175 | 1.16 / 10.6 / 208 | 0.36 / 1.91 / 116 | 0.587 / 7 / 48 |
| (e) clipped pieces | 690 / 690 / 690 | 515 / 515 / 515 | 674 / 674 / 674 | 218 / 218 / 218 | 5.01e+03 / 5.01e+03 / 5.01e+03 | 594 / 594 / 594 | 3.74e+03 / 3.74e+03 / 3.74e+03 | 4.33e+03 / 4.33e+03 / 4.33e+03 | 3.28e+03 / 3.28e+03 / 3.28e+03 | 597 / 597 / 597 |
| (c★) analytic fill | 18.1 / 996 / — | 343 / — / — | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| (d★) analytic + second | 77 / 1.89e+03 / — | 49.9 / 1.18e+03 / — | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| whole mesh, direct | 282 (exact) | 225 (exact) | 366 (exact) | 62.5 (exact) | 3.83e+03 (exact) | 322 (exact) | 2.7e+03 (exact) | 3.3e+03 (exact) | 2.64e+03 (exact) | 290 (exact) |
