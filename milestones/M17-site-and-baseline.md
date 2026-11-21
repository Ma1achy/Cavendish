# M17 — The site, its noise sources, and a baseline tracker (plan / implementation brief)

> **Status: planned — nothing here is built.** Depends on: M11, M15 (the scene spec carries
> populations), M16 (manifests). Geometry and rates below come from published AION sources; anything
> not in them is marked **to confirm with George**.
>
> **Goal.** Scenes that look like where the instrument will actually stand, with the noise sources
> the collaboration itself models, plus two that are not modelled yet; and a non-learned tracker so
> there is something to measure Gradar against besides the predict-the-mean baseline.

---

## 1. What the sources say

- **Site.** AION-10 stands in the stairwell of the Beecroft Building, Oxford. The stairwell runs from
  the ground-floor atrium to underground labs (floors G, B1, B2), about 15 m deep; the tower base is
  on B2 (AION-10 Technical Design Report, arXiv:2508.03491). So the 10 m tower spans floors, and
  sources are at different heights — on stairs beside it, on floors above its base, in a lift.
- **The collaboration's own noise scene** (Carlton & McCabe et al., "From RATs to riches",
  arXiv:2308.10731) — which Cavendish should be able to reproduce as a library scene:

| Population | Mass | Motion | Closest approach | Rate |
|---|---|---|---|---|
| people on the stairs | 50–100 kg | walk to the base at 1 m/s, climb at 0.5 m/s, leave | adjacent to the tower — the dominant source | ~5 per hour by day; closed at night |
| people in the foyer | 50–100 kg | 1 m/s along the foyer | ≥5 m | up to 100 per hour (25 midday); up to 10 at night |
| passenger lift | 1000–2000 kg | from rest at the base, up past the top at 2.5 m/s, stop | beside the shaft | ~20 moves per hour at peak, 5 midday, 1 at night |
| cars | 1000–5000 kg | 10 m/s on a straight road | ≥20 m | ~60 per hour by day; up to 10 at night |
| small animals | ~0.25 kg, five of them | pseudo-random paths on a 10 × 10 m area 15 m below the launch point | — | ~300 paths each per night |

  Their atom shot noise is ~250 µrad per shot (Cavendish's proof of concept used 1 mrad). They report
  the small animals at ~50 µrad — under shot noise but not negligible — and that seismic noise was
  not modelled.
- **Not in either source, to confirm:** the stair geometry (flights, landings, distance from the
  tower axis), the lift shaft position, the road's bearing, the cycle time.

## 2. Requirements

| ID | Requirement |
|---|---|
| M17-R1 | **Populations.** A `Population` in the scene spec: an arrival process (Poisson with a rate that varies over the day), a mass distribution, a body template (point, primitive, walker, tree body) and a path family. Generalises v1's Poisson lift transients. Deterministic per `(seed, population, event index)`. |
| M17-R2 | **Paths that buildings need**: piecewise-linear with per-segment speed (stairs, corridors), with rounded corners so velocity is continuous; the existing `Lift` profile. |
| M17-R3 | **The stairwell scene** — `library::beecroft_stairwell(day | night)` with the five populations above at the published rates, and the published geometry where it exists. Reproduces the published qualitative results (§4). |
| M17-R4 | **Structure.** Static building mass (floor slabs, stair structure) as tree bodies: a constant offset per tower, which matters only through its interaction with tower placement error and tilt — included so those studies are possible. |
| M17-R5 | **Seismic Newtonian noise** as an analytic ambient field: a superposition of Rayleigh plane waves in a half-space, with directions and phases drawn per seed and amplitudes from a displacement spectrum supplied as data. |
| M17-R6 | **A baseline tracker** that uses the forward model's linearity and no training: (a) a matched-filter bank over constant-velocity tracks; (b) a regularised linear inversion onto a coarse mass grid per time window. Reported beside Gradar's error and the M8 Cramér–Rao bound on the same test sets. |

## 3. Design notes

- **Populations are scene-spec data, not code** (`SceneSpec.populations`), so the GUI's dataset
  builder and Python can vary rates and mass ranges without recompiling. Each event becomes an
  ordinary `WorldSource`; nothing in `generate` changes.
- **Day and night** are rate tables. A week of schedule with realistic gaps is George's
  `week_pipeline` case, still to be ported; this milestone supplies the scene it needs.
- **Seismic noise.** Implement from the standard treatment (Harms, *Terrestrial gravity
  fluctuations*, Living Reviews in Relativity; Hughes & Thorne 1998): a Rayleigh wave perturbs
  gravity through the displaced surface and the compressed ground. Verify the implementation against
  that reference's closed form for a single plane wave before trusting a superposition — the formula
  is deliberately not restated here. The site measurements in the TDR show machinery peaks at
  30–100 Hz, far above the band a 1.46 s flight responds to; the part that could matter is the
  microseism near 0.1–0.3 Hz, for which **no site spectrum is in hand**. Until one is, this is a
  capability with a placeholder spectrum, labelled as such in any dataset that uses it.
- **The baseline.** For one point mass the signal is `m · w(x(t))`, with `w` the per-tower response
  to a unit mass at `x` — linear in mass, non-linear in position. (a) evaluates a bank of
  straight-line tracks and picks the best correlation, refining by Gauss–Newton with the M8
  Jacobians; (b) solves non-negative least squares for mass on a coarse grid in a sliding window and
  takes the centroid. Both are a few hundred lines on existing kernels.

## 4. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `population_is_keyed_and_reproducible` | same seed → same events for any batch order; arrival counts match the rate (χ² over 10⁴ hours) | exact / p > 0.01 |
| unit | `rate_table_switches_day_and_night` | no stair events at night; lift at one per hour | exact |
| unit | `polyline_path_is_continuous` | position and velocity continuous through corners; segment speeds as specified | ≤1e-12 |
| unit | `rayleigh_single_wave_matches_reference` | one plane wave vs the reference closed form, at and below the surface | ≤1e-9 |
| unit | `seismic_field_is_ambient_and_superposes` | on the ambient channel; N waves = sum of each | ≤1e-12 |
| unit | `matched_filter_recovers_a_clean_track` | noiseless constant-velocity pass | ≤1 mm |
| unit | `inversion_conserves_mass` | recovered total mass on a noiseless scene | 1% |
| integration | `stairwell_reproduces_published_ordering` | phase amplitude: stairs > lift, foyer > cars > animals at the published geometry; animals 20–100 µrad | ordering / range |
| integration | `far_field_scales_as_inverse_cube` | a car's peak phase vs closest approach, 20–80 m | exponent −3 ± 0.1 |
| integration | `masking_statistics_are_plausible` | with the published transient rule (rolling σ, mean + 5σ), the daytime masked fraction is within a factor of two of the published ~16% | factor 2 |
| e2e | `baseline_vs_bound_vs_model` | on the proof-of-concept test sets: CRB ≤ baseline error; the table is produced | structural |

## 5. Exit requirements

| Requirement | Check |
|---|---|
| the collaboration's scene runs here (R1–R3) | `stairwell_reproduces_published_ordering`, `masking_statistics_are_plausible` |
| populations are data and reproducible (R1) | `population_is_keyed_and_reproducible` |
| seismic noise is implemented correctly, and honestly labelled (R5) | `rayleigh_single_wave_matches_reference`; the placeholder-spectrum flag reaches the manifest |
| there is a non-learned yardstick (R6) | `baseline_vs_bound_vs_model` table in the repo |
| George has signed off the geometry | the "to confirm" list in §1 is empty |

## 6. Open questions

- Cavendish's array is several towers; AION-10 is one. Whether the stairwell scene is run with one
  tower (to match the paper) and separately with a hypothetical array around the stairwell, or only
  the latter, decides how directly the published numbers can be compared.
- Whether rodent-scale sources need anything beyond point masses (no).
- A counterweight on the lift — the paper omits it; a real lift has one, moving the other way.
