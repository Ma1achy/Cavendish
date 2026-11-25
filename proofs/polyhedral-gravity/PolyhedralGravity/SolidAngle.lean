/-
The face term's solid angle: the algebra behind the formula Cavendish evaluates.

For a triangle `v₁ v₂ v₃` (anticlockwise seen from outside) and a field point `p`, with
`rᵢ = vᵢ − p`, Van Oosterom & Strackee (1983) give the signed solid angle as

  ω = 2 atan2 (N, D),   N = r₁ · (r₂ × r₃),
                        D = r₁r₂r₃ + r₁ (r₂·r₃) + r₂ (r₃·r₁) + r₃ (r₁·r₂).

That formula itself (that `2 atan2 (N, D)` equals the surface integral `∫ h/r³ dS` over the
triangle) is **not** proven here; it is cited. What is proven:

* `triple_eq_edges`, `numerator_rule`: the numerator `r₁·(r₂×r₃)` equals `h · 2A`, the height of
  the point above the face's plane times twice the face's area, formed from the face's own edge
  vectors. This is the rewrite that keeps far-field rounding at ε(R/s)² (the research note, §1.1).
* `height_const`: the height is the same from every vertex (and so from any point of the plane).
* `vos_modulus`: `D² + N² = 2 (r₁r₂ + r₁·r₂)(r₂r₃ + r₂·r₃)(r₃r₁ + r₃·r₁)`, and each factor is
  non-negative (Cauchy–Schwarz). A factor vanishes only when two of the `rᵢ` are antiparallel (or
  one is zero), i.e. when the point lies on a closed edge of the triangle; everywhere else
  `(D, N) ≠ (0, 0)` and `atan2` is well defined (`vos_ne_zero`).
-/
import Mathlib

open Matrix

namespace PolyhedralGravity

/-- The triple product of the offsets equals that of the edge vectors from the first vertex. -/
theorem triple_eq_edges (v₁ v₂ v₃ p : Fin 3 → ℝ) :
    (v₁ - p) ⬝ᵥ ((v₂ - p) ⨯₃ (v₃ - p)) = (v₁ - p) ⬝ᵥ ((v₂ - v₁) ⨯₃ (v₃ - v₁)) := by
  simp only [cross_apply, dotProduct, Fin.sum_univ_three, Pi.sub_apply, Matrix.cons_val_zero,
    Matrix.cons_val_one, Matrix.cons_val_two, Matrix.head_cons, Matrix.tail_cons]
  ring

