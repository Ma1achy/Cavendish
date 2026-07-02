# Cavendish dev container.
# Rust + wgpu (software Vulkan for headless) + Python/maturin for the SDK.
# The spec's LaTeX build lives in CI, not here, to keep the image lean.
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive

# System deps:
#  - mesa-vulkan-drivers + vulkan-tools: lavapipe, a software Vulkan ICD so the
#    wgpu backend and its tests run without a physical GPU (CI and headless dev).
#  - winit/egui build deps (X11 + Wayland + xkbcommon): needed to *build* the viewer.
#  - pkg-config/cmake/libssl-dev: common transitive build needs.
#  - python3 + pipx: the SDK toolchain (maturin); torch is installed on demand (M7).
RUN apt-get update && apt-get install -y --no-install-recommends \
        mesa-vulkan-drivers vulkan-tools libvulkan1 \
        libx11-dev libxkbcommon-dev libwayland-dev libxcb1-dev \
        libgl1-mesa-dev \
        pkg-config cmake libssl-dev \
        python3 python3-pip python3-venv pipx \
    && rm -rf /var/lib/apt/lists/*

# Rust components used by CI (keep local == CI).
RUN rustup component add rustfmt clippy

# maturin for building the PyO3 extension (isolated via pipx).
RUN pipx install maturin && pipx ensurepath

# Force the software Vulkan ICD so wgpu selects lavapipe in this container.
ENV VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json
ENV WGPU_BACKEND=vulkan
