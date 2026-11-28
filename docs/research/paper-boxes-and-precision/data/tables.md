# Tables (written by analyse.py)

## A. Counts at 128 cells across, and growth with N (fitted exponent over 16-128)

| shape | interior cells | boundary cells | octree blocks | octree, merged | runs merged (sweep) | Darnell growth | largest first | exponent: cells / boundary / octree / Darnell |
|---|---|---|---|---|---|---|---|---|
| sphere | 1057445 | 77012 | 47457 | 12773 | 5663 | 11011 | 10733 | 3.13 / 2.01 / 2.13 / 2.25 |
| torus | 276402 | 48014 | 30436 | 7399 | 3888 | 5874 | 6038 | 3.35 / 2.00 / 2.30 / 2.45 |
| teapot | 189474 | 29378 | 17176 | 4232 | 2697 | 3302 | 3322 | 3.25 / 2.04 / 2.38 / 2.32 |
| Suzanne | 203593 | 38399 | 22363 | 5613 | 2868 | 4620 | 4473 | 3.28 / 2.06 / 2.48 / 2.48 |
| bunny | 391420 | 56987 | 34378 | 8352 | 5490 | 6935 | 6833 | 3.23 / 2.03 / 2.37 / 2.32 |
| Spot | 275051 | 44973 | 27510 | 5898 | 3003 | 4731 | 4721 | 3.28 / 2.02 / 2.40 / 2.43 |
| dragon | 96327 | 41751 | 21903 | 6163 | 4244 | 5076 | 5002 | 3.86 / 2.06 / 3.26 / 2.85 |
| Armadillo | 124555 | 40314 | 21200 | 5975 | 4159 | 4837 | 4722 | 3.54 / 2.03 / 2.78 / 2.62 |
| Buddha | 80087 | 32552 | 16107 | 3787 | 2736 | 3151 | 3048 | 3.60 / 2.10 / 2.83 / 2.57 |
| mk7 part | 127342 | 37374 | 18877 | 669 | 527 | 475 | 475 | 3.56 / 2.04 / 2.90 / 2.04 |

## Boxes (largest first) to cover 90 % / 99 % of the interior volume, 128 across

| shape | octree blocks | octree, merged | runs merged (sweep) | Darnell growth | largest first |
|---|---|---|---|---|---|
| sphere | 2677 / 36883 | 698 / 7068 | 3214 / 5098 | 367 / 5265 | 517 / 6219 |
| torus | 6176 / 27672 | 1303 / 5576 | 2609 / 3576 | 602 / 3942 | 875 / 4419 |
| teapot | 2969 / 15282 | 533 / 2980 | 1709 / 2333 | 234 / 2027 | 341 / 2261 |
| Suzanne | 4250 / 20328 | 915 / 4246 | 1323 / 2332 | 486 / 3161 | 652 / 3289 |
| bunny | 5836 / 30464 | 1018 / 5811 | 2942 / 4651 | 555 / 4279 | 725 / 4700 |
| Spot | 5387 / 24760 | 878 / 4173 | 1405 / 2540 | 449 / 2940 | 620 / 3293 |
| dragon | 12271 / 20940 | 2146 / 5203 | 2628 / 3778 | 1340 / 4113 | 1709 / 4187 |
| Armadillo | 8745 / 19955 | 1602 / 4910 | 2374 / 3613 | 948 / 3718 | 1234 / 3808 |
| Buddha | 8099 / 15307 | 1050 / 3117 | 1425 / 2318 | 637 / 2425 | 775 / 2440 |
| mk7 part | 6143 / 17604 | 134 / 394 | 227 / 386 | 61 / 243 | 55 / 243 |

## A1, A2: partition and rounding

- A1 every decomposition partitions the interior: **True**; control (largest box dropped) fails the partition check in 49 of 49 cases.
- A2 boxes against cells within max(1e-11 rad, 1e-14 of the interior phase) at every pitch and distance: **False**. Largest |difference| (rad): octree blocks 2.8e-11, octree, merged 3.3e-11, runs merged (sweep) 3.3e-11, Darnell growth 3.2e-11, largest first 2.8e-11. Control at 1.5 m: smallest 1.0e-01 rad (criterion > 1e-6).
- The five box decompositions agree with one another to 1.6e-11 rad everywhere; their common difference from the cell-by-cell sum is largest for torus at 128 across, 0.1 m (3.1e-11 rad).
- A3 (d) at 1.5 m, 128 across, against the shapes study: largest difference 2.2e-12 rad.

## A5: the octree read with a series switch, extra error at 1.5 m (rad), 128 across

| shape | octree: strict (0.01) / 0.05 / S3 (1/6) | free boxes (sweep): strict / 0.05 / S3 | interior cost at 1.5 m (us), octree: strict / 0.05 / S3 | free boxes: strict / 0.05 / S3 |
|---|---|---|---|---|
| sphere | 1.2e-12 / 5.5e-08 / 3.5e-05 | 8.9e-12 / 3.3e-07 / 2.0e-03 | 2.01e+03 / 430 / 403 | 3.94e+03 / 3.79e+03 / 2.57e+03 |
| torus | 3.4e-12 / 7.7e-08 / 7.7e-07 | 3.7e-12 / 2.7e-06 / 7.6e-03 | 1.88e+03 / 266 / 259 | 2.7e+03 / 2.46e+03 / 1.16e+03 |
| teapot | 6.8e-13 / 2.2e-09 / 2.2e-09 | 3.9e-11 / 6.6e-07 / 2.5e-03 | 233 / 146 / 146 | 1.74e+03 / 1.45e+03 / 102 |
| Suzanne | 6.0e-13 / 1.7e-09 / 1.7e-09 | 1.4e-10 / 2.0e-06 / 3.2e-03 | 284 / 190 / 190 | 1.72e+03 / 1.03e+03 / 56.2 |
| bunny | 3.6e-13 / 5.4e-09 / 5.4e-09 | 1.1e-10 / 2.3e-06 / 1.1e-03 | 486 / 292 / 292 | 3.49e+03 / 2.39e+03 / 977 |
| Spot | 6.3e-13 / 1.7e-09 / 1.7e-09 | 5.1e-11 / 3.0e-06 / 1.4e-03 | 372 / 234 / 234 | 1.97e+03 / 1.02e+03 / 51.1 |
| dragon | 3.8e-13 / 1.6e-11 / 1.6e-11 | 1.8e-10 / 4.4e-06 / 1.9e-04 | 216 / 186 / 186 | 2.48e+03 / 688 / 72.1 |
| Armadillo | 4.8e-14 / 3.7e-10 / 3.7e-10 | 1.3e-10 / 3.5e-06 / 4.5e-04 | 223 / 180 / 180 | 2.43e+03 / 832 / 78.2 |
| Buddha | 2.4e-13 / 4.6e-10 / 4.6e-10 | 7.3e-11 / 2.1e-06 / 9.6e-05 | 153 / 137 / 137 | 1.47e+03 / 673 / 46.5 |
| mk7 part | 6.9e-13 / 3.2e-10 / 3.2e-10 | 8.8e-12 / 5.3e-08 / 1.3e-03 | 204 / 160 / 160 | 333 / 225 / 26.6 |

