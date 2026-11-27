# How accurate must the boundary representation be? An error budget judged by inference

(Draft: pass criteria written 27 Nov 2026 before any measurement; results follow.)

## Pass criteria, written before measuring

Scene and method are in `paper-error-budget/budget.py`'s docstring. The criteria:

- **V1, known-answer bias.** A residual that is exactly a parameter change must come back as that
  change: a 1 % density error gives δ(ln M) = 0.0100 with |bias|/σ < 10⁻⁶ on x₀, y₀ and v; a 1 cm
  shift along x, taken linearly, gives δx₀ = 1.000 cm with the others < 10⁻⁶ σ. Applied as the exact
  (non-linear) 1 cm shift it must give δx₀ = 1 cm within 1 % at every distance (the linearisation holds
  at the scale of the errors studied). **Control arm:** the exact 30 cm shift at 1.5 m must miss
  30 cm by more than 1 % (the linear formula is not trusted beyond its range).
- **V2, the Fisher matrix against a closed form.** For the analytic sphere (a point mass outside), σ
  from finite-difference Jacobians (1 mm step) must agree with σ from the analytic derivative of the
  point mass's k_eff T² Δg_z to 10⁻⁵ relative for every parameter and distance. **Control arm:** a
  30 cm step must disagree by more than 10⁻³ somewhere.
- **V3, the bound.** Every |bias_i|/σ_i ≤ ‖Δφ‖/σ (Cauchy–Schwarz). An identity; a failure is a bug.
- **V4, linear bias against a non-linear fit.** For the bunny at 1/32 m, 1.5 m, techniques (b) and (c):
  Gauss–Newton fitting the technique's own model (its voxels, moved with the body) to the noiseless
  exact signal must give a bias within 10 % (or 0.02 σ, whichever is larger) of the linear formula.
- **V5, the reference is good enough.** The clipped pieces plus interior cubes must reproduce the
  mesh with ‖Δφ‖/σ < 10⁻² at every pitch and distance; otherwise a technique's residual below that
  level is not resolved, and is reported as "at the reference floor".

Criterion for the answer ("the pitch at which bias/σ < 0.1"): the largest of the four |bias_i|/σ_i,
worst of three random lattice placements, below 0.1 at that pitch and every finer one measured;
interpolated log–log between measured pitches; beyond 1/128 m extrapolated with the order fitted over
the last three pitches and marked as extrapolated.
