# Progress

The builder updates this after every slice. The owner reads it at checkpoints.

| Slice | State | Tag | Notes |
|---|---|---|---|
| S0 the shell | research under way; build blocked on the Rust toolchain (see below) | | |

## Questions for the owner
- **21 Nov 2026 — install Rust on the build PC?** Neither `cargo` nor `rustup` is present (no
  `~/.cargo`, no `~/.rustup`); the MSVC build tools are (VS Build Tools 2026, VS Community 2022).
  Installing rustup, the pinned stable toolchain and `cargo-nextest` writes outside the repo, so it
  waits for a yes. Until then only research notes and briefs can move; no slice can be built.

## The build machine, as found
- Windows 10 Pro, NVIDIA GeForce GTX 1660 SUPER (6 GB), Python 3.14, `gh` logged in.
- The §5 target (60 fps at 1440p, 10¹⁰ cells) was written without knowing the card; S4 measures
  first and says what this card holds.

## Decisions worth a second look
(none yet)
