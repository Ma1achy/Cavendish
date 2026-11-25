/-
The edge term of the polyhedral closed form.

For a straight segment `a + t • d`, `t ∈ [0, L]`, `‖d‖ = 1`, and a point `p` off its line,

  ∫₀ᴸ dt / ‖p − (a + t • d)‖ = ln ((r₁ + r₂ + L) / (r₁ + r₂ − L)),

with `r₁ = ‖p − a‖` and `r₂ = ‖p − (a + L • d)‖` the distances to the ends (Werner & Scheeres
1997, eq. 7). Also the two forms Cavendish computes it in: `log1p (2L / (r₁ + r₂ − L))`, and the
near-edge form of the gap `r₁ + r₂ − L = r₀² / (r₁ − ℓ₁) + r₀² / (r₂ + ℓ₂)` (Holstein & Ketteridge
1996, eqs. 27–28), which has no difference of near-equal numbers when the point projects onto the
segment.

Nothing here is assumed: the integral is Mathlib's interval integral, evaluated by the fundamental
theorem of calculus from an explicit antiderivative.
-/
import Mathlib

open Real MeasureTheory intervalIntegral

namespace PolyhedralGravity

/-- Distance from a point to the point of a line at parameter `t`, written in the line's own
coordinate: `t₀` is the foot of the perpendicular and `ρ2 = r₀²` the squared distance to the line. -/
noncomputable def lineDist (t₀ ρ2 t : ℝ) : ℝ := Real.sqrt ((t - t₀) ^ 2 + ρ2)