/-- The Euclidean length of a vector of ℝ³ (Mathlib's norm on `Fin 3 → ℝ` is the sup norm). -/
noncomputable def len3 (v : Fin 3 → ℝ) : ℝ := Real.sqrt (v ⬝ᵥ v)

lemma dot_self_nonneg (v : Fin 3 → ℝ) : 0 ≤ v ⬝ᵥ v := by
  simp only [dotProduct, Fin.sum_univ_three]
  nlinarith [mul_self_nonneg (v 0), mul_self_nonneg (v 1), mul_self_nonneg (v 2)]

lemma len3_sq (v : Fin 3 → ℝ) : len3 v ^ 2 = v ⬝ᵥ v := Real.sq_sqrt (dot_self_nonneg v)

/-- **The numerator rule.** With `N = (v₂ − v₁) × (v₃ − v₁)` (so `|N| = 2A`), unit normal
`n = N/|N|` and height `h = n·(v₁ − p)`: `r₁·(r₂×r₃) = h · |N|`. -/
theorem numerator_rule (v₁ v₂ v₃ p : Fin 3 → ℝ)
    (hN : 0 < len3 ((v₂ - v₁) ⨯₃ (v₃ - v₁))) :
    (v₁ - p) ⬝ᵥ ((v₂ - p) ⨯₃ (v₃ - p)) =
      (((1 / len3 ((v₂ - v₁) ⨯₃ (v₃ - v₁))) • ((v₂ - v₁) ⨯₃ (v₃ - v₁))) ⬝ᵥ (v₁ - p)) *
        len3 ((v₂ - v₁) ⨯₃ (v₃ - v₁)) := by
  have hN0 := hN.ne'
  rw [triple_eq_edges, smul_dotProduct, smul_eq_mul, dotProduct_comm]
  field_simp

/-- The height above the face's plane is the same from every vertex: `N·(vᵢ − p)` does not depend
on `i`. -/
theorem height_const (v₁ v₂ v₃ p : Fin 3 → ℝ) :
    ((v₂ - v₁) ⨯₃ (v₃ - v₁)) ⬝ᵥ (v₂ - p) = ((v₂ - v₁) ⨯₃ (v₃ - v₁)) ⬝ᵥ (v₁ - p) ∧
      ((v₂ - v₁) ⨯₃ (v₃ - v₁)) ⬝ᵥ (v₃ - p) = ((v₂ - v₁) ⨯₃ (v₃ - v₁)) ⬝ᵥ (v₁ - p) := by
  constructor <;>
  · simp only [cross_apply, dotProduct, Fin.sum_univ_three, Pi.sub_apply, Matrix.cons_val_zero,
      Matrix.cons_val_one, Matrix.cons_val_two, Matrix.head_cons, Matrix.tail_cons]
    ring

/-- The square of a triple product is the Gram determinant. -/
lemma triple_sq_gram (A B C : Fin 3 → ℝ) :
    (A ⬝ᵥ (B ⨯₃ C)) ^ 2 =
      (A ⬝ᵥ A) * (B ⬝ᵥ B) * (C ⬝ᵥ C) + 2 * (B ⬝ᵥ C) * (C ⬝ᵥ A) * (A ⬝ᵥ B)
        - (A ⬝ᵥ A) * (B ⬝ᵥ C) ^ 2 - (B ⬝ᵥ B) * (C ⬝ᵥ A) ^ 2 - (C ⬝ᵥ C) * (A ⬝ᵥ B) ^ 2 := by
  simp only [cross_apply, dotProduct, Fin.sum_univ_three, Matrix.cons_val_zero,
    Matrix.cons_val_one, Matrix.cons_val_two, Matrix.head_cons, Matrix.tail_cons]
  ring

/-- **The modulus identity behind Van Oosterom & Strackee's formula.** -/
theorem vos_modulus (A B C : Fin 3 → ℝ) :
    (len3 A * len3 B * len3 C + len3 A * (B ⬝ᵥ C) + len3 B * (C ⬝ᵥ A) + len3 C * (A ⬝ᵥ B)) ^ 2 +
        (A ⬝ᵥ (B ⨯₃ C)) ^ 2 =
      2 * (len3 A * len3 B + A ⬝ᵥ B) * (len3 B * len3 C + B ⬝ᵥ C) *
        (len3 C * len3 A + C ⬝ᵥ A) := by
  rw [triple_sq_gram, ← len3_sq A, ← len3_sq B, ← len3_sq C]
  ring

/-- Cauchy–Schwarz in the form the solid angle needs: `|A||B| + A·B ≥ 0`. -/
lemma len_mul_add_dot_nonneg (A B : Fin 3 → ℝ) : 0 ≤ len3 A * len3 B + A ⬝ᵥ B := by
  have ha := Real.sqrt_nonneg (A ⬝ᵥ A)
  have hb := Real.sqrt_nonneg (B ⬝ᵥ B)
  have hA := len3_sq A
  have hB := len3_sq B
  -- (A·B)² ≤ |A|²|B|² (Lagrange's identity in coordinates)
  have cs : (A ⬝ᵥ B) ^ 2 ≤ (A ⬝ᵥ A) * (B ⬝ᵥ B) := by
    simp only [dotProduct, Fin.sum_univ_three]
    nlinarith [sq_nonneg (A 0 * B 1 - A 1 * B 0), sq_nonneg (A 1 * B 2 - A 2 * B 1),
      sq_nonneg (A 0 * B 2 - A 2 * B 0)]
  have hab : 0 ≤ len3 A * len3 B := mul_nonneg ha hb
  rw [← hA, ← hB] at cs
  have h2 : (A ⬝ᵥ B) ^ 2 ≤ (len3 A * len3 B) ^ 2 := by rw [mul_pow]; exact cs
  linarith [(abs_le_of_sq_le_sq' h2 hab).1]

/-- Away from the closed edges (every factor positive) the pair `(D, N)` is not `(0, 0)`, so
`atan2 (N, D)` is well defined. -/
theorem vos_ne_zero (A B C : Fin 3 → ℝ)
    (hAB : 0 < len3 A * len3 B + A ⬝ᵥ B) (hBC : 0 < len3 B * len3 C + B ⬝ᵥ C)
    (hCA : 0 < len3 C * len3 A + C ⬝ᵥ A) :
    0 < (len3 A * len3 B * len3 C + len3 A * (B ⬝ᵥ C) + len3 B * (C ⬝ᵥ A) +
        len3 C * (A ⬝ᵥ B)) ^ 2 + (A ⬝ᵥ (B ⨯₃ C)) ^ 2 := by
  rw [vos_modulus A B C]
  positivity

end PolyhedralGravity
