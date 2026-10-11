# 0003. Target platforms and macOS universal2

Date: 2026-10-11 · Status: Accepted · Packaging tools and platform floor in
[0009](0009-packaging.md)

## Context

The project targets every mainstream desktop from the start. Packaging a
Rust + Python + Qt application differs on each platform, and problems found
late (missing wheels, signing, sandboxing) are expensive.

## Decision

| Platform | Rust target | Deliverable |
|---|---|---|
| Windows x86-64 | `x86_64-pc-windows-msvc` | MSI/MSIX installer + portable zip |
| Windows ARM64 | `aarch64-pc-windows-msvc` | MSI/MSIX installer |
| macOS | `universal2-apple-darwin` (arm64 + x86_64) | One signed, notarized `.dmg` |
| Linux x86-64 (+ arm64 when feasible) | `*-unknown-linux-gnu` | `.deb`, `.rpm`, AUR `PKGBUILD`, Flatpak, AppImage |

- macOS ships a **single universal2 build** that installs on Apple Silicon and
  Intel. PySide6 and the python.org runtime are already universal2.
- All platforms can be built in GitHub Actions from phase 1 (walking
  skeleton), on native runners where available, triggered manually (ADR 0005).
- **No public release until the application is mature** enough to be used and
  distributed. Until then, packages exist only as CI artifacts for internal
  testing.
- Code signing and notarization are set up in phase 9.

## Consequences

- The macOS bundle is larger than a single-architecture one.
- Windows ARM64 availability of PySide6 wheels must be confirmed in phase 1.
- Flatpak builds from source with its own manifest; it cannot reuse the wheel.
