# 0009. Packaging and platform floor

Date: 2026-10-11 · Status: Accepted · Amends [0003](0003-target-platforms.md)

## Context

ADR 0003 fixed the target platforms. Shipping ArqStudioCAD means bundling a
Python runtime, PySide6 (Qt) and our Rust extension into a native package per
platform. IngeCAD (same UI stack) ships with PyInstaller; see
`docs/references/ingecad.md`.

PySide6 6.12 wheels set hard floors that ADR 0003 did not know about:

| Wheel | Floor |
|---|---|
| `macosx_14_0_universal2` | macOS 14 Sonoma |
| `manylinux_2_34_x86_64` | glibc 2.34 |
| `manylinux_2_39_aarch64` | glibc 2.39 |
| `win_amd64`, `win_arm64` | Windows 10/11 (both architectures available) |

## Decision

### Tools

- **Freezing: PyInstaller, onedir** (not onefile: onefile unpacks ~250 MB to a
  temp directory on every launch). One spec file, `packaging/arqstudiocad.spec`.
- **Qt: `PySide6-Essentials`** only; `PySide6-Addons` is added if a specific
  module is ever needed.
- **Bundled Python: 3.13.** The source-build minimum stays 3.12 (ADR 0004).
- **Application ID: `io.github._3djlchb.ArqStudioCAD`** — Flatpak, AppStream,
  macOS bundle identifier and Windows AppUserModelID. The `_` is required
  because an ID segment may not start with a digit. Never changes once published.

### Deliverables

| Platform | Package | Tool |
|---|---|---|
| Windows x64, ARM64 | `.exe` installer | **Inno Setup** (now) |
| Windows x64, ARM64 | `.msi` | **WiX** (later, for corporate deployment) |
| macOS universal2 | `.dmg` with `.app` bundle | PyInstaller + `hdiutil`; signing/notarization in phase 9 |
| Linux x86-64 (+ arm64) | AppImage, `.deb`, `.rpm`, Flatpak, AUR `PKGBUILD` | PyInstaller output repackaged; Flatpak builds from its manifest |

### Platform floor

| Platform | Minimum |
|---|---|
| Windows | 10 or 11, x64 or ARM64 |
| macOS | **14 Sonoma**, Apple Silicon or Intel |
| Linux x86-64 | **glibc 2.34**: Ubuntu 22.04+, Debian 12+, LMDE 6+, Fedora 35+, RHEL/Alma/Rocky 9+, current Arch/openSUSE Tumbleweed |
| Linux arm64 | glibc 2.39 (Ubuntu 24.04+ class) |

**Linux packages are built inside a `manylinux_2_34` container**, not on the
host runner: a binary built against a newer glibc refuses to start on older
systems even when PySide6 itself would run there.

### Bundle contents and self-check

- Data travels at the same relative paths as in the repository (`i18n/`,
  `AUTHORS`, resources), so the app resolves them identically frozen or not.
- `arqstudiocad --check` verifies the bundle: native core loads and reports
  its version, every language pack parses, `AUTHORS` is present, Qt platform
  plugins load. Every package build runs it; a bundle that fails is not
  published.

## Consequences

- Macs older than macOS 14 (roughly pre-2018 hardware) are not supported.
- Debian 11 / Ubuntu 20.04 and older are not supported.
- One spec file serves all platforms; per-platform differences live in small
  wrapper scripts under `packaging/`.
