# Cavendish dev container.
# Rust + wgpu (software Vulkan for headless) + Python/maturin for the SDK.
# The spec's LaTeX build lives in CI, not here, to keep the image lean.
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive

# System deps:
#  - mesa-vulkan-drivers + vulkan-tools: lavapipe, a software Vulkan ICD so the
#    wgpu backend and its compute tests run without a physical GPU (CI and headless dev).
#  - libgl1-mesa-dri + libegl-mesa0 + libglx-mesa0: software OpenGL (llvmpipe). Lavapipe can render
#    offscreen but not *present* to a window under Xvfb, so the on-screen viewer uses the GL backend
#    (`WGPU_BACKEND=gl`) — there is no GPU to pass through to a Linux container on macOS.
#  - winit/egui build deps (X11 + Wayland + xkbcommon, incl. the -x11 runtime): to build and run the viewer.
#  - pkg-config/cmake/libssl-dev: common transitive build needs.
#  - python3 + pipx: the SDK toolchain (maturin); torch is installed on demand (M7).
RUN apt-get update && apt-get install -y --no-install-recommends \
        mesa-vulkan-drivers vulkan-tools libvulkan1 \
        libgl1-mesa-dri libegl-mesa0 libglx-mesa0 \
        libx11-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxcb1-dev \
        libgl1-mesa-dev \
        pkg-config cmake libssl-dev \
        python3 python3-pip python3-venv pipx \
    && rm -rf /var/lib/apt/lists/*

# Rust components used by CI (keep local == CI).
RUN rustup component add rustfmt clippy

# maturin for building the PyO3 extension (isolated via pipx).
RUN pipx install maturin && pipx ensurepath

# Force the software Vulkan ICD so wgpu selects lavapipe in this container. The lavapipe ICD file is
# named per-arch (lvp_icd.x86_64.json on amd64, lvp_icd.aarch64.json on arm64 — the latter is what
# Docker Desktop runs on Apple Silicon), so symlink whichever exists to a stable, arch-agnostic path.
RUN ln -sf "$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1)" \
        /usr/share/vulkan/icd.d/lvp_icd.active.json
ENV VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.active.json
ENV WGPU_BACKEND=vulkan
