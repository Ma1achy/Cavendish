# Viewer mock-up

The browser mock-up of the egui viewer, also published as a private artifact. It replays one recording
made by `crates/generate/examples/viewer_poc_export.rs` (walker crossing the five-tower array, radiator
heating the room); the octree bodies, LOD cut, adaptive air grid and colour fields are computed in the
page. Serve the folder (`python3 -m http.server`) and open `scene-viewer.html`; it loads three.js r128
from cdnjs. It is the reference for the rebuild's look and interaction (`plan/REBUILD.md` §6).