lemma lineDist_sq {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (t : ℝ) : lineDist t₀ ρ2 t ^ 2 = (t - t₀) ^ 2 + ρ2 :=
  Real.sq_sqrt (by positivity)

lemma lineDist_pos {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (t : ℝ) : 0 < lineDist t₀ ρ2 t :=
  Real.sqrt_pos.2 (by positivity)

/-- `t − t₀ < r(t)`: strictly, because the point is off the line. -/
lemma sub_lt_lineDist {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (t : ℝ) : t - t₀ < lineDist t₀ ρ2 t := by
  have h1 := lineDist_sq (t₀ := t₀) hρ t
  have h2 := lineDist_pos (t₀ := t₀) hρ t
  nlinarith [sq_nonneg (t - t₀ - lineDist t₀ ρ2 t)]

/-- `−(t − t₀) < r(t)`. -/
lemma neg_sub_lt_lineDist {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (t : ℝ) : -(t - t₀) < lineDist t₀ ρ2 t := by
  have h1 := lineDist_sq (t₀ := t₀) hρ t
  have h2 := lineDist_pos (t₀ := t₀) hρ t
  nlinarith [sq_nonneg (t - t₀ + lineDist t₀ ρ2 t)]

/-- The antiderivative `ln (t − t₀ + r(t))` (that is, `arsinh ((t − t₀)/r₀)` up to a constant). -/
noncomputable def edgePrim (t₀ ρ2 t : ℝ) : ℝ := Real.log (t - t₀ + lineDist t₀ ρ2 t)

lemma hasDerivAt_edgePrim {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (t : ℝ) :
    HasDerivAt (edgePrim t₀ ρ2) (1 / lineDist t₀ ρ2 t) t := by
  have hs : (t - t₀) ^ 2 + ρ2 ≠ 0 := by positivity
  have hq : HasDerivAt (fun y : ℝ => (y - t₀) ^ 2 + ρ2) (2 * (t - t₀)) t := by
    have := ((hasDerivAt_id t).sub_const t₀).pow 2
    simpa using this.add_const ρ2
  have hr : HasDerivAt (fun y : ℝ => lineDist t₀ ρ2 y)
      (2 * (t - t₀) / (2 * Real.sqrt ((t - t₀) ^ 2 + ρ2))) t := hq.sqrt hs
  have hsum : HasDerivAt (fun y : ℝ => y - t₀ + lineDist t₀ ρ2 y)
      (1 + 2 * (t - t₀) / (2 * Real.sqrt ((t - t₀) ^ 2 + ρ2))) t :=
    ((hasDerivAt_id t).sub_const t₀).add hr
  have hpos : 0 < t - t₀ + lineDist t₀ ρ2 t := by linarith [neg_sub_lt_lineDist (t₀ := t₀) hρ t]
  have hlog := hsum.log hpos.ne'
  refine hlog.congr_deriv ?_
  have hr0 : 0 < lineDist t₀ ρ2 t := lineDist_pos hρ t
  change (1 + 2 * (t - t₀) / (2 * lineDist t₀ ρ2 t)) / (t - t₀ + lineDist t₀ ρ2 t)
    = 1 / lineDist t₀ ρ2 t
  field_simp
  ring

lemma continuous_inv_lineDist {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) :
    Continuous (fun t => 1 / lineDist t₀ ρ2 t) := by
  refine continuous_const.div ?_ (fun t => (lineDist_pos hρ t).ne')
  show Continuous fun t => Real.sqrt ((t - t₀) ^ 2 + ρ2)
  fun_prop

/-- **The edge integral in line coordinates.** -/
theorem integral_inv_lineDist {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (L : ℝ) :
    ∫ t in (0 : ℝ)..L, 1 / lineDist t₀ ρ2 t =
      Real.log ((lineDist t₀ ρ2 0 + lineDist t₀ ρ2 L + L) /
        (lineDist t₀ ρ2 0 + lineDist t₀ ρ2 L - L)) := by
  rw [integral_eq_sub_of_hasDerivAt (fun t _ => hasDerivAt_edgePrim hρ t)
    ((continuous_inv_lineDist hρ).intervalIntegrable 0 L)]
  unfold edgePrim
  set r₁ := lineDist t₀ ρ2 0
  set r₂ := lineDist t₀ ρ2 L
  have h1 : r₁ ^ 2 = (0 - t₀) ^ 2 + ρ2 := lineDist_sq hρ 0
  have h2 : r₂ ^ 2 = (L - t₀) ^ 2 + ρ2 := lineDist_sq hρ L
  have a1 : 0 < 0 - t₀ + r₁ := by linarith [neg_sub_lt_lineDist (t₀ := t₀) hρ 0]
  have a2 : 0 < L - t₀ + r₂ := by linarith [neg_sub_lt_lineDist (t₀ := t₀) hρ L]
  have b1 : 0 - t₀ < r₁ := sub_lt_lineDist hρ 0
  have b2 : -(L - t₀) < r₂ := neg_sub_lt_lineDist hρ L
  have b3 : L - t₀ < r₂ := sub_lt_lineDist hρ L
  have hgap : 0 < r₁ + r₂ - L := by
    have := neg_sub_lt_lineDist (t₀ := t₀) hρ 0
    linarith
  rw [← Real.log_div a2.ne' a1.ne']
  congr 1
  rw [div_eq_div_iff a1.ne' hgap.ne']
  nlinarith [h1, h2]

/-- The gap `r₁ + r₂ − L` is positive for a point off the line. -/
lemma gap_pos {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (L : ℝ) :
    0 < lineDist t₀ ρ2 0 + lineDist t₀ ρ2 L - L := by
  have := neg_sub_lt_lineDist (t₀ := t₀) hρ 0
  have := sub_lt_lineDist (t₀ := t₀) hρ L
  linarith

/-- **The log1p form** that Cavendish evaluates: `ln ((S + L)/(S − L)) = log1p (2L/(S − L))`. -/
lemma log_ratio_eq_log_one_add {S L : ℝ} (h : 0 < S - L) :
    Real.log ((S + L) / (S - L)) = Real.log (1 + 2 * L / (S - L)) := by
  congr 1
  field_simp
  ring

/-- **The near-edge form of the gap** (Holstein & Ketteridge's `r − |ℓ| = r₀²/(r + |ℓ|)`, applied
at both ends). With `ℓ₁ = −t₀` and `ℓ₂ = L − t₀` the signed positions of the ends along the edge
measured from the foot of the perpendicular, `r₁ + r₂ − L = r₀²/(r₁ − ℓ₁) + r₀²/(r₂ + ℓ₂)`. Both
denominators are positive everywhere off the line; when the point projects onto the segment
(`ℓ₁ ≤ 0 ≤ ℓ₂`) each is a sum of two non-negative numbers, so nothing cancels. -/
theorem gap_near_edge {t₀ ρ2 : ℝ} (hρ : 0 < ρ2) (L : ℝ) :
    lineDist t₀ ρ2 0 + lineDist t₀ ρ2 L - L =
      ρ2 / (lineDist t₀ ρ2 0 - (0 - t₀)) + ρ2 / (lineDist t₀ ρ2 L + (L - t₀)) := by
  have h1 := lineDist_sq (t₀ := t₀) hρ 0
  have h2 := lineDist_sq (t₀ := t₀) hρ L
  have d1 : 0 < lineDist t₀ ρ2 0 - (0 - t₀) := by linarith [sub_lt_lineDist (t₀ := t₀) hρ 0]
  have d2 : 0 < lineDist t₀ ρ2 L + (L - t₀) := by linarith [neg_sub_lt_lineDist (t₀ := t₀) hρ L]
  field_simp
  nlinarith [h1, h2]

/-! ### The same, for a segment in any real inner-product space (ℝ³ in particular) -/

section Vector

variable {E : Type*} [NormedAddCommGroup E] [InnerProductSpace ℝ E]

/-- The distance from `p` to `a + t • d` in line coordinates. -/
lemma norm_sub_line {a d p : E} (hd : ‖d‖ = 1) (t : ℝ) :
    ‖p - (a + t • d)‖ = lineDist (inner ℝ (p - a) d) (‖p - a‖ ^ 2 - inner ℝ (p - a) d ^ 2) t := by
  unfold lineDist
  rw [← Real.sqrt_sq (norm_nonneg _)]
  congr 1
  have : p - (a + t • d) = (p - a) - t • d := by abel
  rw [this, norm_sub_sq_real, inner_smul_right, norm_smul, hd, Real.norm_eq_abs, mul_one, sq_abs]
  ring

/-- Off the line, the squared distance to it, `r₀² = ‖p − a‖² − ⟪p − a, d⟫²`, is positive. -/
lemma rho2_pos {a d p : E} (hd : ‖d‖ = 1) (hoff : ∀ t : ℝ, p ≠ a + t • d) :
    0 < ‖p - a‖ ^ 2 - inner ℝ (p - a) d ^ 2 := by
  set t₀ := inner ℝ (p - a) d
  have key : ‖(p - a) - t₀ • d‖ ^ 2 = ‖p - a‖ ^ 2 - t₀ ^ 2 := by
    rw [norm_sub_sq_real, inner_smul_right, norm_smul, hd, Real.norm_eq_abs, mul_one, sq_abs]
    ring
  rw [← key]
  have hne : (p - a) - t₀ • d ≠ 0 := by
    intro h
    apply hoff t₀
    rw [sub_eq_zero] at h
    rw [← h]
    abel
  positivity

/-- The end distances and every distance along the segment, in line coordinates. -/
lemma line_coords {a d p : E} (hd : ‖d‖ = 1) :
    ‖p - a‖ = lineDist (inner ℝ (p - a) d) (‖p - a‖ ^ 2 - inner ℝ (p - a) d ^ 2) 0 := by
  have := norm_sub_line (p := p) (a := a) hd 0
  simpa using this

/-- **(a) The edge term.** For a segment `a + t • d`, `t ∈ [0, L]`, with `‖d‖ = 1`, and a point
`p` off its line,
`∫₀ᴸ dt / ‖p − (a + t d)‖ = ln ((r₁ + r₂ + L)/(r₁ + r₂ − L))`. -/
theorem edge_integral {a d p : E} (hd : ‖d‖ = 1) (hoff : ∀ t : ℝ, p ≠ a + t • d) (L : ℝ) :
    ∫ t in (0 : ℝ)..L, 1 / ‖p - (a + t • d)‖ =
      Real.log ((‖p - a‖ + ‖p - (a + L • d)‖ + L) / (‖p - a‖ + ‖p - (a + L • d)‖ - L)) := by
  have hρ := rho2_pos hd hoff
  have hint := fun t : ℝ => norm_sub_line (p := p) (a := a) hd t
  have h0 := line_coords (p := p) (a := a) hd
  generalize inner ℝ (p - a) d = t₀ at hρ hint h0
  generalize ‖p - a‖ ^ 2 - t₀ ^ 2 = ρ2 at hρ hint h0
  simp_rw [hint]
  rw [h0]
  exact integral_inv_lineDist hρ L

/-- The same in the form Cavendish evaluates, `log1p (2L / (r₁ + r₂ − L))`. -/
theorem edge_integral_log1p {a d p : E} (hd : ‖d‖ = 1) (hoff : ∀ t : ℝ, p ≠ a + t • d) (L : ℝ) :
    ∫ t in (0 : ℝ)..L, 1 / ‖p - (a + t • d)‖ =
      Real.log (1 + 2 * L / (‖p - a‖ + ‖p - (a + L • d)‖ - L)) := by
  rw [edge_integral hd hoff, log_ratio_eq_log_one_add]
  have hρ := rho2_pos hd hoff
  have hL := norm_sub_line (p := p) (a := a) hd L
  have h0 := line_coords (p := p) (a := a) hd
  generalize inner ℝ (p - a) d = t₀ at hρ hL h0
  generalize ‖p - a‖ ^ 2 - t₀ ^ 2 = ρ2 at hρ hL h0
  rw [h0, hL]
  exact gap_pos hρ L

/-- **The near-edge form in vector terms.** With `ℓ₁ = ⟪a − p, d⟫`, `ℓ₂ = ⟪a + L d − p, d⟫` and
`r₀² = ‖p − a‖² − ⟪p − a, d⟫²`:  `r₁ + r₂ − L = r₀²/(r₁ − ℓ₁) + r₀²/(r₂ + ℓ₂)`. -/
theorem gap_near_edge_vec {a d p : E} (hd : ‖d‖ = 1) (hoff : ∀ t : ℝ, p ≠ a + t • d) (L : ℝ) :
    ‖p - a‖ + ‖p - (a + L • d)‖ - L =
      (‖p - a‖ ^ 2 - inner ℝ (p - a) d ^ 2) / (‖p - a‖ - inner ℝ (a - p) d) +
      (‖p - a‖ ^ 2 - inner ℝ (p - a) d ^ 2) / (‖p - (a + L • d)‖ + inner ℝ (a + L • d - p) d) := by
  have hρ := rho2_pos hd hoff
  have e1 : inner ℝ (a - p) d = 0 - inner ℝ (p - a) d := by
    rw [← neg_sub p a, inner_neg_left]; ring
  have e2 : inner ℝ (a + L • d - p) d = L - inner ℝ (p - a) d := by
    have : a + L • d - p = L • d - (p - a) := by abel
    rw [this, inner_sub_left, inner_smul_left, real_inner_self_eq_norm_sq, hd]
    simp
  rw [e1, e2]
  have hL := norm_sub_line (p := p) (a := a) hd L
  have h0 := line_coords (p := p) (a := a) hd
  generalize inner ℝ (p - a) d = t₀ at hρ hL h0 ⊢
  generalize ‖p - a‖ ^ 2 - t₀ ^ 2 = ρ2 at hρ hL h0 ⊢
  rw [hL, h0]
  exact gap_near_edge hρ L

end Vector

end PolyhedralGravity
