/-
(c) A general uniform polyhedron: from the volume integral to faces and edges, with the
divergence theorem as an explicit hypothesis.

Data. A body `V ⊆ ℝ³` and a finite family of faces. Each face is a compact region `D ⊆ ℝ²` placed
in space by an orthonormal frame `(u, w, n)` at an origin `o` (`n` the outward normal):
`pt x = o + x₀ u + x₁ w`. Its edges are segments `a + t d`, `t ∈ [0, ℓ]`, in the face's
coordinates, with `ν` the outward normal in the plane.

Hypotheses, stated as `Prop`s and passed explicitly (nothing is an `axiom`):
* `DivergenceTheorem V K faces`: Gauss's theorem for `V`, for every field differentiable on an open
  set containing the compact `K ⊇ V` with integrable divergence:
  `∫_V div Φ = Σ_f ∫_{D_f} n_f · Φ(pt_f x) dx`.
* `GreenTheorem f`: the same in the plane for each face: `∫_D div G = Σ_e ∫₀^ℓ ν_e · G(a_e + t d_e) dt`.
These are the classical theorems for polyhedral domains and polygons (Mathlib has them for boxes
and rectangles only; `Box.lean` uses those). Everything else is proven:

* `potential_faces_edges` — Werner & Scheeres (1997) eq. 10, face by face:
  `∫_V dV/|q − p| = ½ Σ_f h_f (Σ_e (ν_e·r_e) L_e − h_f ω_f)`;
* `attraction_faces_edges` — their eq. 15: `∫_V (q − p)/|q − p|³ dV = −Σ_f n_f (Σ_e (ν_e·r_e) L_e − h_f ω_f)`;
* `solid_angles_sum_zero` — their eq. 17 outside the body: `Σ_f ω_f = 0`,
where `h_f = n_f·(o_f − p)` is the signed height of the face's plane above `p`, `ν_e·r_e` the
in-plane distance from `p`'s projection to the edge's line, `L_e = ln((r₁ + r₂ + ℓ)/(r₁ + r₂ − ℓ))`
with `r₁, r₂` the distances from `p` to the edge's ends, and `ω_f = ∫_{D_f} h_f/|q − p|³ dS` the
signed solid angle (Werner & Scheeres eq. 4, taken as its definition here; its closed form is
Van Oosterom & Strackee's, see `SolidAngle.lean`).

Assumed about the point: it lies outside the compact `K` that holds the body and its faces, and off
the line of every edge (the case of a point on an edge's extension, where `L_e` is still finite, is
not covered). The value inside (`Σ ω_f = 4π`) is not proven here.
-/
import Mathlib
import PolyhedralGravity.Edge
import PolyhedralGravity.Divergence

open MeasureTheory Matrix

namespace PolyhedralGravity

/-- An edge of a face, in the face's own 2-D coordinates. -/
structure Edge2 where
  /-- start of the segment -/
  a : Fin 2 → ℝ
  /-- unit direction -/
  d : Fin 2 → ℝ
  /-- length, metres -/
  ℓ : ℝ
  /-- outward unit normal, in the plane -/
  ν : Fin 2 → ℝ
  hdd : d ⬝ᵥ d = 1
  hνd : ν ⬝ᵥ d = 0
  /-- `(d, ν)` is an orthonormal basis of the plane: `|v|² = (d·v)² + (ν·v)²`. -/
  parseval : ∀ v : Fin 2 → ℝ, v ⬝ᵥ v = (d ⬝ᵥ v) ^ 2 + (ν ⬝ᵥ v) ^ 2

/-- A planar face: a compact region `D` of ℝ² placed in ℝ³ by an orthonormal frame. -/
structure Face where
  o : Fin 3 → ℝ
  u : Fin 3 → ℝ
  w : Fin 3 → ℝ
  /-- outward unit normal -/
  n : Fin 3 → ℝ
  huu : u ⬝ᵥ u = 1
  hww : w ⬝ᵥ w = 1
  huw : u ⬝ᵥ w = 0
  hun : n ⬝ᵥ u = 0
  hwn : n ⬝ᵥ w = 0
  /-- `(u, w, n)` is an orthonormal basis of ℝ³: `|v|² = (u·v)² + (w·v)² + (n·v)²`. -/
  parseval : ∀ v : Fin 3 → ℝ, v ⬝ᵥ v = (u ⬝ᵥ v) ^ 2 + (w ⬝ᵥ v) ^ 2 + (n ⬝ᵥ v) ^ 2
  D : Set (Fin 2 → ℝ)
  hD : IsCompact D
  /-- the edges, indexed by a finite type -/
  E : Type
  [fE : Fintype E]
  edge : E → Edge2

attribute [instance] Face.fE

namespace Face

variable (f : Face)

/-- The point of space at face coordinates `x`. -/
def pt (x : Fin 2 → ℝ) : Fin 3 → ℝ := f.o + x 0 • f.u + x 1 • f.w

/-- The signed height of the face's plane above `p` (Werner & Scheeres' `Δz = n_f · r_f`). -/
def h (p : Fin 3 → ℝ) : ℝ := f.n ⬝ᵥ (f.o - p)

