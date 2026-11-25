/-
Divergences of the fields the derivation integrates, in `ℝⁿ` written as `Fin n → ℝ` (the type
Mathlib's divergence theorem is stated on). Distances are Euclidean, written out as
`√(Σⱼ (xⱼ − pⱼ)² + c)` (Mathlib's norm on `Fin n → ℝ` is the sup norm, so it is not used).

* `radialField p c φ x = (x − p) φ(S(x))`, `S(x) = Σⱼ (xⱼ − pⱼ)² + c`, has divergence
  `n φ(S) + 2 (S − c) φ'(S)`. Three instances:
  - `n = 3, c = 0, φ = 1/(2√s)`: `div ((q − p)/(2r)) = 1/r` — the potential's field
    (Werner & Scheeres 1997, eq. 1);
  - `n = 2, c = h², φ = 1/√s`: `div ((x − p')/R) = 1/R + h²/R³` in a face's plane, `R² = |x − p'|² + h²`
    — the face's field (their eq. 3);
  - `n = 3, c = 0, φ = s^{-3/2}`: `div ((q − p)/r³) = 0` off `p` — Gauss's law, the field of the
    solid angle (their eq. 17).
* `constField p c w ψ x = w ψ(S(x))` has divergence `2 ψ'(S) Σᵢ wᵢ (xᵢ − pᵢ)`; with `ψ = 1/√s` and
  `w = eₖ` this is `−(xₖ − pₖ)/r³`, the attraction's field.

Everything here is proven; nothing is assumed.
-/
import Mathlib

open BigOperators

namespace PolyhedralGravity

variable {n : ℕ}

/-- `S(x) = Σⱼ (xⱼ − pⱼ)² + c`: the squared Euclidean distance from `p`, plus a constant. -/
noncomputable def sqDist (p : Fin n → ℝ) (c : ℝ) (x : Fin n → ℝ) : ℝ := ∑ j, (x j - p j) ^ 2 + c

/-- The derivative of `S`: `v ↦ Σⱼ 2 (xⱼ − pⱼ) vⱼ`. -/
noncomputable def sqDistDeriv (p x : Fin n → ℝ) : (Fin n → ℝ) →L[ℝ] ℝ :=
  ∑ j, (2 * (x j - p j)) • ContinuousLinearMap.proj (R := ℝ) (φ := fun _ : Fin n => ℝ) j

lemma sqDistDeriv_apply (p x v : Fin n → ℝ) : sqDistDeriv p x v = ∑ j, 2 * (x j - p j) * v j := by
  simp [sqDistDeriv, mul_comm]

lemma hasFDerivAt_sqDist (p : Fin n → ℝ) (c : ℝ) (x : Fin n → ℝ) :
    HasFDerivAt (sqDist p c) (sqDistDeriv p x) x := by
  have hj : ∀ j ∈ (Finset.univ : Finset (Fin n)),
      HasFDerivAt (fun y : Fin n → ℝ => (y j - p j) ^ 2)
        ((2 * (x j - p j)) • ContinuousLinearMap.proj (R := ℝ) (φ := fun _ : Fin n => ℝ) j) x := by
    intro j _
    have h1 : HasFDerivAt (fun y : Fin n → ℝ => y j - p j)
        (ContinuousLinearMap.proj (R := ℝ) (φ := fun _ : Fin n => ℝ) j) x :=
      (hasFDerivAt_apply j x).sub_const (p j)
    have h2 := h1.pow 2
    convert h2 using 1
    simp
  have := (HasFDerivAt.sum hj).add_const c
  unfold sqDist sqDistDeriv
  simpa using this

/-! ### The radial field `(x − p) φ(S(x))` -/

/-- `(x − p) φ(S(x))`. -/
noncomputable def radialField (p : Fin n → ℝ) (c : ℝ) (φ : ℝ → ℝ) (x : Fin n → ℝ) : Fin n → ℝ :=
  fun i => (x i - p i) * φ (sqDist p c x)

/-- Its derivative at `x`, given `φ'(S(x))`. -/
noncomputable def radialDeriv (p : Fin n → ℝ) (c : ℝ) (φ : ℝ → ℝ) (φ' : ℝ) (x : Fin n → ℝ) :
    (Fin n → ℝ) →L[ℝ] (Fin n → ℝ) :=
  ContinuousLinearMap.pi fun i =>
    (x i - p i) • (φ' • sqDistDeriv p x) +
      φ (sqDist p c x) • ContinuousLinearMap.proj (R := ℝ) (φ := fun _ : Fin n => ℝ) i

lemma hasFDerivAt_radialField (p : Fin n → ℝ) (c : ℝ) {φ : ℝ → ℝ} {φ' : ℝ} (x : Fin n → ℝ)
    (hφ : HasDerivAt φ φ' (sqDist p c x)) :
    HasFDerivAt (radialField p c φ) (radialDeriv p c φ φ' x) x := by
  unfold radialField radialDeriv
  refine hasFDerivAt_pi.2 (fun i => ?_)
  have h1 : HasFDerivAt (fun y : Fin n → ℝ => y i - p i)
      (ContinuousLinearMap.proj (R := ℝ) (φ := fun _ : Fin n => ℝ) i) x :=
    (hasFDerivAt_apply i x).sub_const (p i)
  have h2 : HasFDerivAt (fun y => φ (sqDist p c y)) (φ' • sqDistDeriv p x) x :=
    hφ.comp_hasFDerivAt x (hasFDerivAt_sqDist p c x)
  exact h1.mul h2

/-- **The divergence of the radial field**: `Σᵢ ∂ᵢ Fᵢ = n φ(S) + 2 (S − c) φ'(S)`. -/
lemma radialDeriv_trace (p : Fin n → ℝ) (c : ℝ) (φ : ℝ → ℝ) (φ' : ℝ) (x : Fin n → ℝ) :
    ∑ i, radialDeriv p c φ φ' x (Pi.single i 1) i =
      n * φ (sqDist p c x) + 2 * (sqDist p c x - c) * φ' := by
  simp only [radialDeriv, ContinuousLinearMap.pi_apply, ContinuousLinearMap.add_apply,
    ContinuousLinearMap.smul_apply, ContinuousLinearMap.proj_apply, sqDistDeriv_apply,
    smul_eq_mul, Pi.single_apply, mul_ite, mul_one, mul_zero, Finset.sum_ite_eq',
    Finset.mem_univ, ite_true]
  simp only [Finset.sum_add_distrib, Finset.sum_const, Finset.card_univ,
    Fintype.card_fin, nsmul_eq_mul, sqDist, add_sub_cancel_right]
  rw [add_comm]
  congr 1
  rw [Finset.mul_sum, Finset.sum_mul]
  apply Finset.sum_congr rfl
  intro i _
  ring

/-! ### The constant-direction field `w ψ(S(x))` -/

/-- `w ψ(S(x))`. -/
noncomputable def constField (p : Fin n → ℝ) (c : ℝ) (w : Fin n → ℝ) (ψ : ℝ → ℝ)
    (x : Fin n → ℝ) : Fin n → ℝ :=
  fun i => w i * ψ (sqDist p c x)

/-- Its derivative at `x`, given `ψ'(S(x))`. -/
noncomputable def constDeriv (p : Fin n → ℝ) (w : Fin n → ℝ) (ψ' : ℝ) (x : Fin n → ℝ) :
    (Fin n → ℝ) →L[ℝ] (Fin n → ℝ) :=
  ContinuousLinearMap.pi fun i => w i • (ψ' • sqDistDeriv p x)

lemma hasFDerivAt_constField (p : Fin n → ℝ) (c : ℝ) (w : Fin n → ℝ) {ψ : ℝ → ℝ} {ψ' : ℝ}
    (x : Fin n → ℝ) (hψ : HasDerivAt ψ ψ' (sqDist p c x)) :
    HasFDerivAt (constField p c w ψ) (constDeriv p w ψ' x) x := by
  unfold constField constDeriv
  refine hasFDerivAt_pi.2 (fun i => ?_)
  have h2 : HasFDerivAt (fun y => ψ (sqDist p c y)) (ψ' • sqDistDeriv p x) x :=
    hψ.comp_hasFDerivAt x (hasFDerivAt_sqDist p c x)
  exact h2.const_mul (w i)

lemma constDeriv_trace (p : Fin n → ℝ) (w : Fin n → ℝ) (ψ' : ℝ) (x : Fin n → ℝ) :
    ∑ i, constDeriv p w ψ' x (Pi.single i 1) i = 2 * ψ' * ∑ i, w i * (x i - p i) := by
  simp only [constDeriv, ContinuousLinearMap.pi_apply, ContinuousLinearMap.smul_apply,
    sqDistDeriv_apply, smul_eq_mul, Pi.single_apply, mul_ite, mul_one, mul_zero,
    Finset.sum_ite_eq', Finset.mem_univ, ite_true]
  rw [Finset.mul_sum]
  apply Finset.sum_congr rfl
  intro i _
  ring

/-! ### The three scalar profiles and their derivatives -/

lemma hasDerivAt_half_inv_sqrt {s : ℝ} (hs : 0 < s) :
    HasDerivAt (fun s : ℝ => 1 / (2 * Real.sqrt s)) (-1 / (4 * s * Real.sqrt s)) s := by
  have h := ((Real.hasDerivAt_sqrt hs.ne').const_mul 2).inv
    (mul_pos two_pos (Real.sqrt_pos.2 hs)).ne'
  simp only [one_div]
  refine h.congr_deriv ?_
  obtain ⟨q, hq, rfl⟩ : ∃ q, 0 < q ∧ s = q ^ 2 :=
    ⟨Real.sqrt s, Real.sqrt_pos.2 hs, (Real.sq_sqrt hs.le).symm⟩
  rw [Real.sqrt_sq hq.le]
  field_simp
  try ring

lemma hasDerivAt_inv_sqrt {s : ℝ} (hs : 0 < s) :
    HasDerivAt (fun s : ℝ => 1 / Real.sqrt s) (-1 / (2 * s * Real.sqrt s)) s := by
  have h := (Real.hasDerivAt_sqrt hs.ne').inv (Real.sqrt_pos.2 hs).ne'
  simp only [one_div]
  refine h.congr_deriv ?_
  obtain ⟨q, hq, rfl⟩ : ∃ q, 0 < q ∧ s = q ^ 2 :=
    ⟨Real.sqrt s, Real.sqrt_pos.2 hs, (Real.sq_sqrt hs.le).symm⟩
  rw [Real.sqrt_sq hq.le]
  field_simp
  try ring

lemma hasDerivAt_inv_sqrt_cube {s : ℝ} (hs : 0 < s) :
    HasDerivAt (fun s : ℝ => 1 / (s * Real.sqrt s)) (-3 / (2 * s * s * Real.sqrt s)) s := by
  have h := ((hasDerivAt_id' s).mul (Real.hasDerivAt_sqrt hs.ne')).inv
    (mul_pos hs (Real.sqrt_pos.2 hs)).ne'
  simp only [one_div]
  refine h.congr_deriv ?_
  simp only [Pi.mul_apply]
  obtain ⟨q, hq, rfl⟩ : ∃ q, 0 < q ∧ s = q ^ 2 :=
    ⟨Real.sqrt s, Real.sqrt_pos.2 hs, (Real.sq_sqrt hs.le).symm⟩
  rw [Real.sqrt_sq hq.le]
  field_simp
  try ring

/-! ### The instances the derivation uses -/

/-- `r(x) = √(Σⱼ (xⱼ − pⱼ)²)`, the Euclidean distance from `p`. -/
noncomputable def edist (p x : Fin n → ℝ) : ℝ := Real.sqrt (sqDist p 0 x)

lemma sqDist_zero_pos {p x : Fin n → ℝ} (h : x ≠ p) : 0 < sqDist p 0 x := by
  unfold sqDist
  rw [add_zero]
  obtain ⟨j, hj⟩ : ∃ j, x j ≠ p j := by
    by_contra hc
    push Not at hc
    exact h (funext hc)
  have : 0 < (x j - p j) ^ 2 := by
    have : x j - p j ≠ 0 := sub_ne_zero.2 hj
    positivity
  exact lt_of_lt_of_le this
    (Finset.single_le_sum (f := fun j => (x j - p j) ^ 2) (fun _ _ => sq_nonneg _)
      (Finset.mem_univ j))

/-- The potential's field `(q − p)/(2r)` on ℝ³. -/
noncomputable def potField (p : Fin 3 → ℝ) : (Fin 3 → ℝ) → (Fin 3 → ℝ) :=
  radialField p 0 (fun s => 1 / (2 * Real.sqrt s))

/-- `div ((q − p)/(2r)) = 1/r` for `q ≠ p`. -/
theorem potField_div {p x : Fin 3 → ℝ} (hx : x ≠ p) :
    HasFDerivAt (potField p)
        (radialDeriv p 0 (fun s => 1 / (2 * Real.sqrt s))
          (-1 / (4 * sqDist p 0 x * Real.sqrt (sqDist p 0 x))) x) x ∧
      ∑ i, radialDeriv p 0 (fun s => 1 / (2 * Real.sqrt s))
          (-1 / (4 * sqDist p 0 x * Real.sqrt (sqDist p 0 x))) x (Pi.single i 1) i =
        1 / edist p x := by
  have hs := sqDist_zero_pos hx
  refine ⟨hasFDerivAt_radialField p 0 x (hasDerivAt_half_inv_sqrt hs), ?_⟩
  rw [radialDeriv_trace]
  unfold edist
  have hq : 0 < Real.sqrt (sqDist p 0 x) := Real.sqrt_pos.2 hs
  push_cast
  field_simp
  ring

/-- Gauss's field `(q − p)/r³` on ℝ³ has zero divergence off `p`. -/
theorem gaussField_div {p x : Fin 3 → ℝ} (hx : x ≠ p) :
    HasFDerivAt (radialField p 0 (fun s => 1 / (s * Real.sqrt s)))
        (radialDeriv p 0 (fun s => 1 / (s * Real.sqrt s))
          (-3 / (2 * sqDist p 0 x * sqDist p 0 x * Real.sqrt (sqDist p 0 x))) x) x ∧
      ∑ i, radialDeriv p 0 (fun s => 1 / (s * Real.sqrt s))
          (-3 / (2 * sqDist p 0 x * sqDist p 0 x * Real.sqrt (sqDist p 0 x))) x
          (Pi.single i 1) i = 0 := by
  have hs := sqDist_zero_pos hx
  refine ⟨hasFDerivAt_radialField p 0 x (hasDerivAt_inv_sqrt_cube hs), ?_⟩
  rw [radialDeriv_trace]
  have hq : 0 < Real.sqrt (sqDist p 0 x) := Real.sqrt_pos.2 hs
  push_cast
  field_simp
  ring

/-- The face's field `(x − p')/R` on a plane, `R = √(|x − p'|² + h²)`:
`div = 1/R + h²/R³`, wherever `R > 0`. -/
theorem faceField_div {p x : Fin 2 → ℝ} {h : ℝ} (hs : 0 < sqDist p (h ^ 2) x) :
    HasFDerivAt (radialField p (h ^ 2) (fun s => 1 / Real.sqrt s))
        (radialDeriv p (h ^ 2) (fun s => 1 / Real.sqrt s)
          (-1 / (2 * sqDist p (h ^ 2) x * Real.sqrt (sqDist p (h ^ 2) x))) x) x ∧
      ∑ i, radialDeriv p (h ^ 2) (fun s => 1 / Real.sqrt s)
          (-1 / (2 * sqDist p (h ^ 2) x * Real.sqrt (sqDist p (h ^ 2) x))) x (Pi.single i 1) i =
        1 / Real.sqrt (sqDist p (h ^ 2) x) +
          h ^ 2 / (sqDist p (h ^ 2) x * Real.sqrt (sqDist p (h ^ 2) x)) := by
  refine ⟨hasFDerivAt_radialField p (h ^ 2) x (hasDerivAt_inv_sqrt hs), ?_⟩
  rw [radialDeriv_trace]
  have hq : 0 < Real.sqrt (sqDist p (h ^ 2) x) := Real.sqrt_pos.2 hs
  push_cast
  field_simp
  ring

/-- The attraction's field `eₖ / r` on ℝ³: `div = −(xₖ − pₖ)/r³`. -/
theorem attrField_div {p x : Fin 3 → ℝ} (hx : x ≠ p) (k : Fin 3) :
    HasFDerivAt (constField p 0 (Pi.single k 1) (fun s => 1 / Real.sqrt s))
        (constDeriv p (Pi.single k 1)
          (-1 / (2 * sqDist p 0 x * Real.sqrt (sqDist p 0 x))) x) x ∧
      ∑ i, constDeriv p (Pi.single k 1)
          (-1 / (2 * sqDist p 0 x * Real.sqrt (sqDist p 0 x))) x (Pi.single i 1) i =
        -(x k - p k) / (sqDist p 0 x * edist p x) := by
  have hs := sqDist_zero_pos hx
  refine ⟨hasFDerivAt_constField p 0 _ x (hasDerivAt_inv_sqrt hs), ?_⟩
  rw [constDeriv_trace]
  simp only [Pi.single_apply, ite_mul, one_mul, zero_mul, Finset.sum_ite_eq', Finset.mem_univ,
    ite_true]
  unfold edist
  have hq : 0 < Real.sqrt (sqDist p 0 x) := Real.sqrt_pos.2 hs
  field_simp

end PolyhedralGravity
