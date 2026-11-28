Noise σ = 5.0e-05 rad per shot (10⁻⁴ rad/√Hz, 2 s cycle), 61 shots, v = 1.0 m/s. Parameters (ln M, x₀, y₀, v).

## Cramér–Rao bounds

| body | d (m) | peak (rad) | SNR | σ(M)/M | σ(x₀) | σ(y₀) | σ(v) | σ(M)/M with z₀ free |
|---|---|---|---|---|---|---|---|---|
| cube | 1.5 | 64.8 | 1.39e+06 | 4.63e-06 | 0.00167 mm | 0.00298 mm | 0.000891 mm/s | 2.86e-05 |
| cube | 5.0 | 4.68 | 2.49e+05 | 5.91e-06 | 0.0427 mm | 0.0776 mm | 0.00749 mm/s | 1.81e-03 |
| cube | 20.0 | 0.433 | 3.08e+04 | 1.58e-01 | 0.722 mm | 1.19e+03 mm | 52.5 mm/s | 3.51e+01 |
| bunny | 1.5 | 15.5 | 3.35e+05 | 2.56e-05 | 0.00709 mm | 0.018 mm | 0.00439 mm/s | 1.45e-04 |
| bunny | 5.0 | 0.804 | 4.47e+04 | 2.92e-05 | 0.246 mm | 0.274 mm | 0.0421 mm/s | 6.26e-03 |
| bunny | 20.0 | 0.086 | 6.13e+03 | 5.77e-01 | 6.99 mm | 4.4e+03 mm | 192 mm/s | 1.16e+01 |
| bracket | 1.5 | 6.33 | 1.34e+05 | 6.86e-05 | 0.0183 mm | 0.0447 mm | 0.0115 mm/s | 3.76e-04 |
| bracket | 5.0 | 0.313 | 1.67e+04 | 7.99e-05 | 0.64 mm | 1.03 mm | 0.113 mm/s | 2.16e-02 |
| bracket | 20.0 | 0.0294 | 2.1e+03 | 2.04e+00 | 15.5 mm | 1.55e+04 mm | 679 mm/s | 1.09e+01 |

## Pitch at which max |bias|/σ < 0.1 (worst of the placements); in brackets, ‖Δφ‖/σ < 0.1

\* extrapolated beyond 1/128 m with the fitted order of the last three pitches.

| body | technique | 1.5 m | 5 m | 20 m |
|---|---|---|---|---|
| cube | (a) cubes | 0.000683 mm* (6.19e-09 mm*) | 0.000841 mm* (0.000692 mm*) | 0.512 mm* (0.0318 mm*) |
| cube | (b) sampled fill | 0.000738 mm* (0.00185 mm*) | 0.0011 mm* (0.00151 mm*) | 3.13 mm* (0.141 mm*) |
| cube | (c) fill + centroid | 20.6 mm (11.1 mm) | 76 mm (75.9 mm) | ≥ 250 mm (220 mm) |
| cube | (d) + second moment | 131 mm (95.7 mm) | ≥ 250 mm (≥ 250 mm) | ≥ 250 mm (≥ 250 mm) |
| bunny | (a) cubes | 0.432 mm* (0.104 mm*) | 0.348 mm* (0.301 mm*) | 28.2 mm (1.02 mm*) |
| bunny | (b) sampled fill | 3.55 mm* (1.35 mm*) | 5.64 mm* (5.3 mm*) | 116 mm (43.8 mm) |
| bunny | (c) fill + centroid | 37.1 mm (23.6 mm) | 90.8 mm (82.8 mm) | ≥ 250 mm (≥ 250 mm) |
| bunny | (d) + second moment | 129 mm (100 mm) | ≥ 250 mm (≥ 250 mm) | ≥ 250 mm (≥ 250 mm) |
| bracket | (a) cubes | 0.0199 mm* (0.0131 mm*) | 0.735 mm* (0.509 mm*) | 2.91 mm* (0.77 mm*) |
| bracket | (b) sampled fill | 0.131 mm* (0.0989 mm*) | 0.0773 mm* (0.0633 mm*) | 19.4 mm (2.25 mm*) |
| bracket | (c) fill + centroid | 29.8 mm (19 mm) | 121 mm (116 mm) | ≥ 250 mm (≥ 250 mm) |
| bracket | (d) + second moment | 151 mm (91.8 mm) | ≥ 250 mm (≥ 250 mm) | ≥ 250 mm (≥ 250 mm) |

## max |bias|/σ by pitch (worst placement) — 1.5 / 5 / 20 m


**cube**

