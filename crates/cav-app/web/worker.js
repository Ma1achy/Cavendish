// A Web Worker of the forward stage (crates/cav-app/src/platform/web.rs): the same module as the
// page, so that a voxel body's seconds of work never run on the page's thread. The page sends the
// module it compiled first; the module's own start then answers every job that follows.
import init from "./cavendish.js";

self.addEventListener("message", (event) => {
  init({ module_or_path: event.data.module ?? undefined }).catch((error) => {
    console.error("the forward stage's worker could not start:", error);
    throw error;
  });
}, { once: true });
