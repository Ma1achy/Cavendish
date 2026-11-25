/-
(d) Linearity: pieces add up to the body, and the phase is linear in the potential.

* `integral_pieces`: if the cells `C k` (`k ∈ t`) are measurable, pairwise disjoint and cover the
  body `B`, then for any integrand integrable on `B` (the potential's `ρ/|q − p|`, any component of
  the attraction's `ρ (q − p)/|q − p|³`, any density), the body's integral is the sum over cells of
  the integral over `B ∩ C k`. With `interior_and_boundary`: cells inside the body contribute
  their whole integral, the others (the boundary cells) their clipped piece.
* `phase_*`: the gradiometer phase Cavendish computes is
  `δφ = κ ∫₀^{2T} [V(x_u(t)) − V(x_l(t))] dt` (`κ = m_A/ħ`), taken by a quadrature rule
  `Q[V] = κ Σᵢ wᵢ [V(x_u(tᵢ)) − V(x_l(tᵢ))]`. Both are linear in `V`. So if the potential is a
  finite sum of pieces' potentials, each evaluated exactly (the closed form), the phase is the sum of
  the pieces' phases, and the only error left is the quadrature's own, the same as for the whole
  body: `Q[Σ V_k] − δφ[Σ V_k] = Σ (Q[V_k] − δφ[V_k])` (`quadrature_error_additive`).

Everything here is proven; nothing is assumed.
-/
import Mathlib

open MeasureTheory

namespace PolyhedralGravity

section Pieces

variable {X : Type*} [MeasurableSpace X] {μ : Measure X} {ι : Type*}

/-- **Pieces sum to the body.** -/
theorem integral_pieces (B : Set X) (C : ι → Set X) (t : Finset ι)
    (hC : ∀ k ∈ t, MeasurableSet (C k)) (hB : MeasurableSet B)
    (hdisj : (t : Set ι).PairwiseDisjoint C) (hcover : B ⊆ ⋃ k ∈ t, C k)
    (f : X → ℝ) (hf : IntegrableOn f B μ) :
    ∫ x in B, f x ∂μ = ∑ k ∈ t, ∫ x in B ∩ C k, f x ∂μ := by
  have hunion : (⋃ k ∈ t, B ∩ C k) = B := by
    ext x
    simp only [Set.mem_iUnion, Set.mem_inter_iff, exists_prop]
    constructor
    · rintro ⟨k, _, hx, _⟩; exact hx
    · intro hx
      obtain ⟨k, hk, hxk⟩ := Set.mem_iUnion₂.1 (hcover hx)
      exact ⟨k, hk, hx, hxk⟩
  conv_lhs => rw [← hunion]
  refine integral_biUnion_finset t (fun k hk => hB.inter (hC k hk)) ?_ ?_
  · intro i hi j hj hij
    exact Disjoint.mono Set.inter_subset_right Set.inter_subset_right (hdisj hi hj hij)
  · intro k _
    exact hf.mono_set Set.inter_subset_left

/-- **Interior cells whole, boundary cells clipped.** Cells in `inner ⊆ t` lie inside the body, so
their piece is the whole cell. -/
theorem interior_and_boundary [DecidableEq ι] (B : Set X) (C : ι → Set X) (t inner : Finset ι)
    (hsub : inner ⊆ t) (hin : ∀ k ∈ inner, C k ⊆ B)
    (hC : ∀ k ∈ t, MeasurableSet (C k)) (hB : MeasurableSet B)
    (hdisj : (t : Set ι).PairwiseDisjoint C) (hcover : B ⊆ ⋃ k ∈ t, C k)
    (f : X → ℝ) (hf : IntegrableOn f B μ) :
    ∫ x in B, f x ∂μ =
      ∑ k ∈ inner, ∫ x in C k, f x ∂μ + ∑ k ∈ t \ inner, ∫ x in B ∩ C k, f x ∂μ := by
  rw [integral_pieces B C t hC hB hdisj hcover f hf, ← Finset.sum_sdiff hsub, add_comm]
  congr 1
  apply Finset.sum_congr rfl
  intro k hk
  rw [Set.inter_eq_right.2 (hin k hk)]

end Pieces

section Phase

/-- The gradiometer's phase from a potential `V` (J/kg, any frame), for arms `xu`, `xl`
(position at time `t`), `κ = m_A/ħ`, over the flight `[0, 2T]`. -/
noncomputable def phase {P : Type*} (κ T : ℝ) (xu xl : ℝ → P) (V : P → ℝ) : ℝ :=
  κ * ∫ t in (0 : ℝ)..(2 * T), (V (xu t) - V (xl t))

/-- The same by a quadrature rule with nodes `tq i` and weights `wq i`. -/
noncomputable def phaseQ {P : Type*} {m : ℕ} (κ : ℝ) (tq wq : Fin m → ℝ) (xu xl : ℝ → P)
    (V : P → ℝ) : ℝ :=
  κ * ∑ i, wq i * (V (xu (tq i)) - V (xl (tq i)))

variable {P : Type*} {ι : Type*}

/-- The quadrature phase of a sum of pieces is the sum of their quadrature phases, exactly. -/
theorem phaseQ_sum {m : ℕ} (κ : ℝ) (tq wq : Fin m → ℝ) (xu xl : ℝ → P) (s : Finset ι)
    (V : ι → P → ℝ) :
    phaseQ κ tq wq xu xl (fun x => ∑ k ∈ s, V k x) = ∑ k ∈ s, phaseQ κ tq wq xu xl (V k) := by
  unfold phaseQ
  rw [← Finset.mul_sum]
  congr 1
  rw [Finset.sum_comm]
  apply Finset.sum_congr rfl
  intro i _
  rw [← Finset.sum_sub_distrib, Finset.mul_sum]

/-- The exact phase of a sum of pieces is the sum of their phases (each piece's potential
integrable along both arms over the flight). -/
theorem phase_sum (κ T : ℝ) (xu xl : ℝ → P) (s : Finset ι) (V : ι → P → ℝ)
    (hi : ∀ k ∈ s, IntervalIntegrable (fun t => V k (xu t) - V k (xl t)) volume 0 (2 * T)) :
    phase κ T xu xl (fun x => ∑ k ∈ s, V k x) = ∑ k ∈ s, phase κ T xu xl (V k) := by
  unfold phase
  rw [← Finset.mul_sum]
  congr 1
  have : (fun t => (∑ k ∈ s, V k (xu t)) - ∑ k ∈ s, V k (xl t)) =
      fun t => ∑ k ∈ s, (V k (xu t) - V k (xl t)) := by
    funext t; rw [Finset.sum_sub_distrib]
  rw [this, intervalIntegral.integral_finsetSum hi]

/-- **The pieces add no error of their own.** The quadrature error of the body's phase is the sum
of the pieces' quadrature errors; there is no other term. -/
theorem quadrature_error_additive {m : ℕ} (κ T : ℝ) (tq wq : Fin m → ℝ) (xu xl : ℝ → P)
    (s : Finset ι) (V : ι → P → ℝ)
    (hi : ∀ k ∈ s, IntervalIntegrable (fun t => V k (xu t) - V k (xl t)) volume 0 (2 * T)) :
    phaseQ κ tq wq xu xl (fun x => ∑ k ∈ s, V k x) - phase κ T xu xl (fun x => ∑ k ∈ s, V k x) =
      ∑ k ∈ s, (phaseQ κ tq wq xu xl (V k) - phase κ T xu xl (V k)) := by
  rw [phaseQ_sum, phase_sum κ T xu xl s V hi, Finset.sum_sub_distrib]

/-- **Exact pieces give the exact quadrature phase.** If each piece's computed potential `Vc k`
equals its true potential `V k` at every arm point the rule visits, the computed phase equals the
quadrature phase of the true body's potential `Vb` (any `Vb` that is the sum of the pieces there). -/
theorem exact_pieces_exact_phase {m : ℕ} (κ : ℝ) (tq wq : Fin m → ℝ) (xu xl : ℝ → P)
    (s : Finset ι) (V Vc : ι → P → ℝ) (Vb : P → ℝ)
    (hb : ∀ x, Vb x = ∑ k ∈ s, V k x)
    (hc : ∀ k ∈ s, ∀ i, Vc k (xu (tq i)) = V k (xu (tq i)) ∧ Vc k (xl (tq i)) = V k (xl (tq i))) :
    ∑ k ∈ s, phaseQ κ tq wq xu xl (Vc k) = phaseQ κ tq wq xu xl Vb := by
  have : Vb = fun x => ∑ k ∈ s, V k x := funext hb
  rw [this, phaseQ_sum]
  apply Finset.sum_congr rfl
  intro k hk
  unfold phaseQ
  congr 1
  apply Finset.sum_congr rfl
  intro i _
  rw [(hc k hk i).1, (hc k hk i).2]

/-- The gradiometer reads the difference of two interferometers' phases; a difference of linear
functionals is linear, so all of the above holds for it as well. -/
theorem gradiometer_sum {m : ℕ} (κ : ℝ) (tq wq : Fin m → ℝ) (xu₁ xl₁ xu₂ xl₂ : ℝ → P)
    (s : Finset ι) (V : ι → P → ℝ) :
    phaseQ κ tq wq xu₁ xl₁ (fun x => ∑ k ∈ s, V k x) -
        phaseQ κ tq wq xu₂ xl₂ (fun x => ∑ k ∈ s, V k x) =
      ∑ k ∈ s, (phaseQ κ tq wq xu₁ xl₁ (V k) - phaseQ κ tq wq xu₂ xl₂ (V k)) := by
  rw [phaseQ_sum, phaseQ_sum, Finset.sum_sub_distrib]

end Phase

end PolyhedralGravity
