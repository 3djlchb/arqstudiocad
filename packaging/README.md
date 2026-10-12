# Packaging

How ArqStudioCAD is turned into installable packages. Decisions are in
[ADR 0009](../docs/architecture/adr/0009-packaging.md).

Every build follows the same pipeline, with a thin script per platform:

1. **Release wheel** of the Rust extension (`maturin build --release`).
2. **Clean packaging environment** with the bundled Python (3.13) — never the
   development `.venv`, which holds a debug build of the extension.
3. **PyInstaller onedir** bundle from [`arqstudiocad.spec`](arqstudiocad.spec):
   one folder, two executables — `ArqStudioCAD` (the application) and
   `arqstudiocad-cli` (console variant for `--check` and diagnostics).
4. **Self-check**: `arqstudiocad-cli --check` must pass, or the build stops.
5. **Native package** for the platform.

Output goes to `dist/`. PyInstaller cannot cross-compile: each architecture is
built on a machine (or CI runner) of that architecture.

## Windows (x64, ARM64)

Requirements: the [build requirements](../README.md) plus Inno Setup 6:

```powershell
winget install --source winget --id JRSoftware.InnoSetup
```

Build:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build.ps1
```

Produces `dist\ArqStudioCAD\` (the bundle) and
`dist\ArqStudioCAD-<version>-windows-<arch>-setup.exe`. The installer:

- installs for the current user without administrator rights, or for all
  users if chosen in its first dialog;
- is in English and Spanish;
- adds a Start menu entry and, optionally, a desktop icon;
- uses the application ID as its `AppId`, so upgrades replace the previous
  version.

It also produces `dist\ArqStudioCAD-<version>-windows-<arch>-portable.zip`:
unzip anywhere (a USB stick included) and run `ArqStudioCAD.exe`. The
`portable.txt` file inside switches the app to portable mode — settings go to
`settings.ini` in that folder instead of the registry; delete it to use the
registry. The build unpacks the ZIP and self-checks it in portable mode.

`-SkipInstaller` stops after the portable ZIP. An MSI (WiX) for corporate
deployment is planned (ADR 0009).

Silent install, for testing:

```powershell
dist\ArqStudioCAD-0.1.0-windows-x64-setup.exe /VERYSILENT /CURRENTUSER /DIR="C:\Temp\arq"
```

## Linux (x86-64, arm64)

The bundle is built **inside a `manylinux_2_34` container** (Podman), so it
links against glibc 2.34 and starts on every distribution with glibc ≥ 2.34
(ADR 0009). The packages are then assembled on the host from that one bundle.

Requirements on the host (Debian/Ubuntu names):

```sh
sudo apt install podman dpkg-dev rpm desktop-file-utils appstream curl
```

Build, then test each package in clean containers of the distributions it
targets:

```sh
bash packaging/linux/build.sh            # -> dist/linux/
bash packaging/linux/test-packages.sh
```

| Package | Installs to | Tested on |
|---|---|---|
| `ArqStudioCAD-<v>-linux-<arch>.tar.gz` | anywhere (run `ArqStudioCAD/ArqStudioCAD`) | — |
| `ArqStudioCAD-<v>-linux-<arch>.AppImage` | nothing, runs in place | Ubuntu 22.04 |
| `arqstudiocad_<v>_<arch>.deb` | `/opt/arqstudiocad`, command `arqstudiocad` | Debian 13, Ubuntu 22.04 |
| `arqstudiocad-<v>-1.<arch>.rpm` | `/opt/arqstudiocad`, command `arqstudiocad` | Fedora, AlmaLinux 9 |

All of them carry the desktop entry, the AppStream metadata and the icon
under the application ID (`io.github._3djlchb.ArqStudioCAD`), validated during
the build. Flatpak and AUR have their own recipes (built from source).

The icon in `resources/icons/` is **provisional** until the project has a logo.

## Flatpak

Flathub builds from source, offline, so the Flatpak does not reuse the
PyInstaller bundle. The manifest
[`flatpak/io.github._3djlchb.ArqStudioCAD.yml`](flatpak/io.github._3djlchb.ArqStudioCAD.yml)
builds on the KDE 6.11 runtime with PySide6 from the official
`io.qt.PySide.BaseApp` (so the Flatpak uses the PySide version that base
provides, currently 6.11), compiles the Rust extension with the SDK's
`rust-stable` extension, and vendors crates through `cargo-sources.json`
(regenerated from `Cargo.lock` by the build script).

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
bash packaging/flatpak/build.sh     # build, install for the user, --check, dist/flatpak/*.flatpak
```

> **Status: blocked.** Flathub's `rust-stable` extension ships Rust 1.98.1,
> older than our `rust-version` (1.99). The script stops with a clear message
> until Flathub updates it; nothing else needs to change then.

## macOS (universal2: Apple Silicon + Intel)

One `.app` carries both architectures. Every piece must be universal2: the
python.org Python 3.13 installer (uv's Pythons are single-architecture), the
PySide6 wheels (already universal2), and our extension, built with
`maturin --target universal2-apple-darwin`.

Requirements: Xcode command line tools, rustup, and Python 3.13 from
python.org (or set `PYTHON` to another universal2 interpreter).

```sh
bash packaging/macos/build.sh     # -> dist/macos/ArqStudioCAD-<v>-macos-universal2.dmg
```

The build checks with `lipo` that the executable and the extension contain
both architectures, then runs `--check` natively and again with
`arch -x86_64` (Rosetta), so the Intel half is proven on Apple Silicon too.
Minimum macOS: 14 Sonoma. Not signed or notarized until phase 9: open it the
first time with right click > Open.

## All platforms in CI

[`.github/workflows/packages.yml`](../.github/workflows/packages.yml) runs the
same scripts on GitHub's runners — the only way to build Windows ARM64, Linux
arm64 and macOS without that hardware — and uploads the packages as workflow
artifacts (kept 14 days). It runs only when started by hand
(*Actions → Packages → Run workflow*). It is not a release: nothing is published.

| Job | Runner | Output |
|---|---|---|
| Windows x64 / arm64 | `windows-2025`, `windows-11-arm` | installer + portable ZIP |
| Linux x86_64 / aarch64 | `ubuntu-24.04`, `ubuntu-24.04-arm` | AppImage, `.deb`, `.rpm`, tarball (tested in clean containers) |
| macOS universal2 | `macos-15` | `.dmg` |