| n across | h (mm) | (a) cubes | (b) sampled fill | (c) fill + centroid | (d) + second moment | (e) ‖Δφ‖/σ |
|---|---|---|---|---|---|---|
| 4 | 250 | 7.5e+04 / 2.2e+04 / 1.7e+02 | 1.7e+04 / 2.6e+03 / 4.1e+01 | 2.1e+02 / 7.5e+00 / 1.6e-02 | 2.3e+00 / 3.5e-02 / 4.4e-06 | 8e-08 / 8e-08 / 7e-08 |
| 8 | 125 | 3.2e+04 / 9.7e+03 / 6.7e+01 | 8.1e+03 / 1.3e+03 / 1.9e+01 | 2.3e+01 / 1.2e+00 / 7.9e-04 | 8.1e-02 / 1.1e-03 / 1.8e-07 | 7e-08 / 8e-08 / 8e-08 |
| 16 | 62.5 | 1.7e+04 / 5.0e+03 / 3.2e+01 | 4.7e+03 / 1.1e+03 / 1.0e+01 | 2.3e+00 / 3.8e-02 / 1.8e-04 | 2.4e-03 / 4.0e-05 / 1.2e-08 | 9e-08 / 9e-08 / 9e-08 |
| 32 | 31.2 | 7.5e+03 / 1.9e+03 / 1.7e+01 | 2.1e+03 / 5.5e+02 / 4.9e+00 | 3.3e-01 / 1.5e-02 / 2.4e-05 | 1.2e-04 / 1.6e-06 / 8.8e-09 | 2e-07 / 7e-08 / 7e-08 |
| 64 | 15.6 | 4.6e+03 / 1.4e+03 / 7.9e+00 | 1.1e+03 / 2.2e+02 / 2.5e+00 | 4.4e-02 / 3.2e-03 / 3.9e-06 | 4.9e-06 / 8.8e-08 / 5.9e-09 | 1e-07 / 8e-08 / 6e-08 |
| 128 | 7.81 | 1.7e+03 / 5.2e+02 / 3.0e+00 | 5.8e+02 / 1.7e+02 / 4.2e-01 | 6.4e-03 / 3.2e-04 / 1.2e-07 | 5.2e-08 / 2.0e-08 / 8.9e-09 | 1e-07 / 8e-08 / 7e-08 |

**bunny**

| n across | h (mm) | (a) cubes | (b) sampled fill | (c) fill + centroid | (d) + second moment | (e) ‖Δφ‖/σ |
|---|---|---|---|---|---|---|
| 4 | 250 | 1.2e+04 / 7.1e+03 / 1.1e+01 | 1.1e+03 / 3.9e+02 / 7.0e-01 | 3.3e+01 / 3.0e+00 / 2.4e-03 | 1.5e+00 / 3.0e-02 / 1.5e-05 | 7e-08 / 7e-08 / 6e-08 |
| 8 | 125 | 2.9e+03 / 1.3e+03 / 1.2e+00 | 3.2e+02 / 1.1e+02 / 1.2e-01 | 3.7e+00 / 3.3e-01 / 3.8e-04 | 8.9e-02 / 2.8e-03 / 3.3e-07 | 6e-08 / 7e-08 / 6e-08 |
| 16 | 62.5 | 9.7e+02 / 8.5e+02 / 2.4e-01 | 5.2e+01 / 1.8e+01 / 2.1e-02 | 4.8e-01 / 2.5e-02 / 5.4e-05 | 5.1e-03 / 2.2e-04 / 2.9e-08 | 5e-08 / 6e-08 / 5e-08 |
| 32 | 31.2 | 2.2e+02 / 1.2e+02 / 1.1e-01 | 2.5e+01 / 8.6e+00 / 4.5e-03 | 5.9e-02 / 4.0e-03 / 6.8e-06 | 3.1e-04 / 1.2e-05 / 1.9e-08 | 5e-08 / 6e-08 / 6e-08 |
| 64 | 15.6 | 5.6e+01 / 2.1e+01 / 5.4e-02 | 4.5e+00 / 1.4e+00 / 4.0e-04 | 7.0e-03 / 4.1e-04 / 8.4e-07 | 2.2e-05 / 7.9e-07 / 4.1e-09 | 7e-08 / 7e-08 / 7e-08 |
| 128 | 7.81 | 1.8e+01 / 1.4e+01 / 1.0e-02 | 7.4e-01 / 2.3e-01 / 6.6e-04 | 8.6e-04 / 4.5e-05 / 1.1e-07 | 1.4e-06 / 4.9e-08 / 2.1e-08 | 8e-08 / 7e-08 / 6e-08 |

**bracket**