/-- The face coordinates of `p`'s projection onto the plane. -/
def xp (p : Fin 3 → ℝ) : Fin 2 → ℝ := ![f.u ⬝ᵥ (p - f.o), f.w ⬝ᵥ (p - f.o)]

lemma n_dot_pt_sub (p : Fin 3 → ℝ) (x : Fin 2 → ℝ) : f.n ⬝ᵥ (f.pt x - p) = f.h p := by
  simp only [pt, h, dotProduct_sub, dotProduct_add, dotProduct_smul, f.hun, f.hwn, smul_eq_mul,
    mul_zero, add_zero]

/-- Distances from `p` to the face, in the face's coordinates: `|pt x − p|² = |x − x_p|² + h²`. -/
lemma sqDist_pt (p : Fin 3 → ℝ) (x : Fin 2 → ℝ) :
    sqDist p 0 (f.pt x) = sqDist (f.xp p) (f.h p ^ 2) x := by
  have hv : sqDist p 0 (f.pt x) = (f.pt x - p) ⬝ᵥ (f.pt x - p) := by
    simp [sqDist, dotProduct, sq]
  rw [hv, f.parseval, n_dot_pt_sub]
  have hu : f.u ⬝ᵥ (f.pt x - p) = x 0 - f.u ⬝ᵥ (p - f.o) := by
    simp only [pt, dotProduct_sub, dotProduct_add, dotProduct_smul, smul_eq_mul, f.huu, f.huw]
    ring
  have hw : f.w ⬝ᵥ (f.pt x - p) = x 1 - f.w ⬝ᵥ (p - f.o) := by
    have : f.w ⬝ᵥ f.u = 0 := by rw [dotProduct_comm]; exact f.huw
    simp only [pt, dotProduct_sub, dotProduct_add, dotProduct_smul, smul_eq_mul, f.hww, this]
    ring
  rw [hu, hw]
  simp [sqDist, xp, Fin.sum_univ_two]

end Face

