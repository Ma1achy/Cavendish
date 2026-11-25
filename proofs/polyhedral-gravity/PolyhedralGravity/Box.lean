/-
(c) for a rectangular box (Nagy's prism), with no hypotheses: the divergence-theorem step proven
from Mathlib's divergence theorem on boxes (`MeasureTheory.integral_divergence_of_hasFDerivAt_off_countable`).

For a box `[a, b] ⊆ ℝ³` and a point `p` outside it:

* `box_potential`: `∫_box dV/|q − p| = ½ Σ_faces h_f ∫_f dS/|q − p|` (Werner & Scheeres eq. 2),
  where the face `x_i = b_i` has outward normal `+e_i` and height `h = b_i − p_i`, and the face
  `x_i = a_i` has normal `−e_i` and height `p_i − a_i`;
* `box_attraction`: `∫_box (q − p)_k/|q − p|³ dV = −Σ_faces (n_f)_k ∫_f dS/|q − p|`;
* `box_gauss`: `Σ_faces h_f ∫_f dS/|q − p|³ = 0`, i.e. the six solid angles sum to zero from outside.

The faces are Mathlib's: `Icc (a ∘ i.succAbove) (b ∘ i.succAbove)` in ℝ², placed by
`Fin.insertNth i (b i)` (front) and `Fin.insertNth i (a i)` (back).
-/
import Mathlib
import PolyhedralGravity.Divergence

open MeasureTheory Set

namespace PolyhedralGravity

variable {a b p : Fin 3 → ℝ}

lemma mem_Icc_of_mem_pi_Ioo {x : Fin 3 → ℝ} (hx : x ∈ Set.pi univ fun i => Ioo (a i) (b i)) :
    x ∈ Icc a b := by
  simp only [Set.mem_pi, Set.mem_univ, forall_const, Set.mem_Ioo] at hx
  exact ⟨fun i => (hx i).1.le, fun i => (hx i).2.le⟩

lemma ne_of_not_mem (hp : p ∉ Icc a b) : ∀ q ∈ Icc a b, q ≠ p := fun _ hq h => hp (h ▸ hq)

lemma continuousOn_inv_edist_Icc (hp : p ∉ Icc a b) :
    ContinuousOn (fun q => 1 / edist p q) (Icc a b) := by
  apply ContinuousOn.div continuousOn_const
  · unfold sqDist; fun_prop
  · intro q hq
    exact (Real.sqrt_pos.2 (sqDist_zero_pos (ne_of_not_mem hp q hq))).ne'

/-- **The potential of a box, as face integrals** (the divergence-theorem step, proven). -/
theorem box_potential (hle : a ≤ b) (hp : p ∉ Icc a b) :
    ∫ q in Icc a b, 1 / edist p q =
      ∑ i : Fin 3,
        ((b i - p i) / 2 * (∫ y in Icc (a ∘ i.succAbove) (b ∘ i.succAbove),
            1 / edist p (i.insertNth (b i) y)) -
          (a i - p i) / 2 * (∫ y in Icc (a ∘ i.succAbove) (b ∘ i.succAbove),
            1 / edist p (i.insertNth (a i) y))) := by
  have hne := ne_of_not_mem hp
  have hd := fun q (hq : q ∈ Icc a b) => potField_div (hne q hq)
  have Hc : ContinuousOn (potField p) (Icc a b) :=
    fun q hq => (hd q hq).1.continuousAt.continuousWithinAt
  have Hi : IntegrableOn (fun q => ∑ i, radialDeriv p 0 (fun s => 1 / (2 * Real.sqrt s))
      (-1 / (4 * sqDist p 0 q * Real.sqrt (sqDist p 0 q))) q (Pi.single i 1) i) (Icc a b) :=
    ((continuousOn_inv_edist_Icc hp).integrableOn_compact isCompact_Icc).congr_fun
      (fun q hq => ((hd q hq).2).symm) measurableSet_Icc
  have key := integral_divergence_of_hasFDerivAt_off_countable a b hle (potField p)
    (fun q => radialDeriv p 0 (fun s => 1 / (2 * Real.sqrt s))
      (-1 / (4 * sqDist p 0 q * Real.sqrt (sqDist p 0 q))) q) ∅ Set.countable_empty Hc
    (fun x hx => (hd x (mem_Icc_of_mem_pi_Ioo hx.1)).1) Hi
  rw [setIntegral_congr_fun measurableSet_Icc (fun q hq => (hd q hq).2)] at key
  rw [key]
  apply Finset.sum_congr rfl
  intro i _
  rw [← integral_const_mul, ← integral_const_mul]
  congr 1 <;>
  · congr 1
    funext y
    simp only [potField, radialField, Fin.insertNth_apply_same, edist]
    ring

/-- **The attraction of a box, as face integrals**, component `k`:
`∫_box (q − p)_k/|q − p|³ dV = −(∫_{x_k = b_k} dS/r − ∫_{x_k = a_k} dS/r)`. -/
theorem box_attraction (hle : a ≤ b) (hp : p ∉ Icc a b) (k : Fin 3) :
    ∫ q in Icc a b, (q k - p k) / (sqDist p 0 q * edist p q) =
      -((∫ y in Icc (a ∘ k.succAbove) (b ∘ k.succAbove), 1 / edist p (k.insertNth (b k) y)) -
        ∫ y in Icc (a ∘ k.succAbove) (b ∘ k.succAbove), 1 / edist p (k.insertNth (a k) y)) := by
  have hne := ne_of_not_mem hp
  have hd := fun q (hq : q ∈ Icc a b) => attrField_div (hne q hq) k
  have Hc : ContinuousOn (constField p 0 (Pi.single k 1) (fun s => 1 / Real.sqrt s)) (Icc a b) :=
    fun q hq => (hd q hq).1.continuousAt.continuousWithinAt
  have hcont : ContinuousOn (fun q => -(q k - p k) / (sqDist p 0 q * edist p q)) (Icc a b) := by
    refine ContinuousOn.div (f := fun q => -(q k - p k)) (g := fun q => sqDist p 0 q * edist p q)
      ?_ ?_ ?_
    · exact ((continuous_apply k).sub continuous_const).neg.continuousOn
    · have h1 : Continuous fun q : Fin 3 → ℝ => sqDist p 0 q := by unfold sqDist; fun_prop
      have h2 : Continuous fun q : Fin 3 → ℝ => edist p q := by unfold edist sqDist; fun_prop
      exact (h1.mul h2).continuousOn
    · intro q hq
      have h1 := sqDist_zero_pos (hne q hq)
      exact (mul_pos h1 (Real.sqrt_pos.2 h1)).ne'
  have Hi : IntegrableOn (fun q => ∑ i, constDeriv p (Pi.single k 1)
      (-1 / (2 * sqDist p 0 q * Real.sqrt (sqDist p 0 q))) q (Pi.single i 1) i) (Icc a b) :=
    (hcont.integrableOn_compact isCompact_Icc).congr_fun
      (fun q hq => ((hd q hq).2).symm) measurableSet_Icc
  have key := integral_divergence_of_hasFDerivAt_off_countable a b hle _ _ ∅
    Set.countable_empty Hc (fun x hx => (hd x (mem_Icc_of_mem_pi_Ioo hx.1)).1) Hi
  rw [setIntegral_congr_fun measurableSet_Icc (fun q hq => (hd q hq).2)] at key
  have hneg : ∫ q in Icc a b, (q k - p k) / (sqDist p 0 q * edist p q) =
      -∫ q in Icc a b, -(q k - p k) / (sqDist p 0 q * edist p q) := by
    rw [← integral_neg]
    congr 1
    funext q
    ring
  rw [hneg, key]
  congr 1
  rw [Finset.sum_eq_single k]
  · simp [constField, edist]
  · intro i _ hik
    simp [constField, hik]
  · simp

/-- **Gauss's law for a box**: the faces' solid angles `h_f ∫_f dS/r³` sum to zero from outside. -/
theorem box_gauss (hle : a ≤ b) (hp : p ∉ Icc a b) :
    ∑ i : Fin 3,
      ((b i - p i) * (∫ y in Icc (a ∘ i.succAbove) (b ∘ i.succAbove),
          1 / (sqDist p 0 (i.insertNth (b i) y) * edist p (i.insertNth (b i) y))) -
        (a i - p i) * (∫ y in Icc (a ∘ i.succAbove) (b ∘ i.succAbove),
          1 / (sqDist p 0 (i.insertNth (a i) y) * edist p (i.insertNth (a i) y)))) = 0 := by
  have hne := ne_of_not_mem hp
  have hd := fun q (hq : q ∈ Icc a b) => gaussField_div (hne q hq)
  have Hc : ContinuousOn (radialField p 0 (fun s => 1 / (s * Real.sqrt s))) (Icc a b) :=
    fun q hq => (hd q hq).1.continuousAt.continuousWithinAt
  have Hi : IntegrableOn (fun q => ∑ i, radialDeriv p 0 (fun s => 1 / (s * Real.sqrt s))
      (-3 / (2 * sqDist p 0 q * sqDist p 0 q * Real.sqrt (sqDist p 0 q))) q (Pi.single i 1) i)
      (Icc a b) :=
    (integrableOn_zero).congr_fun (fun q hq => ((hd q hq).2).symm) measurableSet_Icc
  have key := integral_divergence_of_hasFDerivAt_off_countable a b hle _ _ ∅
    Set.countable_empty Hc (fun x hx => (hd x (mem_Icc_of_mem_pi_Ioo hx.1)).1) Hi
  rw [setIntegral_congr_fun measurableSet_Icc (fun q hq => (hd q hq).2), integral_zero] at key
  refine Eq.trans ?_ key.symm
  apply Finset.sum_congr rfl
  intro i _
  rw [← integral_const_mul, ← integral_const_mul]
  congr 1 <;>
  · congr 1
    funext y
    simp only [radialField, Fin.insertNth_apply_same, edist]

end PolyhedralGravity
