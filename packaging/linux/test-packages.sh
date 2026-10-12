#!/bin/bash
# Install each Linux package in a clean container of the distributions it
# targets and run `arqstudiocad --check` there. Proves the glibc floor and the
# declared dependencies, not just that the files were built.
#
# Usage: bash packaging/linux/test-packages.sh     (after build.sh)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/dist/linux"
APPIMAGE="$(ls "$OUT"/*.AppImage)"
DEB="$(ls "$OUT"/*.deb)"
RPM="$(ls "$OUT"/*.rpm)"
CHECK="QT_QPA_PLATFORM=offscreen arqstudiocad --check"
failures=0

run() {  # run <label> <image> <script>
    printf '\n\033[1;36m==> %s (%s)\033[0m\n' "$1" "$2"
    if podman run --rm -v "$OUT:/pkg:ro" "$2" bash -c "$3"; then
        echo "PASS: $1 on $2"
    else
        echo "FAIL: $1 on $2"; failures=$((failures + 1))
    fi
}

APT='export DEBIAN_FRONTEND=noninteractive; apt-get update -qq >/dev/null'
for image in docker.io/library/debian:13 docker.io/library/ubuntu:22.04; do
    run ".deb" "$image" "$APT && apt-get install -y -qq /pkg/$(basename "$DEB") >/dev/null && $CHECK"
done

for image in registry.fedoraproject.org/fedora:latest docker.io/library/almalinux:9; do
    run ".rpm" "$image" "dnf install -y -q epel-release >/dev/null 2>&1; \
        dnf install -y -q /pkg/$(basename "$RPM") >/dev/null && $CHECK"
done

# AppImage: system Qt dependencies are expected on the host, as on any desktop.
run "AppImage" docker.io/library/ubuntu:22.04 "$APT && apt-get install -y -qq \
    libgl1 libegl1 libfontconfig1 libxkbcommon0 libxkbcommon-x11-0 libxcb-cursor0 \
    libdbus-1-3 >/dev/null && cd /tmp && /pkg/$(basename "$APPIMAGE") --appimage-extract >/dev/null \
    && QT_QPA_PLATFORM=offscreen squashfs-root/AppRun --check"

printf '\n%s package test(s) failed\n' "$failures"
exit "$failures"
