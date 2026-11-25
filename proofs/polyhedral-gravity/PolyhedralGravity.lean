import PolyhedralGravity.Edge
import PolyhedralGravity.SolidAngle
import PolyhedralGravity.Divergence
import PolyhedralGravity.Box
import PolyhedralGravity.BoxFaces
import PolyhedralGravity.Polyhedron
import PolyhedralGravity.Superposition

/-! Every main theorem depends only on Lean's standard axioms (`propext`, `Classical.choice`,
`Quot.sound`): no `sorry`, no axiom of our own. `lake build` prints these. -/

#print axioms PolyhedralGravity.edge_integral
#print axioms PolyhedralGravity.edge_integral_log1p
#print axioms PolyhedralGravity.gap_near_edge_vec
#print axioms PolyhedralGravity.numerator_rule
#print axioms PolyhedralGravity.vos_modulus
#print axioms PolyhedralGravity.vos_ne_zero
#print axioms PolyhedralGravity.box_potential
#print axioms PolyhedralGravity.box_attraction
#print axioms PolyhedralGravity.box_gauss
#print axioms PolyhedralGravity.rect_face
#print axioms PolyhedralGravity.rect_edge
#print axioms PolyhedralGravity.potential_faces_edges
#print axioms PolyhedralGravity.attraction_faces_edges
#print axioms PolyhedralGravity.solid_angles_sum_zero
#print axioms PolyhedralGravity.quadrature_error_additive
#print axioms PolyhedralGravity.exact_pieces_exact_phase
#print axioms PolyhedralGravity.interior_and_boundary
