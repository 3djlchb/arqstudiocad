#!/bin/bash
# Build, install (per user) and self-check the Flatpak of ArqStudioCAD.
#
#   1. Regenerate cargo-sources.json from Cargo.lock (offline crate vendoring)
#   2. flatpak-builder: build from source with the KDE SDK + PySide BaseApp
#   3. Install for the current user and run `--check` inside the sandbox
#   4. Export a single-file bundle: dist/flatpak/<id>.flatpak
#
# Needs: flatpak, flatpak-builder, uv, curl, and the Flathub remote:
#   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/packaging/flatpak"
APP_ID="io.github._3djlchb.ArqStudioCAD"
OUT="$ROOT/dist/flatpak"
WORK="${XDG_CACHE_HOME:-$HOME/.cache}/arqstudiocad-flatpak"
GENERATOR_URL="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py"

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
mkdir -p "$OUT" "$WORK"

step "1/4 Vendored Rust crates (cargo-sources.json)"
[ -f "$WORK/flatpak-cargo-generator.py" ] || curl -fsSL -o "$WORK/flatpak-cargo-generator.py" "$GENERATOR_URL"
uv run --quiet --no-project --with aiohttp --with tomlkit \
    python "$WORK/flatpak-cargo-generator.py" "$ROOT/Cargo.lock" -o "$HERE/cargo-sources.json"

step "Rust in the SDK extension vs. rust-version in Cargo.toml"
NEED="$(sed -n 's/^rust-version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml")"
HAVE="$(flatpak run --user --command=/usr/lib/sdk/rust-stable/bin/rustc org.kde.Sdk//6.11 --version \
    | awk '{print $2}')"
echo "required $NEED, SDK extension has $HAVE"
if [ "$(printf '%s\n%s\n' "$NEED" "$HAVE" | sort -V | head -1)" != "$NEED" ]; then
    echo "Blocked: Flathub's rust-stable is older than rust-version; waiting for an update." >&2
    exit 2
fi

step "2/4 flatpak-builder"
flatpak-builder --user --force-clean --install-deps-from=flathub --install \
    --state-dir="$WORK/state" --repo="$WORK/repo" \
    "$WORK/build" "$HERE/$APP_ID.yml"

step "3/4 Self-check inside the sandbox"
flatpak run --user --env=QT_QPA_PLATFORM=offscreen "$APP_ID" --check

step "4/4 Single-file bundle"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$WORK/repo" "$OUT/$APP_ID.flatpak" "$APP_ID"
ls -lh "$OUT"
