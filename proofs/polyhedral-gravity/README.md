# Polyhedral gravity in Lean 4

A machine-checked derivation of the closed-form gravity of a uniform polyhedron, as Cavendish uses
it (Werner & Scheeres 1997 face by face, with the numerical rules of
[`docs/research/exact-skin-and-partial-fill.md`](../../docs/research/exact-skin-and-partial-fill.md)
§1.1), written for the paper in [`docs/papers/polyhedral-gravity/`](../../docs/papers/polyhedral-gravity/).

Self-contained Lake project, pinned: Lean `v4.36.0` (`lean-toolchain`), Mathlib `v4.36.0`
(`lakefile.toml`, `lake-manifest.json`).

```text
lake exe cache get     # Mathlib's prebuilt .olean files; do not build Mathlib
lake build             # below normal priority on the owner's PC, one build at a time
```

It is not part of the Cargo workspace and no `cargo xtask` tier looks at it; nothing needs
excluding.

## No `sorry`, no `axiom`

`lake build` finishes with no `sorry` warning, and `#print axioms` on the main theorems lists only
Lean's three standard axioms (`propext`, `Classical.choice`, `Quot.sound`). What is not proven is an
**explicit hypothesis** of the theorem that needs it, listed below with why.

## What is proven

| file | statement | Lean name |
|---|---|---|
| `Edge.lean` | (a) ∫₀ᴸ dt/‖p − (a + t d)‖ = ln((r₁+r₂+L)/(r₁+r₂−L)) for ‖d‖ = 1 and p off the line, in any real inner-product space, by the fundamental theorem of calculus from the antiderivative ln(t − t₀ + r(t)) | `edge_integral` (vector), `integral_inv_lineDist` (line coordinates) |
| | the log1p form ln(1 + 2L/(r₁+r₂−L)) | `edge_integral_log1p` |
| | the near-edge form r₁+r₂−L = r₀²/(r₁−ℓ₁) + r₀²/(r₂+ℓ₂) | `gap_near_edge_vec`, `gap_near_edge` |
| `SolidAngle.lean` | (b) the numerator rule r₁·(r₂×r₃) = h·2A (h from the unit normal, 2A = ‖(v₂−v₁)×(v₃−v₁)‖) | `triple_eq_edges`, `numerator_rule` |
| | the height is the same from every vertex | `height_const` |
| | D² + N² = 2(r₁r₂+r₁·r₂)(r₂r₃+r₂·r₃)(r₃r₁+r₃·r₁), each factor ≥ 0 (Cauchy–Schwarz), so (D, N) ≠ (0, 0) off the closed edges | `vos_modulus`, `len_mul_add_dot_nonneg`, `vos_ne_zero` |
| `Divergence.lean` | divergences with explicit derivatives: div((q−p)/2r) = 1/r; div((q−p)/r³) = 0; in a plane div((x−x_p)/R) = 1/R + h²/R³; div(e_k/r) = −(q−p)_k/r³ | `potField_div`, `gaussField_div`, `faceField_div`, `attrField_div` |
| `Box.lean` | (c) for a box [a, b] ⊂ ℝ³ and p outside it, **with no hypotheses** (Mathlib's divergence theorem on boxes): ∫ dV/r = ½ Σ_faces h_f ∫_f dS/r; ∫ (q−p)_k/r³ dV = −(∫_{x_k=b_k} − ∫_{x_k=a_k}) dS/r; the six solid angles sum to zero | `box_potential`, `box_attraction`, `box_gauss` |
| `BoxFaces.lean` | for a rectangle [c, d] ⊂ ℝ² at height h, **with no hypotheses** (Mathlib's divergence theorem in the plane): ∫ dS/R = Σ_edges (ν·r_e) ∫_e ds/R − h ∫ h/R³ dS; each edge (a box of `Fin 1 → ℝ` in Mathlib's form) as an interval integral, and in closed form ln((r₁+r₂+ℓ)/(r₁+r₂−ℓ)). Substituting these into `box_potential` (inserting a face's coordinates into the box's) is not written out as one theorem; it is index bookkeeping | `rect_face`, `integral_Icc_fin_one`, `integral_inv_lineDist'`, `rect_edge` |
| `Polyhedron.lean` | (c) for a general polyhedron, **given** the divergence theorem for V and Green's theorem on each face: U = ½ Σ_f h_f (Σ_e (ν_e·r_e) L_e − h_f ω_f); g = −Σ_f n_f (…); Σ_f ω_f = 0 outside | `potential_faces_edges`, `attraction_faces_edges`, `solid_angles_sum_zero`, and one face: `face_integral` |
| `Superposition.lean` | (d) cells that partition the body: the body's integral is the sum over pieces (interior cells whole, boundary cells clipped); the phase κ∫₀^{2T}[V(x_u) − V(x_l)]dt and its quadrature are linear in V; the quadrature error of the body is the sum of the pieces'; exact pieces give the exact quadrature phase | `integral_pieces`, `interior_and_boundary`, `phase_sum`, `phaseQ_sum`, `quadrature_error_additive`, `exact_pieces_exact_phase`, `gradiometer_sum` |

## What is assumed, and why

| assumption | where | why it is not proven |
|---|---|---|
| **The divergence theorem for a general polyhedron** (`DivergenceTheorem V K F`): for every field differentiable on an open set containing a compact K ⊇ V, with integrable divergence, ∫_V div Φ = Σ_f ∫_{D_f} n_f·Φ | hypothesis of the three `Polyhedron.lean` theorems | Mathlib proves the divergence theorem for boxes only. For the box it is discharged (`Box.lean`). Stated over *all* such fields, so it is the classical theorem, not something shaped to these fields. |
| **Green's theorem on each face's polygon** (`GreenTheorem f`) | hypothesis of `face_integral` and the body theorems | as above, in two dimensions (Mathlib: rectangles). |
| Each face's frame (u, w, n) and each edge's (d, ν) satisfy Parseval: \|v\|² = Σ (basis·v)² | fields `Face.parseval`, `Edge2.parseval` | equivalent to orthonormality of a square frame; a property of the data, kept as a field to keep the files short. |
| The point is outside a compact set holding the body and its faces, and off the line of every edge | `Setting` | the closed form is proven where the integrands are smooth. Points on an edge's extension (finite L_e, r₀ = 0) and on the surface are not covered. |
| ω_f is **defined** as ∫_f h/r³ dS (W&S eq. 4) | `Face.ω` | the Van Oosterom–Strackee closed form ω = 2 atan2(N, D) is cited, not proven; only its algebraic ingredients (numerator rule, modulus identity) are. |
| Σ_f ω_f = 4π inside the body | not stated | needs the divergence theorem on V minus a small ball; out of scope. |
| g is taken as the attraction integral ∫ (q−p)/r³ dV | `attraction_faces_edges`, `box_attraction` | that this equals ∇U (differentiation under the integral sign) is standard and not formalised. |

## Notes

- Mathlib's norm on `Fin n → ℝ` is the sup norm, so distances in `Divergence.lean`, `Box.lean`
  and `Polyhedron.lean` are written out as `√(Σⱼ (qⱼ − pⱼ)²)` (`edist`, `sqDist`). `Edge.lean` is
  stated in any real inner-product space, so it applies to `EuclideanSpace ℝ (Fin 3)` directly.
- Build at below-normal priority on the owner's PC:
  `$p = Start-Process lake -ArgumentList build -NoNewWindow -PassThru; $p.PriorityClass = 'BelowNormal'; $p.WaitForExit()`.