## Error (rad) and model cost per point (us) at 0.1 m from the surface, 128 across

| shape | signal | whole mesh | cells (paper's model) + (d) | octree strict + (d) | octree S3 + (d) | free boxes + (d) | free boxes + (e) | (d) error | (e) error |
|---|---|---|---|---|---|---|---|---|---|
| sphere | -9.96e+02 | 282 | 1.03e+04 | 1.03e+04 | 1.73e+03 (err 5.3e-04) | 5.25e+03 | 1.09e+05 | 9.4e-06 | 2.5e-12 |
| torus | -1.36e+03 | 225 | 3.17e+03 | 8.91e+03 | 1.09e+03 (err 7.1e-04) | 3.52e+03 | 6.95e+04 | 2.5e-04 | 3.1e-11 |
| teapot | -9.87e+01 | 366 | 2.11e+03 | 5.58e+03 | 646 (err 1.4e-04) | 2.35e+03 | 4.68e+04 | 1.3e-06 | 1.1e-13 |
| Suzanne | -3.28e+02 | 62.5 | 2.38e+03 | 8.57e+03 | 846 (err 4.8e-05) | 2.64e+03 | 5.2e+04 | 3.7e-06 | 1.9e-12 |
| bunny | -1.40e+02 | 3.83e+03 | 4.3e+03 | 8.38e+03 | 1.27e+03 (err 1.8e-05) | 4.76e+03 | 1.16e+05 | 1.6e-05 | 2.9e-12 |
| Spot | 2.34e+01 | 322 | 3.1e+03 | 8.92e+03 | 1e+03 (err 9.8e-05) | 2.85e+03 | 6.61e+04 | 3.0e-06 | 3.6e-12 |
| dragon | 5.37e+00 | 2.7e+03 | 1.53e+03 | 7.23e+03 | 897 (err 4.0e-05) | 3.63e+03 | 8.67e+04 | 6.2e-06 | 1.5e-12 |
| Armadillo | -1.10e+02 | 3.3e+03 | 1.74e+03 | 7.38e+03 | 867 (err 8.6e-05) | 3.56e+03 | 8.89e+04 | 6.2e-06 | 2.1e-13 |
| Buddha | 2.18e+01 | 2.64e+03 | 1.23e+03 | 7.23e+03 | 692 (err 5.0e-06) | 2.46e+03 | 7.05e+04 | 6.5e-06 | 2.7e-12 |
| mk7 part | -7.40e+01 | 290 | 1.72e+03 | 5.52e+03 | 796 (err 2.9e-06) | 1e+03 | 5.73e+04 | 2.2e-06 | 2.2e-12 |

## Error (rad) and model cost per point (us) at 1.5 m from the surface, 128 across

| shape | signal | whole mesh | cells (paper's model) + (d) | octree strict + (d) | octree S3 + (d) | free boxes + (d) | free boxes + (e) | (d) error | (e) error |
|---|---|---|---|---|---|---|---|---|---|
| sphere | -2.94e+02 | 282 | 1.03e+04 | 3.32e+03 | 1.71e+03 (err 3.5e-05) | 5.25e+03 | 1.09e+05 | 9.6e-08 | 1.3e-12 |
| torus | -2.12e+02 | 225 | 3.17e+03 | 2.69e+03 | 1.07e+03 (err 1.0e-06) | 3.52e+03 | 6.95e+04 | 2.4e-07 | 2.2e-12 |
| teapot | -9.14e+00 | 366 | 2.11e+03 | 732 | 645 (err 2.8e-09) | 2.24e+03 | 4.67e+04 | 5.9e-10 | 5.1e-13 |
| Suzanne | -1.20e+01 | 62.5 | 2.38e+03 | 936 | 843 (err 2.5e-09) | 2.37e+03 | 5.18e+04 | 7.8e-10 | 1.9e-13 |
| bunny | -1.74e+01 | 3.83e+03 | 4.3e+03 | 1.45e+03 | 1.26e+03 (err 6.5e-09) | 4.46e+03 | 1.16e+05 | 1.1e-09 | 5.8e-13 |
| Spot | -1.38e+01 | 322 | 3.1e+03 | 1.14e+03 | 998 (err 2.6e-09) | 2.74e+03 | 6.6e+04 | 9.1e-10 | 9.9e-14 |
| dragon | -5.13e+00 | 2.7e+03 | 1.53e+03 | 926 | 896 (err 4.0e-10) | 3.19e+03 | 8.63e+04 | 3.9e-10 | 2.1e-13 |
| Armadillo | -7.94e+00 | 3.3e+03 | 1.74e+03 | 908 | 866 (err 8.0e-10) | 3.11e+03 | 8.85e+04 | 4.2e-10 | 6.2e-13 |
| Buddha | -4.19e+00 | 2.64e+03 | 1.23e+03 | 707 | 690 (err 6.9e-10) | 2.02e+03 | 7.01e+04 | 2.3e-10 | 6.2e-15 |
| mk7 part | -6.95e+00 | 290 | 1.72e+03 | 839 | 796 (err 3.9e-10) | 969 | 5.73e+04 | 7.3e-11 | 6.1e-13 |

## Error (rad) and model cost per point (us) at 100 m from the surface, 128 across

| shape | signal | whole mesh | cells (paper's model) + (d) | octree strict + (d) | octree S3 + (d) | free boxes + (d) | free boxes + (e) | (d) error | (e) error |
|---|---|---|---|---|---|---|---|---|---|
| sphere | -1.43e-02 | 282 | 1.03e+04 | 1.71e+03 | 1.71e+03 (err 3.6e-13) | 1.41e+03 | 1.05e+05 | 5.3e-13 | 6.4e-14 |
| torus | -1.06e-02 | 225 | 3.17e+03 | 1.07e+03 | 1.07e+03 (err 2.3e-13) | 1.09e+03 | 6.7e+04 | 1.0e-13 | 1.0e-12 |
| teapot | -3.38e-04 | 366 | 2.11e+03 | 645 | 645 (err 3.2e-13) | 545 | 4.5e+04 | 2.9e-13 | 8.3e-14 |
| Suzanne | -3.68e-04 | 62.5 | 2.38e+03 | 843 | 843 (err 6.2e-14) | 702 | 5.01e+04 | 8.5e-14 | 9.1e-14 |
| bunny | -6.97e-04 | 3.83e+03 | 4.3e+03 | 1.26e+03 | 1.26e+03 (err 2.9e-13) | 1.06e+03 | 1.12e+05 | 1.3e-13 | 6.1e-14 |
| Spot | -4.95e-04 | 322 | 3.1e+03 | 998 | 998 (err 3.2e-14) | 816 | 6.41e+04 | 2.9e-13 | 2.9e-13 |
| dragon | -1.94e-04 | 2.7e+03 | 1.53e+03 | 896 | 896 (err 4.0e-14) | 782 | 8.39e+04 | 1.4e-13 | 3.2e-13 |
| Armadillo | -2.38e-04 | 3.3e+03 | 1.74e+03 | 866 | 866 (err 8.5e-15) | 756 | 8.61e+04 | 2.0e-13 | 3.3e-14 |
| Buddha | -1.59e-04 | 2.64e+03 | 1.23e+03 | 690 | 690 (err 4.0e-14) | 600 | 6.86e+04 | 2.0e-13 | 5.7e-14 |
| mk7 part | -2.37e-04 | 290 | 1.72e+03 | 796 | 796 (err 1.7e-13) | 644 | 5.69e+04 | 1.7e-13 | 1.8e-13 |

## Corrected cost to reach 1e-4 / 1e-6 / 1e-8 rad at 1.5 m (us per point, model), 4-128 across

Interior as: every cell at 8.5 ns each (the paper's Table 5; the series' own error not included) / the octree's blocks / free boxes (sweep). For the octree and the boxes the series switch is the cheapest of strict, 0.05 and S3 that reaches the tolerance, the interior's own error (series truncation) added to the boundary's. Boundary: (a) 8.5 ns per cell whose centre is inside, (b) 8.5 ns per cut cell (errors from the shapes study's sweep), (c) 8.5 ns, (d) 17 ns per cut cell, (e) 55 ns per piece triangle. — not reached within 128 across.

| shape | whole mesh | (d), cells (paper) | (d), octree | (d), free boxes | (c), octree | (b), octree | (a), octree | (e), free boxes |
|---|---|---|---|---|---|---|---|---|
| sphere | 282 | 77 / 1.89e+03 / — | 60 / 593 / — | 149 / 1.6e+03 / — | 10.9 / 168 / — | — / — / — | — / — / — | 691 / 691 / 691 |
| torus | 225 | 48.9 / 1.16e+03 / — | 42.3 / 568 / — | 118 / 1.7e+03 / — | 148 / — / — | — / — / — | — / — / — | 515 / 515 / 515 |
| teapot | 366 | 0.808 / 13.7 / 286 | 0.808 / 11.7 / 168 | 1.14 / 38.6 / 497 | 4.38 / 94.4 / — | — / — / — | — / — / — | 674 / 674 / 674 |
| Suzanne | 62.5 | 1.26 / 19.2 / 407 | 1.26 / 16.8 / 260 | 1.66 / 44 / 633 | 3.25 / 96.2 / — | 54.4 / — / — | — / — / — | 218 / 218 / 218 |
| bunny | 3.83e+03 | 1.43 / 39.9 / 893 | 1.42 / 32.4 / 608 | 3.07 / 111 / 1.42e+03 | 1.52 / 77.9 / — | 265 / — / — | — / — / — | 5.01e+03 / 5.01e+03 / 5.01e+03 |
| Spot | 322 | 1.37 / 25 / 582 | 1.37 / 21.7 / 357 | 0.904 / 55.7 / 811 | 8.46 / 198 / — | 228 / — / — | — / — / — | 594 / 594 / 594 |
| dragon | 2.7e+03 | 0.51 / 7 / 175 | 0.51 / 6.97 / 147 | 0.51 / 7.92 / 514 | 2.1 / 109 / — | 53.4 / — / — | — / — / — | 3.74e+03 / 3.74e+03 / 3.74e+03 |
| Armadillo | 3.3e+03 | 1.16 / 10.6 / 208 | 1.16 / 10.4 / 161 | 1.18 / 23.7 / 568 | 1.8 / 99.8 / — | 370 / — / — | — / — / — | 4.33e+03 / 4.33e+03 / 4.33e+03 |
| Buddha | 2.64e+03 | 0.36 / 1.91 / 116 | 0.36 / 1.91 / 95.3 | 0.36 / 2.14 / 272 | 2.52 / 88.3 / — | — / — / — | — / — / — | 3.28e+03 / 3.28e+03 / 3.28e+03 |
| mk7 part | 290 | 0.587 / 7 / 48 | 0.587 / 6.87 / 42.5 | 0.614 / 8.08 / 37.1 | 6.95 / 152 / — | — / — / — | — / — / — | 597 / 597 / 597 |

Whole mesh against the cheapest voxel route (d, octree or free boxes) at each tolerance: sphere 1e-04: voxels (paper's model: voxels); sphere 1e-06: mesh (paper's model: mesh); sphere 1e-08: mesh (paper's model: mesh); torus 1e-04: voxels (paper's model: voxels); torus 1e-06: mesh (paper's model: mesh); torus 1e-08: mesh (paper's model: mesh); teapot 1e-04: voxels (paper's model: voxels); teapot 1e-06: voxels (paper's model: voxels); teapot 1e-08: voxels (paper's model: voxels); Suzanne 1e-04: voxels (paper's model: voxels); Suzanne 1e-06: voxels (paper's model: voxels); Suzanne 1e-08: mesh (paper's model: mesh); bunny 1e-04: voxels (paper's model: voxels); bunny 1e-06: voxels (paper's model: voxels); bunny 1e-08: voxels (paper's model: voxels); Spot 1e-04: voxels (paper's model: voxels); Spot 1e-06: voxels (paper's model: voxels); Spot 1e-08: mesh (paper's model: mesh); dragon 1e-04: voxels (paper's model: voxels); dragon 1e-06: voxels (paper's model: voxels); dragon 1e-08: voxels (paper's model: voxels); Armadillo 1e-04: voxels (paper's model: voxels); Armadillo 1e-06: voxels (paper's model: voxels); Armadillo 1e-08: voxels (paper's model: voxels); Buddha 1e-04: voxels (paper's model: voxels); Buddha 1e-06: voxels (paper's model: voxels); Buddha 1e-08: voxels (paper's model: voxels); mk7 part 1e-04: voxels (paper's model: voxels); mk7 part 1e-06: voxels (paper's model: voxels); mk7 part 1e-08: voxels (paper's model: voxels).

Cases where (d) is not the cheapest of (d), (c), (b) on the octree: [('sphere', 0.0001), ('sphere', 1e-06)].

## B1: one element, relative error of g_z against R/s (f64 reference)

| element | variant | 1e+00 | 1e+01 | 1e+02 | 1e+03 | 1e+04 | 1e+05 |
|---|---|---|---|---|---|---|---|
| triangle | f32 local h2A log1p | 7e-08 | 1e-06 | 4e-05 | 2e-04 | 4e-03 | 1e-02 |
| triangle | f32 local triple log1p | 7e-08 | 1e-06 | 5e-05 | 3e-04 | 5e-03 | 2e+01 |
| triangle | f32 local h2A ratio-log | 7e-08 | 5e-07 | 3e-03 | 1e-01 | 9e+00 | 2e+03 |
| triangle | f32 body frame h2A log1p | 3e-06 | 4e-06 | 4e-05 | 2e-04 | 2e-03 | 1e-02 |
| triangle | f32 world 1 km h2A log1p | 2e-03 | 2e-03 | 2e-03 | 3e-03 | 2e-03 | 1e-02 |
| triangle | f32 local h2A, WGSL worst case | 2e-05 | 1e-04 | 1e-02 | 7e-01 | 1e+02 | 6e+03 |
| triangle | f64 vs 40 digits |  |
| box 1:1:1 | f32 local h2A log1p | 6e-08 | 2e-05 | 1e-03 | 3e-02 | 2e+01 | 2e+03 |
| box 1:1:1 | f32 local triple log1p | 6e-08 | 3e-05 | 1e-02 | 1e+01 | 5e+03 | 5e+06 |
| box 1:1:1 | f32 local h2A ratio-log | 3e-07 | 5e-05 | 1e-01 | 2e+01 | 9e+00 | 6e+02 |
| box 1:1:1 | f32 body frame h2A log1p | 1e-06 | 2e-05 | 5e-04 | 2e-01 | 2e+01 | 2e+03 |
| box 1:1:1 | f32 world 1 km h2A log1p | 1e-03 | 3e-05 | 3e-04 | 6e-02 | 1e+01 | 8e+02 |
| box 1:1:1 | f32 local h2A, WGSL worst case | 2e-05 | 2e-03 | 3e-01 | 1e+02 | 5e+05 | 3e+08 |
| box 1:1:1 | f64 vs 40 digits | 1e-16 | 2e-14 | 3e-12 | 1e-10 | 1e-09 | 2e-07 |
| box 1:1:16 | f32 local h2A log1p | 2e-06 | 2e-04 | 1e-02 | 4e+00 | 6e+01 | 2e+04 |
| box 1:1:16 | f32 local triple log1p | 3e-06 | 4e-03 | 1e+00 | 4e+02 | 4e+06 | 2e+09 |
| box 1:1:16 | f32 local h2A ratio-log | 5e-06 | 2e-04 | 2e+01 | 2e+00 | 1e+02 | 1e+04 |
| box 1:1:16 | f32 body frame h2A log1p | 3e-06 | 2e-04 | 1e-02 | 4e+00 | 1e+02 | 8e+03 |
| box 1:1:16 | f32 world 1 km h2A log1p | 2e-03 | 2e-03 | 3e-02 | 6e-01 | 2e+02 | 8e+03 |
| box 1:1:16 | f32 local h2A, WGSL worst case | 3e-04 | 2e-01 | 1e+02 | 1e+05 | 3e+07 | 2e+11 |
| box 1:1:16 | f64 vs 40 digits | 3e-15 | 3e-13 | 7e-12 | 6e-10 | 1e-07 | 8e-06 |
| box 1:1:256 | f32 local h2A log1p | 6e-05 | 3e-03 | 4e-01 | 3e+01 | 8e-01 | 4e+05 |
| box 1:1:256 | f32 local triple log1p | 3e-04 | 3e-02 | 7e+01 | 2e+05 | 1e+08 | 2e+10 |
| box 1:1:256 | f32 local h2A ratio-log | 2e-03 | 8e+00 | 8e-01 | 9e+01 | 6e+02 | 8e-01 |
| box 1:1:256 | f32 body frame h2A log1p | 7e-05 | 4e-03 | 4e-01 | 5e+00 | 1e+03 | 4e+04 |
| box 1:1:256 | f32 world 1 km h2A log1p | 4e-03 | 5e-03 | 2e-01 | 1e+01 | 1e+03 | 4e+04 |
| box 1:1:256 | f32 local h2A, WGSL worst case | 3e-02 | 5e+01 | 5e+04 | 4e+07 | 2e+10 | 6e+13 |
| box 1:1:256 | f64 vs 40 digits | 5e-14 | 9e-12 | 7e-10 | 8e-09 | 2e-06 | 6e-04 |
| bound | 30 u (R/s)^2, u = 6e-8 | 2e-06 | 2e-04 | 2e-02 | 2e+00 | 2e+02 | 2e+04 |

## B: which f32 ingredient makes the error (bunny, whole mesh, 69664 triangles; terms summed exactly)

| ingredient in f32 | 0.1 m | 0.3 m | 1.5 m | 5 m | 20 m | 100 m | 1000 m |
|---|---|---|---|---|---|---|---|
| phase itself (f64) | -1.4e+02 | -7.4e+01 | -1.7e+01 | -4.0e+00 | -1.0e-01 | -7.0e-04 | -6.6e-07 |
| offsets | 8.7e-05 | -6.1e-05 | 1.5e-05 | -1.7e-06 | 6.4e-07 | -6.0e-07 | 3.0e-08 |
| functions | 3.5e-05 | 2.7e-05 | -2.5e-05 | -3.2e-05 | -1.0e-05 | 1.6e-05 | -8.8e-06 |
| geometry | -1.1e-05 | -1.1e-05 | -1.1e-05 | -8.2e-06 | -2.5e-06 | -4.9e-07 | -4.9e-08 |
| products | -9.3e-05 | 2.1e-05 | -1.0e-05 | -7.6e-05 | -9.3e-05 | 1.7e-05 | 1.1e-04 |
| all f32 | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |

## B2-B4: bunny at 32 cells across (1537 octree blocks, 3465 cut cells, 323780 piece triangles): phase error of the f32 evaluation (rad)

f32 cut-cell moments: total mass relative error 8.0e-08; largest centroid error 6.6e-04 h.

| part, frame, sum | 0.1 m | 0.3 m | 1.5 m | 5 m | 20 m | 100 m | 1000 m |
|---|---|---|---|---|---|---|---|
| blockseries, local, f64acc | -8.3e-07 | -2.1e-07 | -2.0e-07 | 2.7e-07 | -3.1e-09 | 5.4e-11 | -8.9e-14 |
| blockseries, local, ff | -8.3e-07 | -2.1e-07 | -2.0e-07 | 2.7e-07 | -3.1e-09 | 5.4e-11 | -8.9e-14 |
| blockseries, local, kahan | -5.0e-06 | -8.1e-07 | -1.2e-07 | 3.6e-07 | -3.4e-10 | -2.1e-10 | 3.2e-14 |
| blockseries, local, lane32 | -5.0e-06 | -8.1e-07 | -1.2e-07 | 3.6e-07 | -3.4e-10 | -2.1e-10 | 3.2e-14 |
| blockseries, local, seq32 | -3.1e-05 | -3.4e-05 | 2.8e-07 | -4.1e-08 | 8.0e-08 | 1.6e-09 | -2.1e-11 |
| blockseries, signal | -1.0e+02 | -5.3e+01 | -1.3e+01 | -3.0e+00 | -7.5e-02 | -5.3e-04 | -5.0e-07 |
| blocks, body, ff | 4.7e-05 | 9.6e-05 | -2.0e-04 | 9.6e-05 | 3.8e-05 | 3.0e-04 | 1.2e-04 |
| blocks, local, f64acc | 9.5e-06 | 8.3e-05 | -2.6e-04 | 1.1e-04 | 3.8e-05 | -2.0e-04 | 1.5e-04 |
| blocks, local, ff | 9.5e-06 | 8.3e-05 | -2.6e-04 | 1.1e-04 | 3.8e-05 | -2.0e-04 | 1.5e-04 |
| blocks, local, kahan | 4.4e-06 | 9.9e-05 | -2.6e-04 | 1.1e-04 | 3.8e-05 | -2.0e-04 | 1.5e-04 |
| blocks, local, lane32 | 5.6e-06 | 7.0e-05 | -2.6e-04 | 1.0e-04 | 3.8e-05 | -2.0e-04 | 1.5e-04 |
| blocks, local, seq32 | 1.9e-05 | 8.4e-05 | -2.6e-04 | 1.1e-04 | 3.8e-05 | -2.0e-04 | 1.5e-04 |
| blocks, signal | -1.0e+02 | -5.3e+01 | -1.3e+01 | -3.0e+00 | -7.5e-02 | -5.3e-04 | -5.0e-07 |
| blocks, wgsl, ff | 3.3e-02 | 2.5e-02 | 8.6e-02 | -2.3e-02 | -1.0e+00 | 6.5e+00 | -2.3e+01 |
| blocks, world, ff | -1.6e-02 | -6.6e-03 | -3.5e-04 | -3.1e-04 | 2.2e-04 | 8.2e-05 | -6.5e-05 |
| mesh, body, ff | -2.5e-05 | 1.9e-04 | -1.8e-04 | -6.0e-05 | -1.7e-04 | -1.6e-04 | 6.3e-05 |
| mesh, local, f64acc | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, ff | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, kahan | 1.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, lane32 | 3.2e-05 | 1.7e-04 | -1.1e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, seq32 | -1.6e-03 | -2.3e-04 | 1.9e-04 | -1.8e-04 | -6.0e-05 | -1.4e-05 | -6.6e-05 |
| mesh, signal | -1.4e+02 | -7.4e+01 | -1.7e+01 | -4.0e+00 | -1.0e-01 | -7.0e-04 | -6.6e-07 |
| mesh, wgsl, ff | -1.7e-01 | -1.1e-01 | -2.6e-01 | -3.1e-01 | -7.0e-01 | 6.7e+00 | -4.5e+01 |
| mesh, world, ff | -2.1e-02 | -1.0e-02 | 1.6e-03 | 8.9e-04 | -2.5e-05 | -2.4e-05 | 1.1e-04 |
| moments, local, f64acc | -3.8e-06 | -2.9e-06 | -4.5e-07 | 1.5e-08 | -2.9e-09 | -9.1e-12 | -4.5e-14 |
| moments, local, ff | -3.8e-06 | -2.9e-06 | -4.5e-07 | 1.5e-08 | -2.9e-09 | -9.1e-12 | -4.5e-14 |
| moments, local, kahan | -3.6e-06 | -3.0e-06 | -5.1e-07 | 3.2e-08 | -3.6e-09 | 4.7e-11 | 5.4e-13 |
| moments, local, lane32 | -3.6e-06 | -3.0e-06 | -5.1e-07 | 3.2e-08 | -3.6e-09 | 4.7e-11 | -2.3e-13 |
| moments, local, seq32 | 7.1e-08 | -1.0e-05 | 1.5e-06 | -3.2e-07 | -4.5e-08 | -1.5e-10 | -6.4e-12 |
| moments, signal | -3.8e+01 | -2.0e+01 | -4.5e+00 | -1.0e+00 | -2.5e-02 | -1.7e-04 | -1.6e-07 |
| pieces, body, ff | 5.8e-04 | 1.8e-04 | 2.4e-04 | 1.8e-04 | 1.3e-04 | -3.2e-04 | 1.8e-04 |
| pieces, local, f64acc | 2.6e-04 | 2.1e-04 | 2.1e-04 | -2.1e-05 | 1.9e-04 | -7.3e-05 | -8.3e-05 |
| pieces, local, ff | 2.6e-04 | 2.1e-04 | 2.1e-04 | -2.1e-05 | 1.9e-04 | -7.3e-05 | -8.3e-05 |
| pieces, local, kahan | 2.6e-04 | 2.1e-04 | 2.1e-04 | -2.0e-05 | 1.9e-04 | -7.3e-05 | -8.3e-05 |
| pieces, local, lane32 | 2.5e-04 | 2.1e-04 | 2.1e-04 | -2.2e-05 | 1.9e-04 | -7.3e-05 | -8.3e-05 |
| pieces, local, seq32 | 2.2e-04 | 1.6e-04 | 2.2e-04 | -2.0e-05 | 1.9e-04 | -7.3e-05 | -8.3e-05 |
| pieces, signal | -3.8e+01 | -2.0e+01 | -4.5e+00 | -1.0e+00 | -2.5e-02 | -1.7e-04 | -1.6e-07 |
| pieces, wgsl, ff | 8.1e-03 | 3.3e-01 | -2.0e-01 | -4.4e-01 | -2.1e+00 | 3.4e+00 | -1.8e+02 |
| pieces, world, ff | -2.8e-01 | -2.0e-01 | -4.9e-02 | -3.9e-03 | 2.4e-04 | -1.3e-04 | -4.4e-05 |

## B2-B4: bunny at 64 cells across (7487 octree blocks, 14180 cut cells, 739600 piece triangles): phase error of the f32 evaluation (rad)

f32 cut-cell moments: total mass relative error 8.4e-08; largest centroid error 4.4e-04 h.

| part, frame, sum | 0.1 m | 0.3 m | 1.5 m | 5 m | 20 m | 100 m | 1000 m |
|---|---|---|---|---|---|---|---|
| blockseries, local, f64acc | 4.6e-07 | 1.4e-07 | 3.8e-08 | -4.1e-07 | -5.9e-09 | -3.1e-11 | -3.1e-13 |
| blockseries, local, ff | 4.6e-07 | 1.4e-07 | 3.8e-08 | -4.1e-07 | -5.9e-09 | -3.1e-11 | -3.1e-13 |
| blockseries, local, kahan | 1.3e-06 | -4.1e-06 | 3.1e-07 | -4.5e-07 | -1.1e-08 | 2.0e-10 | 6.5e-12 |
| blockseries, local, lane32 | 1.3e-06 | 3.1e-06 | 3.1e-07 | -4.5e-07 | -1.1e-08 | -2.0e-10 | 6.5e-12 |
| blockseries, local, seq32 | 5.8e-04 | -2.5e-06 | 1.6e-05 | 2.0e-06 | -2.3e-07 | -1.9e-08 | -1.9e-10 |
| blockseries, signal | -1.2e+02 | -6.2e+01 | -1.5e+01 | -3.4e+00 | -8.7e-02 | -6.1e-04 | -5.8e-07 |
| blocks, body, ff | 1.5e-04 | -1.2e-04 | 3.2e-04 | -1.7e-04 | -2.4e-04 | -2.3e-04 | -2.4e-04 |
| blocks, local, f64acc | 9.6e-05 | -8.1e-05 | 2.3e-04 | -2.3e-04 | -2.5e-04 | -1.3e-04 | -2.4e-04 |
| blocks, local, ff | 9.6e-05 | -8.1e-05 | 2.3e-04 | -2.3e-04 | -2.5e-04 | -1.3e-04 | -2.4e-04 |
| blocks, local, kahan | 8.5e-05 | -7.6e-05 | 2.3e-04 | -2.3e-04 | -2.5e-04 | -1.3e-04 | -2.4e-04 |
| blocks, local, lane32 | 1.0e-04 | -6.6e-05 | 2.4e-04 | -2.3e-04 | -2.5e-04 | -1.3e-04 | -2.4e-04 |
| blocks, local, seq32 | 4.7e-04 | 2.1e-04 | 1.8e-04 | -2.1e-04 | -2.5e-04 | -1.3e-04 | -2.4e-04 |
| blocks, signal | -1.2e+02 | -6.2e+01 | -1.5e+01 | -3.4e+00 | -8.7e-02 | -6.1e-04 | -5.8e-07 |
| blocks, wgsl, ff | -8.0e-02 | -4.3e-02 | -1.6e-01 | 3.7e-02 | -2.7e-01 | 2.6e+00 | -5.9e+01 |
| blocks, world, ff | 1.2e-02 | 2.5e-03 | 2.1e-04 | 2.1e-04 | -1.2e-04 | -1.4e-04 | -4.0e-05 |
| mesh, body, ff | -2.5e-05 | 1.9e-04 | -1.8e-04 | -6.0e-05 | -1.7e-04 | -1.6e-04 | 6.3e-05 |
| mesh, local, f64acc | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, ff | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, kahan | 1.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, lane32 | 3.2e-05 | 1.7e-04 | -1.1e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, seq32 | -1.6e-03 | -2.3e-04 | 1.9e-04 | -1.8e-04 | -6.0e-05 | -1.4e-05 | -6.6e-05 |
| mesh, signal | -1.4e+02 | -7.4e+01 | -1.7e+01 | -4.0e+00 | -1.0e-01 | -7.0e-04 | -6.6e-07 |
| mesh, wgsl, ff | 9.2e-02 | 5.8e-02 | 1.1e-01 | 1.8e-01 | 1.5e+00 | 1.6e+00 | -9.4e+01 |
| mesh, world, ff | -2.1e-02 | -1.0e-02 | 1.6e-03 | 8.9e-04 | -2.5e-05 | -2.4e-05 | 1.1e-04 |
| moments, local, f64acc | -1.9e-06 | -8.9e-07 | -2.0e-07 | -1.0e-07 | -2.1e-09 | -2.5e-12 | -6.8e-15 |
| moments, local, ff | -1.9e-06 | -8.9e-07 | -2.0e-07 | -1.0e-07 | -2.1e-09 | -2.5e-12 | -6.8e-15 |
| moments, local, kahan | -2.3e-06 | -1.1e-06 | -3.2e-07 | -1.2e-07 | -2.5e-09 | 3.2e-11 | 2.6e-13 |
| moments, local, lane32 | -2.3e-06 | -1.1e-06 | -3.2e-07 | -1.2e-07 | -2.5e-09 | 3.2e-11 | 2.6e-13 |
| moments, local, seq32 | 9.3e-06 | 6.4e-06 | 1.1e-06 | -2.8e-08 | 1.5e-08 | -1.8e-09 | 8.7e-12 |
| moments, signal | -2.2e+01 | -1.2e+01 | -2.4e+00 | -5.3e-01 | -1.3e-02 | -9.0e-05 | -8.6e-08 |
| pieces, body, ff | 3.4e-04 | -1.5e-04 | -1.9e-04 | -1.5e-04 | -9.2e-05 | 4.7e-05 | 2.2e-04 |
| pieces, local, f64acc | 1.1e-04 | 1.5e-04 | -8.4e-05 | -3.0e-04 | -1.2e-04 | -6.8e-05 | 3.6e-05 |
| pieces, local, ff | 1.1e-04 | 1.5e-04 | -8.4e-05 | -3.0e-04 | -1.2e-04 | -6.8e-05 | 3.6e-05 |
| pieces, local, kahan | 1.2e-04 | 1.5e-04 | -8.5e-05 | -3.0e-04 | -1.2e-04 | -6.8e-05 | 3.6e-05 |
| pieces, local, lane32 | 1.2e-04 | 1.5e-04 | -8.4e-05 | -3.0e-04 | -1.2e-04 | -6.8e-05 | 3.6e-05 |
| pieces, local, seq32 | 2.0e-04 | 1.7e-04 | -7.8e-05 | -3.0e-04 | -1.2e-04 | -6.8e-05 | 3.6e-05 |
| pieces, signal | -2.2e+01 | -1.2e+01 | -2.4e+00 | -5.3e-01 | -1.3e-02 | -9.0e-05 | -8.6e-08 |
| pieces, wgsl, ff | -3.2e-01 | 2.9e-01 | -8.8e-01 | 1.2e+00 | 2.4e+00 | -1.9e+01 | 1.8e+02 |
| pieces, world, ff | -1.2e-02 | 2.0e-03 | -4.4e-04 | -4.7e-04 | -8.1e-05 | -1.3e-04 | 1.7e-04 |

## B2-B4: bunny at 128 cells across (34378 octree blocks, 56987 cut cells, 2040788 piece triangles): phase error of the f32 evaluation (rad)

f32 cut-cell moments: total mass relative error 8.7e-08; largest centroid error 5.9e-04 h.

| part, frame, sum | 0.1 m | 0.3 m | 1.5 m | 5 m | 20 m | 100 m | 1000 m |
|---|---|---|---|---|---|---|---|
| blockseries, local, f64acc | -3.7e-06 | -6.2e-07 | -1.4e-06 | 6.3e-08 | 2.3e-09 | -9.4e-11 | -5.9e-14 |
| blockseries, local, ff | -3.7e-06 | -6.2e-07 | -1.4e-06 | 6.3e-08 | 2.3e-09 | -9.4e-11 | -5.9e-14 |
| blockseries, local, kahan | 9.2e-06 | -2.1e-06 | -1.1e-06 | -7.7e-08 | 1.2e-08 | -2.8e-10 | 2.1e-12 |
| blockseries, local, lane32 | -3.7e-06 | -2.1e-06 | -1.1e-06 | 1.2e-07 | 1.1e-08 | -2.8e-10 | 2.1e-12 |
| blockseries, local, seq32 | -7.1e-04 | 5.9e-04 | 4.9e-06 | -1.9e-05 | 1.3e-06 | 4.3e-07 | -4.1e-08 |
| blockseries, signal | -1.3e+02 | -6.8e+01 | -1.6e+01 | -3.7e+00 | -9.3e-02 | -6.5e-04 | -6.2e-07 |
| blocks, body, ff | 1.0e-04 | 1.8e-04 | 4.6e-05 | -6.4e-05 | -9.4e-05 | 1.5e-04 | 2.4e-04 |
| blocks, local, f64acc | -6.7e-05 | 3.1e-05 | 9.8e-05 | -7.5e-05 | -1.5e-04 | 1.5e-04 | -2.4e-05 |
| blocks, local, ff | -6.7e-05 | 3.1e-05 | 9.8e-05 | -7.5e-05 | -1.5e-04 | 1.5e-04 | -2.4e-05 |
| blocks, local, kahan | -6.0e-05 | 2.9e-05 | 9.8e-05 | -7.4e-05 | -1.5e-04 | 1.5e-04 | -2.4e-05 |
| blocks, local, lane32 | -2.4e-06 | 1.1e-04 | 1.1e-04 | -7.8e-05 | -1.4e-04 | 1.5e-04 | -2.4e-05 |
| blocks, local, seq32 | 5.0e-04 | -1.2e-03 | 2.3e-05 | -7.7e-05 | -1.5e-04 | 1.5e-04 | -2.4e-05 |
| blocks, signal | -1.3e+02 | -6.8e+01 | -1.6e+01 | -3.7e+00 | -9.3e-02 | -6.5e-04 | -6.2e-07 |
| blocks, wgsl, ff | -8.6e-02 | -3.6e-01 | -1.9e-01 | -3.4e-01 | -1.9e+00 | -5.2e+00 | -5.3e+01 |
| blocks, world, ff | 4.1e-03 | 8.5e-05 | 6.7e-06 | -1.4e-04 | -9.4e-05 | -8.7e-05 | 3.1e-04 |
| mesh, body, ff | -2.5e-05 | 1.9e-04 | -1.8e-04 | -6.0e-05 | -1.7e-04 | -1.6e-04 | 6.3e-05 |
| mesh, local, f64acc | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, ff | 2.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, kahan | 1.8e-05 | 1.7e-04 | -1.0e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, lane32 | 3.2e-05 | 1.7e-04 | -1.1e-04 | -1.5e-04 | -9.1e-05 | -2.0e-05 | -6.7e-05 |
| mesh, local, seq32 | -1.6e-03 | -2.3e-04 | 1.9e-04 | -1.8e-04 | -6.0e-05 | -1.4e-05 | -6.6e-05 |
| mesh, signal | -1.4e+02 | -7.4e+01 | -1.7e+01 | -4.0e+00 | -1.0e-01 | -7.0e-04 | -6.6e-07 |
| mesh, wgsl, ff | -3.8e-02 | -3.4e-02 | -2.5e-01 | 6.6e-01 | -1.3e+00 | -2.9e+00 | 7.4e+01 |
| mesh, world, ff | -2.1e-02 | -1.0e-02 | 1.6e-03 | 8.9e-04 | -2.5e-05 | -2.4e-05 | 1.1e-04 |
| moments, local, f64acc | -1.1e-06 | -4.4e-07 | -2.0e-07 | -2.1e-08 | -4.0e-10 | -4.8e-12 | -1.5e-15 |
| moments, local, ff | -1.1e-06 | -4.4e-07 | -2.0e-07 | -2.1e-08 | -4.0e-10 | -4.8e-12 | -1.5e-15 |
| moments, local, kahan | -1.0e-06 | -1.9e-07 | -2.0e-07 | -2.0e-08 | -2.0e-10 | -9.6e-12 | -2.4e-14 |
| moments, local, lane32 | -1.0e-06 | -6.4e-07 | -2.0e-07 | -2.0e-08 | -3.0e-10 | -9.6e-12 | 1.7e-13 |
| moments, local, seq32 | 2.3e-06 | 9.7e-06 | 4.3e-06 | 1.7e-06 | 4.6e-08 | -1.0e-09 | 1.6e-11 |
| moments, signal | -1.1e+01 | -5.8e+00 | -1.3e+00 | -2.7e-01 | -6.7e-03 | -4.7e-05 | -4.4e-08 |
| pieces, body, ff | 1.5e-04 | 1.9e-04 | -1.0e-04 | 2.8e-04 | 1.4e-05 | -1.4e-04 | -6.2e-05 |
| pieces, local, f64acc | -1.8e-05 | -4.4e-05 | 4.4e-05 | 1.2e-04 | -2.3e-04 | -1.3e-04 | -3.3e-05 |
| pieces, local, ff | -1.8e-05 | -4.4e-05 | 4.4e-05 | 1.2e-04 | -2.3e-04 | -1.3e-04 | -3.3e-05 |
| pieces, local, kahan | -2.2e-05 | -4.6e-05 | 4.4e-05 | 1.2e-04 | -2.3e-04 | -1.3e-04 | -3.3e-05 |
| pieces, local, lane32 | -2.3e-05 | -3.9e-05 | 4.2e-05 | 1.2e-04 | -2.3e-04 | -1.3e-04 | -3.3e-05 |
| pieces, local, seq32 | -4.0e-06 | -5.3e-05 | 4.5e-05 | 1.2e-04 | -2.3e-04 | -1.3e-04 | -3.3e-05 |
| pieces, signal | -1.1e+01 | -5.8e+00 | -1.3e+00 | -2.7e-01 | -6.7e-03 | -4.7e-05 | -4.4e-08 |
| pieces, wgsl, ff | -2.5e-01 | 1.3e-02 | 4.0e-02 | -9.9e-01 | -8.3e+00 | -6.7e+01 | 8.6e+01 |
| pieces, world, ff | 3.4e-01 | 2.4e-01 | 6.2e-02 | 5.3e-03 | -2.8e-04 | -2.9e-05 | 7.2e-05 |
