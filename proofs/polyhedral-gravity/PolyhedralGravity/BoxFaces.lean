/-
(c) for a rectangular box, continued: each face, from Mathlib's divergence theorem in the plane,
and each edge, in closed form. With `Box.lean` this is the whole of Werner & Scheeres' chain for
Nagy's prism with no hypotheses; the solid angle stays the integral `ω = ∫ h/R³ dS`.

* `rect_face`: for a rectangle `[c, d] ⊂ ℝ²` in a plane at height `h` above the point, whose
  projection is `p'`, `∫ dS/R = Σ_edges (ν·r_e) ∫_e ds/R − h ∫ h/R³ dS` (W&S eq. 3, 8);
* `integral_Icc_fin_one`: an integral over a box of `Fin 1 → ℝ` (Mathlib's form of an edge) is an
  interval integral;
* `integral_inv_lineDist'`: the edge integral on any interval `[α, β]`.
-/
import Mathlib
import PolyhedralGravity.Edge
import PolyhedralGravity.Divergence

open MeasureTheory Set

namespace PolyhedralGravity

lemma mem_Icc_of_mem_pi_Ioo2 {c d x : Fin 2 → ℝ}
    (hx : x ∈ Set.pi univ fun i => Ioo (c i) (d i)) : x ∈ Icc c d := by
  simp only [Set.mem_pi, Set.mem_univ, forall_const, Set.mem_Ioo] at hx
  exact ⟨fun i => (hx i).1.le, fun i => (hx i).2.le⟩

/-- **A rectangular face** (Green's theorem from Mathlib's divergence theorem on a box of ℝ²). -/
theorem rect_face {c d p' : Fin 2 → ℝ} {h : ℝ} (hle : c ≤ d)
    (hpos : ∀ x ∈ Icc c d, 0 < sqDist p' (h ^ 2) x) :
    ∫ x in Icc c d, 1 / Real.sqrt (sqDist p' (h ^ 2) x) =
      (∑ i : Fin 2,
        ((d i - p' i) * (∫ z in Icc (c ∘ i.succAbove) (d ∘ i.succAbove),
            1 / Real.sqrt (sqDist p' (h ^ 2) (i.insertNth (d i) z))) -
          (c i - p' i) * (∫ z in Icc (c ∘ i.succAbove) (d ∘ i.succAbove),
            1 / Real.sqrt (sqDist p' (h ^ 2) (i.insertNth (c i) z))))) -
      h * (∫ x in Icc c d, h / (sqDist p' (h ^ 2) x * Real.sqrt (sqDist p' (h ^ 2) x))) := by
  have hd := fun x (hx : x ∈ Icc c d) => faceField_div (p := p') (h := h) (hpos x hx)
  have Hc : ContinuousOn (radialField p' (h ^ 2) (fun s => 1 / Real.sqrt s)) (Icc c d) :=
    fun x hx => (hd x hx).1.continuousAt.continuousWithinAt
  have hS : Continuous fun x : Fin 2 → ℝ => sqDist p' (h ^ 2) x := by unfold sqDist; fun_prop
  have hc1 : ContinuousOn (fun x => 1 / Real.sqrt (sqDist p' (h ^ 2) x)) (Icc c d) :=
    ContinuousOn.div (f := fun _ => (1 : ℝ)) (g := fun x => Real.sqrt (sqDist p' (h ^ 2) x))
      continuousOn_const hS.sqrt.continuousOn (fun x hx => (Real.sqrt_pos.2 (hpos x hx)).ne')
  have hc2 : ContinuousOn
      (fun x => h ^ 2 / (sqDist p' (h ^ 2) x * Real.sqrt (sqDist p' (h ^ 2) x))) (Icc c d) :=
    ContinuousOn.div (f := fun _ => h ^ 2)
      (g := fun x => sqDist p' (h ^ 2) x * Real.sqrt (sqDist p' (h ^ 2) x))
      continuousOn_const (hS.mul hS.sqrt).continuousOn
      (fun x hx => (mul_pos (hpos x hx) (Real.sqrt_pos.2 (hpos x hx))).ne')
  have hi1 := hc1.integrableOn_compact (μ := volume) isCompact_Icc
  have hi2 := hc2.integrableOn_compact (μ := volume) isCompact_Icc
  have Hi : IntegrableOn (fun x => ∑ i, radialDeriv p' (h ^ 2) (fun s => 1 / Real.sqrt s)
      (-1 / (2 * sqDist p' (h ^ 2) x * Real.sqrt (sqDist p' (h ^ 2) x))) x (Pi.single i 1) i)
      (Icc c d) :=
    (hi1.add hi2).congr_fun (fun x hx => ((hd x hx).2).symm) measurableSet_Icc
  have key := integral_divergence_of_hasFDerivAt_off_countable c d hle _ _ ∅
    Set.countable_empty Hc
    (fun x hx => (hd x (mem_Icc_of_mem_pi_Ioo2 hx.1)).1) Hi
  rw [setIntegral_congr_fun measurableSet_Icc (fun x hx => (hd x hx).2),
    integral_add hi1 hi2] at key
  have hω : ∫ x in Icc c d, h ^ 2 / (sqDist p' (h ^ 2) x * Real.sqrt (sqDist p' (h ^ 2) x)) =
      h * (∫ x in Icc c d, h / (sqDist p' (h ^ 2) x * Real.sqrt (sqDist p' (h ^ 2) x))) := by
    rw [← integral_const_mul]
    congr 1; funext x; ring
  have hsum : ∑ i : Fin 2,
      ((∫ z in Icc (c ∘ i.succAbove) (d ∘ i.succAbove),
          radialField p' (h ^ 2) (fun s => 1 / Real.sqrt s) (i.insertNth (d i) z) i) -
        ∫ z in Icc (c ∘ i.succAbove) (d ∘ i.succAbove),
          radialField p' (h ^ 2) (fun s => 1 / Real.sqrt s) (i.insertNth (c i) z) i) =
      ∑ i : Fin 2,
        ((d i - p' i) * (∫ z in Icc (c ∘ i.succAbove) (d ∘ i.succAbove),
            1 / Real.sqrt (sqDist p' (h ^ 2) (i.insertNth (d i) z))) -
          (c i - p' i) * (∫ z in Icc (c ∘ i.succAbove) (d ∘ i.succAbove),
            1 / Real.sqrt (sqDist p' (h ^ 2) (i.insertNth (c i) z)))) := by
    apply Finset.sum_congr rfl
    intro i _
    rw [← integral_const_mul, ← integral_const_mul]
    congr 1 <;>
    · congr 1
      funext z
      simp only [radialField, Fin.insertNth_apply_same]
  rw [hsum, hω] at key
  linarith

/-- An integral over a box of `Fin 1 → ℝ` (how Mathlib writes an edge of a rectangle) is an
interval integral. -/
theorem integral_Icc_fin_one (c d : Fin 1 → ℝ) (hcd : c 0 ≤ d 0) (f : ℝ → ℝ) :
    ∫ z in Icc c d, f (z 0) = ∫ s in (c 0)..(d 0), f s := by
  have hmp := MeasureTheory.volume_preserving_funUnique (Fin 1) ℝ
  have hpre : Icc c d = (MeasurableEquiv.funUnique (Fin 1) ℝ) ⁻¹' Icc (c 0) (d 0) := by
    ext z
    simp only [Set.mem_Icc, Set.mem_preimage, MeasurableEquiv.funUnique_apply, Pi.le_def,
      Fin.forall_fin_one]
    rfl
  rw [hpre]
  show ∫ z in _, f ((MeasurableEquiv.funUnique (Fin 1) ℝ) z) = _
  refine (hmp.setIntegral_preimage_emb (MeasurableEquiv.funUnique (Fin 1) ℝ).measurableEmbedding
    f (Icc (c 0) (d 0))).trans ?_
  rw [intervalIntegral.integral_of_le hcd, integral_Icc_eq_integral_Ioc]

/-- **The edge integral on any interval.** -/
theorem integral_inv_lineDist' {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (α β : ℝ) :
    ∫ t in α..β, 1 / lineDist t₀ ρ2 t =
      Real.log ((lineDist t₀ ρ2 α + lineDist t₀ ρ2 β + (β - α)) /
        (lineDist t₀ ρ2 α + lineDist t₀ ρ2 β - (β - α))) := by
  have hshift : ∀ t, lineDist t₀ ρ2 (t + α) = lineDist (t₀ - α) ρ2 t := by
    intro t; unfold lineDist; congr 1; ring
  have h1 : ∫ t in α..β, 1 / lineDist t₀ ρ2 t = ∫ t in (0 : ℝ)..(β - α), 1 / lineDist t₀ ρ2 (t + α) := by
    rw [intervalIntegral.integral_comp_add_right (fun t => 1 / lineDist t₀ ρ2 t)]
    simp
  rw [h1]
  simp_rw [hshift]
  rw [integral_inv_lineDist hρ]
  have e0 : lineDist (t₀ - α) ρ2 0 = lineDist t₀ ρ2 α := by
    rw [← hshift]; simp
  have eL : lineDist (t₀ - α) ρ2 (β - α) = lineDist t₀ ρ2 β := by
    rw [← hshift]; simp
  rw [e0, eL]

/-- The distance from `p'` (at height `h`) to the point of a rectangle's edge, in the edge's own
coordinate: `insertNth i v z` puts the constant coordinate `v` in slot `i`. -/
theorem sqDist_insertNth (p' : Fin 2 → ℝ) (k : ℝ) (i : Fin 2) (v : ℝ) (z : Fin 1 → ℝ) :
    sqDist p' k (i.insertNth v z) = (z 0 - p' (i.succAbove 0)) ^ 2 + ((v - p' i) ^ 2 + k) := by
  unfold sqDist
  rw [Fin.sum_univ_succAbove _ i]
  simp only [Fin.insertNth_apply_same, Fin.insertNth_apply_succAbove, Finset.univ_unique,
    Finset.sum_singleton, Fin.default_eq_zero]
  ring

/-- **A rectangle's edge in closed form**: the edge `x_i = v`, `x_{i'} ∈ [α, β]`, seen from a point
at height `h` above `p'`, off the edge's line. -/
theorem rect_edge (p' : Fin 2 → ℝ) (h : ℝ) (i : Fin 2) (v : ℝ) (c d : Fin 1 → ℝ)
    (hcd : c 0 ≤ d 0) (hoff : 0 < (v - p' i) ^ 2 + h ^ 2) :
    ∫ z in Icc c d, 1 / Real.sqrt (sqDist p' (h ^ 2) (i.insertNth v z)) =
      Real.log ((lineDist (p' (i.succAbove 0)) ((v - p' i) ^ 2 + h ^ 2) (c 0) +
          lineDist (p' (i.succAbove 0)) ((v - p' i) ^ 2 + h ^ 2) (d 0) + (d 0 - c 0)) /
        (lineDist (p' (i.succAbove 0)) ((v - p' i) ^ 2 + h ^ 2) (c 0) +
          lineDist (p' (i.succAbove 0)) ((v - p' i) ^ 2 + h ^ 2) (d 0) - (d 0 - c 0))) := by
  have : ∀ z : Fin 1 → ℝ, 1 / Real.sqrt (sqDist p' (h ^ 2) (i.insertNth v z)) =
      1 / lineDist (p' (i.succAbove 0)) ((v - p' i) ^ 2 + h ^ 2) (z 0) := by
    intro z; rw [sqDist_insertNth]; rfl
  simp_rw [this]
  rw [integral_Icc_fin_one c d hcd (fun s => 1 / lineDist (p' (i.succAbove 0)) ((v - p' i) ^ 2 + h ^ 2) s),
    integral_inv_lineDist' hoff]

end PolyhedralGravity