| n across | h (mm) | (a) cubes | (b) sampled fill | (c) fill + centroid | (d) + second moment | (e) ‖Δφ‖/σ |
|---|---|---|---|---|---|---|
| 4 | 250 | 9.6e+03 / 6.3e+03 / 1.1e+01 | 2.1e+03 / 6.4e+02 / 2.5e+00 | 2.5e+01 / 1.3e+00 / 1.8e-03 | 1.2e+00 / 6.9e-02 / 1.3e-05 | 8e-08 / 8e-08 / 8e-08 |
| 8 | 125 | 5.5e+03 / 4.7e+03 / 1.7e+00 | 7.8e+02 / 1.6e+02 / 1.0e+00 | 5.0e+00 / 1.1e-01 / 3.5e-04 | 4.0e-02 / 2.3e-03 / 7.5e-07 | 9e-08 / 8e-08 / 8e-08 |
| 16 | 62.5 | 2.2e+03 / 1.9e+03 / 9.3e-01 | 2.5e+02 / 7.9e+01 / 3.1e-01 | 7.8e-01 / 3.0e-02 / 2.9e-05 | 3.7e-03 / 1.0e-04 / 7.4e-08 | 8e-08 / 7e-08 / 7e-08 |
| 32 | 31.2 | 1.4e+03 / 1.2e+03 / 8.3e-01 | 3.1e+02 / 4.5e+01 / 3.8e-01 | 1.2e-01 / 4.7e-03 / 5.2e-06 | 9.6e-05 / 4.1e-06 / 9.9e-09 | 7e-08 / 8e-08 / 6e-08 |
| 64 | 15.6 | 5.1e+02 / 4.4e+02 / 3.3e-01 | 5.7e+01 / 1.5e+01 / 5.4e-02 | 1.5e-02 / 3.3e-04 / 5.5e-07 | 4.7e-06 / 1.9e-07 / 1.0e-08 | 7e-08 / 7e-08 / 7e-08 |
| 128 | 7.81 | 2.3e+02 / 3.4e+01 / 2.6e-01 | 4.3e+01 / 1.1e+01 / 2.9e-02 | 1.2e-03 / 4.5e-05 / 6.3e-08 | 3.4e-07 / 1.9e-08 / 1.4e-08 | 7e-08 / 7e-08 / 6e-08 |

## Physical uncertainties: peak |Δφ| (rad) / max |bias|/σ — 1.5 / 5 / 20 m

