# Cavendish

<p>
<a href="plan/tasks.yaml">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/Ma1achy/Cavendish/progress/progress-dark.png">
  <img src="https://raw.githubusercontent.com/Ma1achy/Cavendish/progress/progress-light.png" alt="Build progress: one square per task of the plan, by slice from S0 to S12, inked when merged, ochre when in review and outlined when not started, with each slice's share done and the overall percentage." width="100%">
</picture>
</a>
</p>

A voxel world engine around an array of atom-interferometer gravity gradiometers, rebuilt from a blank
slate. Start with `plan/REBUILD.md`; how the build runs is `plan/KICKOFF.md`; where it stands is
`plan/PROGRESS.md`. The browser mock-up of the viewer is in `docs/viewer-mockup/`.

The previous code is kept in history: tag `v1-final` (the validated v1 forward model, used as the
oracle) and tag `v2-prototype` (the world-physics prototype; branch `v2-world-physics`, commit `71ec48a`). Their design documents are in `reference/`.
