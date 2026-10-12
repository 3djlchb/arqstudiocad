#!/bin/bash
# Runs INSIDE the manylinux container chosen by build.sh (manylinux_2_34 on
# x86_64, manylinux_2_39 on aarch64). The bundle links against that glibc, so
# it starts on every Linux with the same or a newer one (ADR 0009).
#
#   /src   repository (read-only)
#   /out   output directory (bundle + metadata)
#   /cache rustup/cargo/pip caches shared between runs
set -euo pipefail

# The bundled Python version (ADR 0009). The image's own /opt/python builds
# are static (no libpython.so), which PyInstaller cannot bundle, so Python
# comes from uv's standalone builds instead.
PYTHON="${PYTHON:-3.13}"

export RUSTUP_HOME=/cache/rustup CARGO_HOME=/cache/cargo CARGO_TARGET_DIR=/cache/target
export UV_CACHE_DIR=/cache/uv UV_PYTHON_INSTALL_DIR=/cache/python UV_INSTALL_DIR=/cache/uv-bin
export PATH="/cache/cargo/bin:/cache/uv-bin:$PATH"

echo "==> System libraries Qt needs at run time (for the bundle self-check)"
dnf install -y -q epel-release >/dev/null
dnf install -y -q mesa-libGL mesa-libEGL libxkbcommon libxkbcommon-x11 fontconfig \
    dbus-libs xcb-util-cursor xcb-util-wm xcb-util-image xcb-util-keysyms \
    xcb-util-renderutil libX11-xcb >/dev/null

echo "==> Rust toolchain (pinned by rust-toolchain.toml)"
if ! command -v rustup >/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain none --no-modify-path >/dev/null
fi

echo "==> uv and Python $PYTHON (shared libpython)"
if ! command -v uv >/dev/null; then
    curl -LsSf https://astral.sh/uv/install.sh | env UV_NO_MODIFY_PATH=1 sh >/dev/null
fi

echo "==> Release wheel"
rm -rf /tmp/build && mkdir -p /tmp/build/wheels
uv venv -q --python "$PYTHON" --managed-python /tmp/build/venv
VENV_PY=/tmp/build/venv/bin/python
uv pip install -q --python "$VENV_PY" "maturin>=1.15,<2.0" "pyinstaller>=6.22,<7"
(cd /src && "$VENV_PY" -m maturin build --release --locked \
    --compatibility "${MANYLINUX:-manylinux_2_34}" --interpreter "$VENV_PY" --out /tmp/build/wheels)
uv pip install -q --python "$VENV_PY" /tmp/build/wheels/*.whl

echo "==> PyInstaller bundle"
rm -rf /out/bundle && mkdir -p /out/bundle
"$VENV_PY" -m PyInstaller /src/packaging/arqstudiocad.spec --noconfirm \
    --distpath /out/bundle --workpath /tmp/build/pyinstaller

"$VENV_PY" - > /out/metadata.env <<'EOF'
from arqstudiocad import __version__
from arqstudiocad.product import APP_ID, APP_NAME
print(f"APP_NAME={APP_NAME}\nAPP_ID={APP_ID}\nVERSION={__version__}")
EOF
source /out/metadata.env

echo "==> Bundle self-check"
QT_QPA_PLATFORM=offscreen "/out/bundle/${APP_NAME}/arqstudiocad-cli" --check
