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

`-SkipInstaller` stops after the bundle. An MSI (WiX) for corporate
deployment is planned (ADR 0009).

Silent install, for testing:

```powershell
dist\ArqStudioCAD-0.1.0-windows-x64-setup.exe /VERYSILENT /CURRENTUSER /DIR="C:\Temp\arq"
```

## Linux, macOS

Coming in the next steps of phase 1: AppImage, `.deb`, `.rpm`, Flatpak and
AUR on Linux (built in a `manylinux_2_34` container), universal2 `.dmg` on
macOS.
