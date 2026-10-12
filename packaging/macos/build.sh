#!/bin/bash
# Build the macOS package of ArqStudioCAD (ADR 0009): one universal2 .app
# (Apple Silicon + Intel) inside a .dmg.
#
#   1. Release wheel of the Rust extension for universal2-apple-darwin
#   2. Packaging environment on a universal2 Python (python.org installer)
#   3. PyInstaller .app with target_arch=universal2
#   4. Checks: both architectures present (lipo), --check natively and
#      under Rosetta (arch -x86_64)
#   5. .dmg with an Applications shortcut
#
# Needs: Xcode command line tools, rustup, and the python.org universal2
# Python 3.13 (PYTHON may point elsewhere). Not signed or notarized yet
# (phase 9): first launch needs right click > Open.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PYTHON="${PYTHON:-/Library/Frameworks/Python.framework/Versions/3.13/bin/python3.13}"
OUT="$ROOT/dist/macos"
BUILD="$ROOT/build/macos"
cd "$ROOT"

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
fat() {  # fail unless $1 contains both arm64 and x86_64
    local archs; archs="$(lipo -archs "$1")"
    [[ "$archs" == *arm64* && "$archs" == *x86_64* ]] || { echo "Not universal2: $1 ($archs)" >&2; exit 1; }
    echo "universal2: $(basename "$1") [$archs]"
}

rm -rf "$OUT" "$BUILD" && mkdir -p "$OUT" "$BUILD/wheels"
fat "$PYTHON"

step "1/5 Release wheel (universal2)"
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
"$PYTHON" -m venv "$BUILD/venv"
PY="$BUILD/venv/bin/python"
"$PY" -m pip install -q --upgrade pip "maturin>=1.15,<2.0" "pyinstaller>=6.22,<7"
"$PY" -m maturin build --release --locked --target universal2-apple-darwin \
    --interpreter "$PY" --out "$BUILD/wheels"

step "2/5 Packaging environment"
"$PY" -m pip install -q "$BUILD"/wheels/*.whl
eval "$("$PY" - <<'EOF'
from arqstudiocad import __version__
from arqstudiocad.product import APP_NAME
print(f"APP_NAME={APP_NAME}; VERSION={__version__}")
EOF
)"

step "3/5 PyInstaller .app ($APP_NAME $VERSION, universal2)"
ARQ_TARGET_ARCH=universal2 "$PY" -m PyInstaller packaging/arqstudiocad.spec --noconfirm \
    --distpath "$BUILD/dist" --workpath "$BUILD/pyinstaller"
APP="$BUILD/dist/$APP_NAME.app"
MACOS="$APP/Contents/MacOS"

step "4/5 Checks"
fat "$MACOS/$APP_NAME"
fat "$(find "$APP" -name '_core*.so' | head -1)"
QT_QPA_PLATFORM=offscreen "$MACOS/arqstudiocad-cli" --check
echo "-- Intel slice, under Rosetta:"
QT_QPA_PLATFORM=offscreen arch -x86_64 "$MACOS/arqstudiocad-cli" --check

step "5/5 Disk image"
STAGE="$BUILD/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO \
    "$OUT/$APP_NAME-$VERSION-macos-universal2.dmg"
ls -lh "$OUT"
