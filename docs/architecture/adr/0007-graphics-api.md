# 0007. Graphics API: one renderer on wgpu, backend chosen at runtime

Date: 2026-10-11 · Status: Accepted (to be validated by the phase 7 spike)

## Context

ArqStudioCAD must draw on NVIDIA, AMD and Intel GPUs, on integrated graphics,
and on hardware up to ~15 years old, across Windows, Linux and macOS. No
single native API covers all of them: macOS has no native Vulkan and OpenGL is
frozen at 4.1 and deprecated there; old Intel iGPUs on Windows lack DX12 and
Vulkan; old GPUs on Linux may only have OpenGL through Mesa.

Writing separate renderers per API (e.g. one for Vulkan, one for OpenGL)
multiplies code and bugs without adding coverage. wgpu is a single Rust API
that runs natively on Vulkan, DirectX 12, Metal and OpenGL / OpenGL ES.

## Decision

### One renderer, several backends

`arq-render` is written once against **wgpu**, with **WGSL** shaders (naga
translates them to SPIR-V, HLSL, MSL and GLSL). The backend is chosen at
startup according to the hardware:

| OS | Probe order |
|---|---|
| Windows | DirectX 12 → Vulkan → OpenGL 3.3 → WARP (software) |
| Linux | Vulkan → OpenGL ES 3.0 (EGL) → llvmpipe (software) |
| macOS | Metal |

- Each candidate is probed in a **short-lived child process** (device, upload,
  pipeline creation, offscreen draw, readback) so a crashing or hanging driver
  cannot take the application down; a crash sentinel skips a backend that
  killed the previous run (pattern from Open CAD Studio, see
  `docs/references/opencadstudio.md`).
- Users can force a backend in Preferences, and `--safe-mode` forces software
  rendering.

### Minimum versions

| Component | Minimum |
|---|---|
| wgpu | 30.x, pinned; upgrades are deliberate commits |
| Vulkan | 1.0 + the extensions wgpu requires |
| DirectX 12 | Feature Level 11_0, Windows 10+ |
| OpenGL | 3.3 core (Windows) / OpenGL ES 3.0 (Linux, via EGL) |
| Metal | Metal 2 |

### Downlevel rule

The core drawing path (2D wireframe, selection highlight, grips, snaps) must
work within wgpu's **downlevel limits** — what an OpenGL ES 3.0 device offers:
no compute shaders, no storage buffers, conservative texture and buffer sizes.
Features beyond that (e.g. GPU picking with compute, shaded 3D) are optional
and detected at runtime; when absent, a compatible path is used.

### Hardware support tiers

| Tier | Hardware (approx.) | Backend |
|---|---|---|
| **Supported** | 2012+: NVIDIA Kepler and later, AMD GCN and later, Intel Ivy Bridge and later, every Mac our macOS minimum supports | Vulkan / DX12 / Metal, or OpenGL where the driver lacks them |
| **Supported (reference low end)** | NVIDIA GeForce GT 730 2 GB (Kepler) on LMDE 7 (Debian 13), nouveau | OpenGL ES 3.0 (its Vulkan is software only) |
| **Best effort** | 2010–2011: NVIDIA Fermi, AMD TeraScale 2/3, Intel Sandy Bridge on Linux | OpenGL 3.3 / ES 3.0 |
| **Software only** | Anything else that runs the OS, including Intel Sandy Bridge on Windows (driver stops at OpenGL 3.1) | WARP / llvmpipe |

The **reference low-end machine** — GeForce GT 730 2 GB, LMDE 7 — is a release
gate: the renderer is not accepted unless it works there. Best-effort failures
do not block a release.

**Reference machine details** (recorded 2026-10-12):

| Component | Value |
|---|---|
| GPU | NVIDIA GeForce GT 730, **GK208B (Kepler)**, PCI ID `10de:1287` |
| Driver | **nouveau** (Mesa); OpenGL and OpenGL ES enabled |
| Vulkan | **software only** (lavapipe): no hardware Vulkan with this driver |
| CPU | Intel Core i7-2600 @ 3.40 GHz (Sandy Bridge, 2011): AVX, no AVX2 |
| RAM | 16 GB |
| Board / firmware | BIOSTAR H61MGC, AMI UEFI (2011) |
| OS | LMDE 7 (Debian 13), Cinnamon 6.6.9, **X11**, kernel 6.12 |

Mesa and OpenGL versions (`glxinfo -B`) are still to be recorded. What this
machine implies — software Vulkan rejected, OpenGL ES as its GPU path, a
CPU rasterizer, the build baseline — is decided in
[ADR 0013, D5.8](0013-application-shell.md).

### Test matrix available today

| Machine | GPUs | Backends testable |
|---|---|---|
| Windows 11 laptop (i7-8750H) | NVIDIA GTX 1050 Ti, Intel UHD 630 | DX12, Vulkan, OpenGL, WARP on each GPU (`WGPU_BACKEND`, `WGPU_ADAPTER_NAME`) |
| LMDE 7 desktop (i7-2600, nouveau) | NVIDIA GT 730 2 GB (Kepler GK208B) | OpenGL / OpenGL ES 3.0 on nouveau, llvmpipe, lavapipe (software Vulkan) |

AMD and macOS are covered by manual CI runs and volunteer testers.

## Consequences

- One drawing code path to maintain; the backend is a runtime detail.
- The practical floor is set by the operating system more than by the GPU:
  Qt 6 requires Windows 10+, and the macOS minimum follows PySide6.
- **Open risk — Qt integration:** wgpu must draw into a Qt widget through its
  native window handle on Windows, macOS, X11 and Wayland. Phase 7 starts with
  a spike on all of them. Fallback: render offscreen with wgpu and let Qt
  display the image (slower, works with any backend).
- GPU drivers on the development laptop (NVIDIA 2022, Intel 2021) should be
  updated before phase 7.
