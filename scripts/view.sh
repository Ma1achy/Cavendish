#!/usr/bin/env bash
# Launch the wgpu viewer inside the dev container (software rendering).
#
# Lavapipe (software Vulkan, the container default for the compute tests) cannot *present* to a
# window under Xvfb, so the on-screen viewer uses software OpenGL (llvmpipe) via wgpu's GL backend.
# This overrides WGPU_BACKEND for the viewer only — the container-wide Vulkan default is untouched.
#
# View it in a browser: open the forwarded port 6080 (noVNC, password: vscode).
# On a real display (e.g. the macOS host with Rust installed), just run `cargo run -p viewer`.
set -euo pipefail

export WGPU_BACKEND=gl
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/runtime-$(id -un)}"
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

exec cargo run -p viewer "$@"