/-- **Hypothesis: the divergence theorem for `V`** with outward faces `F`. -/
def DivergenceTheorem {ι : Type} [Fintype ι] (V K : Set (Fin 3 → ℝ)) (F : ι → Face) : Prop :=
  ∀ (U : Set (Fin 3 → ℝ)), IsOpen U → K ⊆ U →
  ∀ (Φ : (Fin 3 → ℝ) → (Fin 3 → ℝ)) (Φ' : (Fin 3 → ℝ) → (Fin 3 → ℝ) →L[ℝ] (Fin 3 → ℝ)),
    (∀ q ∈ U, HasFDerivAt Φ (Φ' q) q) →
    IntegrableOn (fun q => ∑ i, Φ' q (Pi.single i 1) i) V →
    ∫ q in V, ∑ i, Φ' q (Pi.single i 1) i =
      ∑ j, ∫ x in (F j).D, (F j).n ⬝ᵥ Φ ((F j).pt x)

/-- **Hypothesis: the divergence (Green's) theorem on a face's polygon** with its edges. -/
def GreenTheorem (f : Face) : Prop :=
  ∀ (U : Set (Fin 2 → ℝ)), IsOpen U → f.D ⊆ U →
  ∀ (G : (Fin 2 → ℝ) → (Fin 2 → ℝ)) (G' : (Fin 2 → ℝ) → (Fin 2 → ℝ) →L[ℝ] (Fin 2 → ℝ)),
    (∀ x ∈ U, HasFDerivAt G (G' x) x) →
    IntegrableOn (fun x => ∑ i, G' x (Pi.single i 1) i) f.D →
    ∫ x in f.D, ∑ i, G' x (Pi.single i 1) i =
      ∑ e, ∫ t in (0 : ℝ)..(f.edge e).ℓ, (f.edge e).ν ⬝ᵥ G ((f.edge e).a + t • (f.edge e).d)

/-- The edge logarithm `L_e = ln((r₁ + r₂ + ℓ)/(r₁ + r₂ − ℓ))`, from the distances to the ends. -/
noncomputable def edgeLog (r₁ r₂ ℓ : ℝ) : ℝ := Real.log ((r₁ + r₂ + ℓ) / (r₁ + r₂ - ℓ))

/-- The signed solid angle of a face seen from `p`, as Werner & Scheeres define it (eq. 4):
`ω_f = ∫_D h/|q − p|³ dS`. -/
noncomputable def Face.ω (f : Face) (p : Fin 3 → ℝ) : ℝ :=
  ∫ x in f.D, f.h p / (sqDist p 0 (f.pt x) * Real.sqrt (sqDist p 0 (f.pt x)))

/-- The face's edge sum `Σ_e (ν_e·r_e) L_e`. -/
noncomputable def Face.edgeSum (f : Face) (p : Fin 3 → ℝ) : ℝ :=
  ∑ e, (f.edge e).ν ⬝ᵥ ((f.edge e).a - f.xp p) *
    edgeLog (edist p (f.pt (f.edge e).a))
      (edist p (f.pt ((f.edge e).a + (f.edge e).ℓ • (f.edge e).d))) (f.edge e).ℓ

/-! ### One edge -/

/-- Along an edge, the distance from `p` in line coordinates. -/
lemma edge_lineDist (f : Face) (e : Edge2) (p : Fin 3 → ℝ) (t : ℝ) :
    Real.sqrt (sqDist (f.xp p) (f.h p ^ 2) (e.a + t • e.d)) =
      lineDist (e.d ⬝ᵥ (f.xp p - e.a)) ((e.ν ⬝ᵥ (e.a - f.xp p)) ^ 2 + f.h p ^ 2) t := by
  unfold lineDist
  congr 1
  have hv : sqDist (f.xp p) (f.h p ^ 2) (e.a + t • e.d) =
      (e.a + t • e.d - f.xp p) ⬝ᵥ (e.a + t • e.d - f.xp p) + f.h p ^ 2 := by
    simp [sqDist, dotProduct, Fin.sum_univ_two, sq]
  rw [hv, e.parseval]
  have h1 : e.d ⬝ᵥ (e.a + t • e.d - f.xp p) = t - e.d ⬝ᵥ (f.xp p - e.a) := by
    simp only [dotProduct_sub, dotProduct_add, dotProduct_smul, smul_eq_mul, e.hdd]
    ring
  have h2 : e.ν ⬝ᵥ (e.a + t • e.d - f.xp p) = e.ν ⬝ᵥ (e.a - f.xp p) := by
    simp only [dotProduct_sub, dotProduct_add, dotProduct_smul, smul_eq_mul, e.hνd]
    ring
  rw [h1, h2]
  ring

/-- **One edge's line integral** of the face field `(x − x_p)/R`: `(ν·r_e) L_e`. -/
lemma edge_term (f : Face) (e : Edge2) (p : Fin 3 → ℝ)
    (hoff : 0 < (e.ν ⬝ᵥ (e.a - f.xp p)) ^ 2 + f.h p ^ 2) :
    ∫ t in (0 : ℝ)..e.ℓ, e.ν ⬝ᵥ radialField (f.xp p) (f.h p ^ 2) (fun s => 1 / Real.sqrt s)
        (e.a + t • e.d) =
      e.ν ⬝ᵥ (e.a - f.xp p) *
        edgeLog (edist p (f.pt e.a)) (edist p (f.pt (e.a + e.ℓ • e.d))) e.ℓ := by
  have hpt : ∀ t : ℝ, e.ν ⬝ᵥ radialField (f.xp p) (f.h p ^ 2) (fun s => 1 / Real.sqrt s)
      (e.a + t • e.d) =
      e.ν ⬝ᵥ (e.a - f.xp p) *
        (1 / lineDist (e.d ⬝ᵥ (f.xp p - e.a)) ((e.ν ⬝ᵥ (e.a - f.xp p)) ^ 2 + f.h p ^ 2) t) := by
    intro t
    rw [← edge_lineDist]
    have h2 : e.ν ⬝ᵥ (e.a + t • e.d - f.xp p) = e.ν ⬝ᵥ (e.a - f.xp p) := by
      simp only [dotProduct_sub, dotProduct_add, dotProduct_smul, smul_eq_mul, e.hνd]
      ring
    rw [← h2]
    simp only [radialField, dotProduct, Fin.sum_univ_two, Pi.sub_apply, Pi.add_apply]
    ring
  simp_rw [hpt]
  rw [intervalIntegral.integral_const_mul, integral_inv_lineDist hoff]
  congr 1
  have h0 := edge_lineDist f e p 0
  rw [zero_smul, add_zero] at h0
  unfold edgeLog edist
  rw [f.sqDist_pt, f.sqDist_pt, edge_lineDist, h0]

/-! ### One face -/

/-- **One face** (Werner & Scheeres eq. 8): `∫_D dS/R = Σ_e (ν_e·r_e) L_e − h ω`, given Green's
theorem on the face, `p` off the face, and `p` off every edge's line. -/
theorem face_integral (f : Face) (p : Fin 3 → ℝ) (hG : GreenTheorem f)
    (hp : ∀ x ∈ f.D, f.pt x ≠ p)
    (hoff : ∀ e, 0 < ((f.edge e).ν ⬝ᵥ ((f.edge e).a - f.xp p)) ^ 2 + f.h p ^ 2) :
    ∫ x in f.D, 1 / edist p (f.pt x) = f.edgeSum p - f.h p * f.ω p := by
  set c := f.h p ^ 2
  set xp := f.xp p
  -- the open set where the field is smooth
  set U : Set (Fin 2 → ℝ) := {x | 0 < sqDist xp c x}
  have hS : Continuous fun x : Fin 2 → ℝ => sqDist xp c x := by unfold sqDist; fun_prop
  have hUo : IsOpen U := isOpen_lt continuous_const hS
  have hpos : ∀ x ∈ f.D, 0 < sqDist xp c x := by
    intro x hx
    rw [← f.sqDist_pt]
    exact sqDist_zero_pos (hp x hx)
  have hDU : f.D ⊆ U := fun x hx => hpos x hx
  -- the divergence, and that it is integrable on the compact face
  have hdiv : ∀ x ∈ U, ∑ i, radialDeriv xp c (fun s => 1 / Real.sqrt s)
      (-1 / (2 * sqDist xp c x * Real.sqrt (sqDist xp c x))) x (Pi.single i 1) i =
      1 / Real.sqrt (sqDist xp c x) + c / (sqDist xp c x * Real.sqrt (sqDist xp c x)) :=
    fun x hx => (faceField_div (h := f.h p) hx).2
  have hcont1 : ContinuousOn (fun x => 1 / Real.sqrt (sqDist xp c x)) f.D :=
    ContinuousOn.div (f := fun _ => (1 : ℝ)) (g := fun x => Real.sqrt (sqDist xp c x))
      continuousOn_const hS.sqrt.continuousOn (fun x hx => (Real.sqrt_pos.2 (hpos x hx)).ne')
  have hcont2 : ContinuousOn (fun x => c / (sqDist xp c x * Real.sqrt (sqDist xp c x))) f.D :=
    ContinuousOn.div (f := fun _ => c)
      (g := fun x => sqDist xp c x * Real.sqrt (sqDist xp c x))
      continuousOn_const (hS.mul hS.sqrt).continuousOn
      (fun x hx => (mul_pos (hpos x hx) (Real.sqrt_pos.2 (hpos x hx))).ne')
  have hi1 := hcont1.integrableOn_compact (μ := volume) f.hD
  have hi2 := hcont2.integrableOn_compact (μ := volume) f.hD
  have hint : IntegrableOn (fun x => ∑ i, radialDeriv xp c (fun s => 1 / Real.sqrt s)
      (-1 / (2 * sqDist xp c x * Real.sqrt (sqDist xp c x))) x (Pi.single i 1) i) f.D := by
    refine (hi1.add hi2).congr_fun (fun x hx => (hdiv x (hDU hx)).symm) f.hD.measurableSet
  have green := hG U hUo hDU (radialField xp c (fun s => 1 / Real.sqrt s))
    (fun x => radialDeriv xp c (fun s => 1 / Real.sqrt s)
      (-1 / (2 * sqDist xp c x * Real.sqrt (sqDist xp c x))) x)
    (fun x hx => (faceField_div (h := f.h p) hx).1) hint
  rw [setIntegral_congr_fun f.hD.measurableSet (fun x hx => hdiv x (hDU hx)),
    integral_add hi1 hi2] at green
  -- the edges
  have hedges : ∀ e, ∫ t in (0 : ℝ)..(f.edge e).ℓ, (f.edge e).ν ⬝ᵥ
      radialField xp c (fun s => 1 / Real.sqrt s) ((f.edge e).a + t • (f.edge e).d) =
      (f.edge e).ν ⬝ᵥ ((f.edge e).a - xp) *
        edgeLog (edist p (f.pt (f.edge e).a))
          (edist p (f.pt ((f.edge e).a + (f.edge e).ℓ • (f.edge e).d))) (f.edge e).ℓ :=
    fun e => edge_term f (f.edge e) p (hoff e)
  have hsum : ∑ e, ∫ t in (0 : ℝ)..(f.edge e).ℓ, (f.edge e).ν ⬝ᵥ
      radialField xp c (fun s => 1 / Real.sqrt s) ((f.edge e).a + t • (f.edge e).d) =
      f.edgeSum p := by
    unfold Face.edgeSum
    exact Finset.sum_congr rfl (fun e _ => hedges e)
  rw [hsum] at green
  -- the solid angle
  have hω : ∫ x in f.D, c / (sqDist xp c x * Real.sqrt (sqDist xp c x)) = f.h p * f.ω p := by
    unfold Face.ω
    rw [← integral_const_mul]
    apply setIntegral_congr_fun f.hD.measurableSet
    intro x _
    simp only [f.sqDist_pt]
    ring
  have hlhs : ∫ x in f.D, 1 / edist p (f.pt x) = ∫ x in f.D, 1 / Real.sqrt (sqDist xp c x) := by
    apply setIntegral_congr_fun f.hD.measurableSet
    intro x _
    simp only [edist, f.sqDist_pt]
    rfl
  rw [hlhs]
  linarith [green, hω]

/-! ### The body -/

section Body

variable {ι : Type} [Fintype ι] (V K : Set (Fin 3 → ℝ)) (F : ι → Face) (p : Fin 3 → ℝ)

/-- The standing assumptions on the body and the point. -/
structure Setting : Prop where
  hK : IsCompact K
  hVK : V ⊆ K
  hV : MeasurableSet V
  /-- the point is outside `K` -/
  hpK : p ∉ K
  /-- every face point is in `K` -/
  hFK : ∀ j, ∀ x ∈ (F j).D, (F j).pt x ∈ K
  /-- the point is off every edge's line -/
  hoff : ∀ j e, 0 < (((F j).edge e).ν ⬝ᵥ (((F j).edge e).a - (F j).xp p)) ^ 2 + (F j).h p ^ 2

variable {V K F p}

omit [Fintype ι] in
lemma Setting.ne (S : Setting V K F p) : ∀ q ∈ K, q ≠ p := fun _ hq h => S.hpK (h ▸ hq)

omit [Fintype ι] in
lemma Setting.face_ne (S : Setting V K F p) : ∀ j, ∀ x ∈ (F j).D, (F j).pt x ≠ p :=
  fun j x hx => S.ne _ (S.hFK j x hx)

lemma isOpen_ne_point (p : Fin 3 → ℝ) : IsOpen {q : Fin 3 → ℝ | q ≠ p} := _root_.isOpen_ne

omit [Fintype ι] in
lemma continuousOn_inv_edist (S : Setting V K F p) :
    ContinuousOn (fun q => 1 / edist p q) K := by
  have hS : Continuous fun q : Fin 3 → ℝ => sqDist p 0 q := by unfold sqDist; fun_prop
  exact ContinuousOn.div (f := fun _ => (1 : ℝ)) (g := fun q => Real.sqrt (sqDist p 0 q))
    continuousOn_const hS.sqrt.continuousOn
    (fun q hq => (Real.sqrt_pos.2 (sqDist_zero_pos (S.ne q hq))).ne')

/-- The normal component of the radial field is the height times the profile. -/
lemma dot_radialField (n p y : Fin 3 → ℝ) (c : ℝ) (φ : ℝ → ℝ) :
    n ⬝ᵥ radialField p c φ y = (n ⬝ᵥ (y - p)) * φ (sqDist p c y) := by
  simp only [radialField, dotProduct, Fin.sum_univ_three, Pi.sub_apply]
  ring

lemma dot_constField (n p y : Fin 3 → ℝ) (c : ℝ) (w : Fin 3 → ℝ) (ψ : ℝ → ℝ) :
    n ⬝ᵥ constField p c w ψ y = (n ⬝ᵥ w) * ψ (sqDist p c y) := by
  simp only [constField, dotProduct, Fin.sum_univ_three]
  ring

/-- **The potential** (Werner & Scheeres eq. 10, face by face):
`∫_V dV/|q − p| = ½ Σ_f h_f (Σ_e (ν_e·r_e) L_e − h_f ω_f)`. -/
theorem potential_faces_edges (S : Setting V K F p) (hdiv : DivergenceTheorem V K F)
    (hG : ∀ j, GreenTheorem (F j)) :
    ∫ q in V, 1 / edist p q = ∑ j, (F j).h p / 2 * ((F j).edgeSum p - (F j).h p * (F j).ω p) := by
  have hi : IntegrableOn (fun q => 1 / edist p q) V :=
    ((continuousOn_inv_edist S).integrableOn_compact S.hK).mono_set S.hVK
  have hd := fun q (hq : q ∈ {q : Fin 3 → ℝ | q ≠ p}) => potField_div (p := p) hq
  have hint := hi.congr_fun (fun q hq => (hd q (S.ne q (S.hVK hq))).2.symm) S.hV
  have := hdiv {q | q ≠ p} (isOpen_ne_point p) (fun q hq => S.ne q hq) _ _ (fun q hq => (hd q hq).1) hint
  rw [setIntegral_congr_fun S.hV (fun q hq => (hd q (S.ne q (S.hVK hq))).2)] at this
  rw [this]
  apply Finset.sum_congr rfl
  intro j _
  rw [← face_integral (F j) p (hG j) (S.face_ne j) (S.hoff j), ← integral_const_mul]
  apply setIntegral_congr_fun (F j).hD.measurableSet
  intro x _
  simp only [potField, dot_radialField, (F j).n_dot_pt_sub, edist]
  ring

/-- **The attraction** (Werner & Scheeres eq. 15, face by face), component `k`:
`∫_V (q − p)_k/|q − p|³ dV = −Σ_f (n_f)_k (Σ_e (ν_e·r_e) L_e − h_f ω_f)`. -/
theorem attraction_faces_edges (S : Setting V K F p) (hdiv : DivergenceTheorem V K F)
    (hG : ∀ j, GreenTheorem (F j)) (k : Fin 3) :
    ∫ q in V, (q k - p k) / (sqDist p 0 q * edist p q) =
      -∑ j, (F j).n k * ((F j).edgeSum p - (F j).h p * (F j).ω p) := by
  have hcont : ContinuousOn (fun q => -(q k - p k) / (sqDist p 0 q * edist p q)) K := by
    refine ContinuousOn.div (f := fun q => -(q k - p k)) (g := fun q => sqDist p 0 q * edist p q)
      ?_ ?_ ?_
    · exact ((continuous_apply k).sub continuous_const).neg.continuousOn
    · have h1 : Continuous fun q : Fin 3 → ℝ => sqDist p 0 q := by unfold sqDist; fun_prop
      have h2 : Continuous fun q : Fin 3 → ℝ => edist p q := by unfold edist sqDist; fun_prop
      exact (h1.mul h2).continuousOn
    · intro q hq
      have h1 := sqDist_zero_pos (S.ne q hq)
      exact (mul_pos h1 (Real.sqrt_pos.2 h1)).ne'
  have hi : IntegrableOn (fun q => -(q k - p k) / (sqDist p 0 q * edist p q)) V :=
    (hcont.integrableOn_compact (μ := volume) S.hK).mono_set S.hVK
  have hd := fun q (hq : q ∈ {q : Fin 3 → ℝ | q ≠ p}) => attrField_div (p := p) hq k
  have hint := hi.congr_fun (fun q hq => (hd q (S.ne q (S.hVK hq))).2.symm) S.hV
  have := hdiv {q | q ≠ p} (isOpen_ne_point p) (fun q hq => S.ne q hq) _ _ (fun q hq => (hd q hq).1) hint
  rw [setIntegral_congr_fun S.hV (fun q hq => (hd q (S.ne q (S.hVK hq))).2)] at this
  have hneg : ∫ q in V, (q k - p k) / (sqDist p 0 q * edist p q) =
      -∫ q in V, -(q k - p k) / (sqDist p 0 q * edist p q) := by
    rw [← integral_neg]
    congr 1
    funext q
    ring
  rw [hneg, this]
  congr 1
  apply Finset.sum_congr rfl
  intro j _
  rw [← face_integral (F j) p (hG j) (S.face_ne j) (S.hoff j), ← integral_const_mul]
  apply setIntegral_congr_fun (F j).hD.measurableSet
  intro x _
  simp only [dot_constField, dotProduct_single, mul_one, edist]

/-- **The solid angles of a closed polyhedron sum to zero from outside** (Werner & Scheeres
eq. 17, the Laplacian `−Gρ Σ_f ω_f` vanishing outside). -/
theorem solid_angles_sum_zero (S : Setting V K F p) (hdiv : DivergenceTheorem V K F) :
    ∑ j, (F j).ω p = 0 := by
  have hd := fun q (hq : q ∈ {q : Fin 3 → ℝ | q ≠ p}) => gaussField_div (p := p) hq
  have hint : IntegrableOn (fun q => ∑ i, radialDeriv p 0 (fun s => 1 / (s * Real.sqrt s))
      (-3 / (2 * sqDist p 0 q * sqDist p 0 q * Real.sqrt (sqDist p 0 q))) q
      (Pi.single i 1) i) V :=
    (integrableOn_zero).congr_fun (fun q hq => (hd q (S.ne q (S.hVK hq))).2.symm) S.hV
  have := hdiv {q | q ≠ p} (isOpen_ne_point p) (fun q hq => S.ne q hq) _ _ (fun q hq => (hd q hq).1) hint
  rw [setIntegral_congr_fun S.hV (fun q hq => (hd q (S.ne q (S.hVK hq))).2),
    integral_zero] at this
  refine Eq.trans ?_ this.symm
  apply Finset.sum_congr rfl
  intro j _
  unfold Face.ω
  apply setIntegral_congr_fun (F j).hD.measurableSet
  intro x _
  simp only [dot_radialField, (F j).n_dot_pt_sub]
  ring

end Body

end PolyhedralGravity
