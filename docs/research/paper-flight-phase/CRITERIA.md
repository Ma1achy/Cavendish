# Pass criteria, written before measuring (27 Nov 2026)

Committed before any number below was computed. The note (`../paper-flight-phase.md`) reports each
against these, and says so where one fails; none is moved after the fact.

Three reads of one shot: **flight** (Cavendish's flight integral, the source moving during the shot),
**frozen** (the same integral with the source frozen at the π pulse, launch + T), **leading** (the
paper's k_eff T² Δg_z at the two launch points, one instant). The *temporal* error of the
instantaneous approximation is flight − frozen; the *spatial* error is frozen − leading; the paper's
total error is flight − leading.

- **C1, the Python is Cavendish.** `flight.py` and `cav run` agree to ≤ 1e-10 rad at every readout,
  on two moving point-mass scenes (a walk past at 1.4 m/s, 2 m; a 1 t pass at 10 m/s, 5 m). Control
  arm: the frozen read differs from `cav run` by ≥ 1e-6 rad on both, so the comparison can fail.
- **C2, transfer function.** For the symmetric launch, no floor, instant kicks, the numerical
  |H(f)|/H(0) from the arms equals sinc²(πfT) to 1e-9 for f ≤ 10 Hz (Cheinet et al. 2008, eq. 9 with
  H_a = k_eff H/ω²). Control: with T changed by 1 % it fails. With the engine's u₀ = 3.86 m/s and
  floor, the departure is reported, not judged.
- **C3, the near-field transfer.** A point mass whose mass is modulated at f, at horizontal distance
  d level with the lower launch: for d = 50 m the response matches sinc²(πfT) to 1 % for f ≤ 2 Hz.
  At d = 1.5–10 m it is measured and reported (no criterion: this is the finding).
- **C4, the scaling law.** For point-mass passes the temporal error at the peak is predicted by
  −φ''(t)T²/12 (from sinc² ≈ 1 − (ωT)²/12) to within a factor 1.5 wherever it exceeds 1e-3 of
  the peak. If it does not, the 1 % / 10 % map is drawn from the measurement alone and the law is
  not quoted.
- **C5, walking past.** The paper's "peaks differing by 4–18 % between poses" is taken to survive if,
  with the flight read, every body's largest pose difference over its peak stays within a factor 2 of
  the leading read's. The peak values themselves are reported for flight, frozen and leading.
- **C6, torus lines.** World-fixed voxels, (c) fill at the centroid, at 1 m/s: the flight read's
  line at 1/h (10 Hz) equals the leading read's line times |H(10 Hz)|/H(0) to within a factor 2 (the
  separable estimate). Body-frame voxels: no line above 1e-3 of the world-frame line.
- **C7, cut-cell error on a moving body.** Body-frame (d) error of the torus pass, flight read: its
  ratio to the signal at the peak is within a factor 2 of the leading read's ratio (so the paper's
  relative accuracy claims carry over).