| body | change | 1.5 m | 5 m | 20 m |
|---|---|---|---|---|
| cube | density 1% | 6.5e-01 / 2.2e+03 | 4.7e-02 / 1.7e+03 | 4.3e-03 / 0.063 |
| cube | position 1 mm x | 2.1e-02 / 6e+02 | 4.7e-04 / 23 | 1.7e-05 / 1.4 |
| cube | position 1 mm y | 1.0e-01 / 3.4e+02 | 4.6e-04 / 13 | 5.7e-05 / 0.00084 |
| cube | position 1 mm z | 3.7e-02 / 6e+02 | 5.6e-03 / 1.8e+02 | 3.2e-05 / 0.0079 |
| cube | position 1 cm x | 2.1e-01 / 6e+03 | 4.7e-03 / 2.3e+02 | 1.7e-04 / 14 |
| cube | position 1 cm y | 1.0e+00 / 3.4e+03 | 4.6e-03 / 1.3e+02 | 5.7e-04 / 0.0084 |
| cube | position 1 cm z | 3.7e-01 / 6e+03 | 5.6e-02 / 1.8e+03 | 3.2e-04 / 0.079 |
| cube | orientation 1 deg about z | 4.7e-04 / 5.1 | 1.5e-05 / 0.054 | 6.7e-09 / 0.00026 |
| cube | orientation 1 deg about x | 1.8e-02 / 69 | 4.5e-05 / 1.2 | 2.7e-08 / 0.00021 |
| cube | inhomogeneity 10%/10% near side (min y) | 1.2e+00 / 2.2e+03 | 4.7e-02 / 1.7e+03 | 4.6e-03 / 0.27 |
| cube | inhomogeneity 10%/10% far side (max y) | 2.9e-01 / 2.2e+03 | 4.7e-02 / 1.7e+03 | 4.1e-03 / 0.24 |
| cube | inhomogeneity 10%/10% leading end (max x) | 6.3e-01 / 2.7e+03 | 4.7e-02 / 1.7e+03 | 4.3e-03 / 6.2 |
| cube | inhomogeneity 10%/10% core (nearest centroid) | 6.5e-01 / 2.2e+03 | 4.7e-02 / 1.7e+03 | 4.3e-03 / 0.063 |
| bunny | density 1% | 1.5e-01 / 3.9e+02 | 8.0e-03 / 3.4e+02 | 8.6e-04 / 0.017 |
| bunny | position 1 mm x | 5.0e-03 / 1.4e+02 | 7.9e-05 / 4.1 | 3.4e-06 / 0.14 |
| bunny | position 1 mm y | 2.2e-02 / 56 | 1.5e-04 / 3.7 | 1.1e-05 / 0.00023 |
| bunny | position 1 mm z | 4.7e-03 / 1.1e+02 | 1.1e-03 / 34 | 6.6e-06 / 0.0019 |
| bunny | position 1 cm x | 5.0e-02 / 1.4e+03 | 7.9e-04 / 41 | 3.4e-05 / 1.4 |
| bunny | position 1 cm y | 2.2e-01 / 5.6e+02 | 1.5e-03 / 37 | 1.1e-04 / 0.0023 |
| bunny | position 1 cm z | 4.7e-02 / 1.1e+03 | 1.1e-02 / 3.4e+02 | 6.6e-05 / 0.019 |
| bunny | orientation 1 deg about z | 3.8e-03 / 96 | 1.9e-04 / 4.7 | 6.5e-07 / 0.014 |
| bunny | orientation 1 deg about x | 3.7e-03 / 49 | 2.1e-04 / 5.5 | 7.1e-07 / 0.0084 |
| bunny | inhomogeneity 10%/10% near side (min y) | 2.1e-01 / 3.9e+02 | 8.0e-03 / 3.4e+02 | 8.9e-04 / 0.033 |
| bunny | inhomogeneity 10%/10% far side (max y) | 1.2e-01 / 3.7e+02 | 7.7e-03 / 3.3e+02 | 8.3e-04 / 0.019 |
| bunny | inhomogeneity 10%/10% leading end (max x) | 1.3e-01 / 4.8e+02 | 1.0e-02 / 4.1e+02 | 8.7e-04 / 0.53 |
| bunny | inhomogeneity 10%/10% core (nearest centroid) | 1.6e-01 / 3.9e+02 | 8.0e-03 / 3.4e+02 | 8.6e-04 / 0.019 |
| bracket | density 1% | 6.3e-02 / 1.5e+02 | 3.1e-03 / 1.3e+02 | 2.9e-04 / 0.0049 |
| bracket | position 1 mm x | 2.1e-03 / 55 | 3.1e-05 / 1.6 | 1.2e-06 / 0.064 |
| bracket | position 1 mm y | 9.8e-03 / 22 | 3.0e-05 / 0.97 | 3.9e-06 / 6.5e-05 |
| bracket | position 1 mm z | 1.9e-03 / 46 | 3.8e-04 / 13 | 2.2e-06 / 0.00055 |
| bracket | position 1 cm x | 2.1e-02 / 5.5e+02 | 3.1e-04 / 16 | 1.2e-05 / 0.64 |
| bracket | position 1 cm y | 9.8e-02 / 2.2e+02 | 3.0e-04 / 9.7 | 3.9e-05 / 0.00065 |
| bracket | position 1 cm z | 1.9e-02 / 4.6e+02 | 3.8e-03 / 1.3e+02 | 2.2e-05 / 0.0055 |
| bracket | orientation 1 deg about z | 1.7e-02 / 81 | 9.2e-05 / 2.1 | 5.6e-07 / 0.0079 |
| bracket | orientation 1 deg about x | 3.7e-03 / 18 | 1.3e-04 / 3.8 | 3.7e-07 / 0.0052 |
| bracket | inhomogeneity 10%/10% near side (min y) | 1.1e-01 / 2.9e+02 | 3.2e-03 / 1.3e+02 | 3.1e-04 / 0.26 |
| bracket | inhomogeneity 10%/10% far side (max y) | 4.3e-02 / 1.5e+02 | 3.2e-03 / 1.3e+02 | 2.9e-04 / 0.12 |
| bracket | inhomogeneity 10%/10% leading end (max x) | 1.1e-01 / 3.3e+02 | 3.1e-03 / 1.2e+02 | 3.1e-04 / 0.29 |
| bracket | inhomogeneity 10%/10% core (nearest centroid) | 6.0e-02 / 1.5e+02 | 3.1e-03 / 1.3e+02 | 2.9e-04 / 0.0064 |
| sphere | facet: 5120-triangle icosphere | 3.2e-01 / 2e+03 | 4.2e-02 / 6.8e+02 | 3.7e-03 / 0.049 |

## Decimation (static, centre of mass to axis; from the shapes study)

| scan | 1.5 m | 5 m | 20 m |
|---|---|---|---|
| dragon | 1.1e-03 rad (-7.2e-04) | 4.8e-04 rad (-7.1e-04) | 1.9e-05 rad (-7.1e-04) |
| buddha | 6.2e-04 rad (-4.2e-04) | 2.2e-04 rad (-3.9e-04) | 8.8e-06 rad (-3.9e-04) |
| armadillo | 1.3e-03 rad (-6.2e-04) | 4.7e-04 rad (-5.7e-04) | 1.9e-05 rad (-5.7e-04) |
