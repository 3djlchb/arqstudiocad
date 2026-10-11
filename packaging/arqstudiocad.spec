# PyInstaller spec for ArqStudioCAD (ADR 0009). -*- mode: python -*-
#
# Run it through the platform script (packaging/windows/build.ps1, ...), which
# prepares a clean environment holding the release build of the Rust extension
# and the bundled Python version. It produces one onedir bundle with two
# executables sharing the same files:
#
#   <APP_NAME>          the application (no console window on Windows)
#   arqstudiocad-cli    console variant, for `--check` and diagnostics
#
# onedir, not onefile: onefile unpacks the whole bundle to a temp directory
# on every launch.
import os
import sys
from pathlib import Path

from arqstudiocad.paths import package_dir
from arqstudiocad.product import APP_ID, APP_NAME

ROOT = Path(SPECPATH).parent  # noqa: F821 (SPECPATH is defined by PyInstaller)
PACKAGE = package_dir()  # the installed package in the packaging environment
TARGET_ARCH = os.environ.get("ARQ_TARGET_ARCH")  # e.g. universal2 on macOS

# Data keeps the same relative paths it has in the source tree, so
# arqstudiocad.paths resolves it identically frozen or not.
datas = [
    (str(PACKAGE / "locales"), "arqstudiocad/locales"),
    (str(ROOT / "AUTHORS"), "."),
    (str(ROOT / "LICENSE"), "."),
]

a = Analysis(  # noqa: F821
    [str(ROOT / "packaging" / "launcher.py")],
    datas=datas,
    excludes=["tkinter"],
)
pyz = PYZ(a.pure)  # noqa: F821

common = dict(exclude_binaries=True, upx=False, target_arch=TARGET_ARCH)
gui = EXE(pyz, a.scripts, [], name=APP_NAME, console=False, **common)  # noqa: F821
cli = EXE(pyz, a.scripts, [], name="arqstudiocad-cli", console=True, **common)  # noqa: F821

coll = COLLECT(gui, cli, a.binaries, a.datas, name=APP_NAME, upx=False)  # noqa: F821

if sys.platform == "darwin":
    app = BUNDLE(  # noqa: F821
        coll,
        name=f"{APP_NAME}.app",
        bundle_identifier=APP_ID,
        info_plist={"NSHighResolutionCapable": True, "LSMinimumSystemVersion": "14.0"},
    )
