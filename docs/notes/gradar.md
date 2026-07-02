# Cavendish — Gradar: passive gravimetric tracking as the JEPA target

The active/tracking evolution of the localisation note, and **the primary thing
the JEPA is for**. This settles scope-Q1: the JEPA's job is to **track the moving
sources** — reconstruct their time-resolved tracks (and how many there are) from
the array signal. ULDM detection sits beside it as the common-mode channel the
tracker learns to ignore. Destined for the inverse-model and §9 sections of
`cavendish.tex`. Builds on `multi-interferometer-localisation.md`.

---

## 0. Framing — radar, not static localisation

Localisation is "where is the source, given a signal." **Radar is the active,
tracking version:** a mass moves through the array's field of view and you
reconstruct its **track** — position over time, hence velocity, hence trajectory —
from how the gradient signature sweeps across the detectors.

The key idea: **the motion *is* the information.** A stationary mass gives one
geometry; a moving one sweeps through many, and that temporal diversity is what
makes the inverse well-posed — the same reason a single-axis gradiometer can
recover a trajectory at all (the sweep encodes what a snapshot cannot).

---

## 1. Why this is the natural JEPA target

It maps onto the temporal-JEPA framing almost too cleanly:

- Past gradient-signature windows → predicted future latent → the latent **must**
  encode the source's position and velocity, because that kinematic state is what
  generates the next window.
- Radar is literally "predict where it'll be" — which is what a **world-model
  latent** does. So this is not a detour from the JEPA plan; it is the most
  natural possible target for it.
- The JEPA ladder maps onto a tracking ladder: baseline supervised tracker →
  temporal JEPA (masked-future prediction, frozen probe reads off the track) →
  world-model tracker (past → predicted future latent → online tracking).
- The Cramér–Rao bound (§9) gives the rigorous floor: the tracker is measured
  against the CRB on the **track parameters**, not just a static position.

---

## 2. Passive, not active — the honest distinction (and the novelty)

Real radar is **active**: you emit, you get echoes, you range by time-of-flight.
This is **passive** — you do not illuminate anything; you read the mass's own
static gravitational field as it moves. Closer to passive sonar, or a gravimetric
tripwire.

- **No time-of-flight ranging.** Range comes from amplitude and the cross-detector
  triangulation from the localisation note (differential amplitude + relative
  timing + tensor orientation).
- This constraint is also what makes it novel: **learned passive gravitational
  tracking from an atom-interferometer array is unexplored.** Clean story — the
  array is a **phased gravimetric aperture**, the JEPA is the **tracker**.

---

## 3. Where it gets genuinely hard (where the ML earns its place)

This is what makes it a real experiment rather than a regression toy:

- **Multi-target / data association.** One mass is analytically tractable. Two or
  three moving masses **superpose linearly** in the field — trivial to *simulate*
  (just add the clouds) — but their tracks must be **disentangled** from the summed
  signal. Linearity helps the forward model and does **nothing** for the inverse;
  the hard part is the unmixing. This is exactly the data-association problem
  classical radar trackers (Kalman, JPDA) are built for, and exactly where a
  learned tracker can beat them.
- **Mass–distance degeneracy, broken by motion.** A heavy/slow/far mass and a
  light/fast/near one can *momentarily* alias, but their tracks diverge — so
  tracking-over-time resolves what a snapshot cannot. **Experiment design:** build
  confusion pairs that are *instantaneously* degenerate and test whether the
  temporal model separates them. This is the headline "does the world-model
  actually use time" result.
- **Detection-before-tracking.** Is there even a target in the noise, given the
  GGN and the data gaps? This folds the whole earlier pipeline (lift excision, the
  realistic schedule) back in as the **front-end** — so the Gradar problem
  *subsumes* the schedule layer rather than competing with it.

---

## 4. The unifying picture — one array, two channels

The array gives both halves of the problem from the same data, split by spatial
coherence (per the localisation note):

- **Differential / cross-channel structure → moving-mass GGN → the Gradar tracker.**
- **Common-mode across the array → coherent ULDM → the DM detector.**

So Gradar and ULDM detection are **not competing framings** — they
are the differential and common-mode halves of the same multi-channel problem. The
Gradar tracker naturally rejects the common-mode ULDM; the DM detector naturally
rejects the differential GGN.

---

## 5. What this pins down (scope-Q1, answered)

- **Q1 answered:** the JEPA characterises the moving sources, and the concrete form
  is **tracking** — the label is the full **time-resolved track** of each source
  plus the **target count**, not a static position or class alone.
- **ULDM** stays in scope as the parallel common-mode task (the closed-form signal
  source + a detection/amplitude label), beside the Gradar tracker.
- **The schedule layer** is now clearly in scope — it is the Gradar front-end
  (detection-before-tracking under GGN + gaps), not an optional add-on.

---

## 6. Architecture — changes the label, not the kernel

Almost nothing changes from the array note; what changes is **what the label is**.

- **Same multi-detector StateBundle, same gravity kernel.** The signal is still
  ΔΦ[detector, measurement]; the kernel still evaluates arbitrary field points.
- **Label becomes the time-resolved track(s):** per-source position-over-time (⇒
  velocity, trajectory) and the **number of sources**. For multi-target, the label
  is a *set* of tracks — which brings the set/association structure into the loss.
- **Scenario prior** must sample multi-target scenes (1, 2, 3 … masses on
  independent trajectories), including instantaneously-degenerate confusion pairs
  as a dedicated test family.
- **§9 identifiability** extends to the **track CRB** (covariance on position and
  velocity over time given array geometry and SNR) and a multi-target
  resolvability analysis (when are two tracks separable?).
- **Compute unchanged in kind** — the outer product already has the detector
  dimension; multi-target just sums more clouds per scene. Strengthens the
  GPU-primary call.

---

## 7. Where this lands in the doc (next actions)

- **Inverse-model section** — recast as the Gradar tracker: the tracking ladder
  (single → multi-target → detection-before-tracking) and the JEPA-as-world-model
  mapping; ULDM as the parallel common-mode task.
- **StateBundle / labels** — time-resolved track label; target count; set-valued
  labels for multi-target.
- **Scenario prior** — multi-target sampling + the instantaneously-degenerate
  confusion-pair test family.
- **§9** — track CRB and multi-target resolvability.
- **Schedule layer** — promote from "pending Q1" to in-scope, as the Gradar
  front-end (detection-before-tracking).
- **Naming** — the project/task is named **Gradar** (gravity + radar, and *grad* for
  the gradient operator ∇ / gradiometer); "passive gradiometric tracking" is the precise
  subtitle. Real radar (active, ranging) stays "radar" in analogy prose; *Gradar* names
  this system. Collision-checked: the only prior "gradar" is a fictional sci-fi device.
