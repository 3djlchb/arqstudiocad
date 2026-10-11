#!/bin/bash
# Build the Linux packages of ArqStudioCAD (ADR 0009):
#   1. PyInstaller bundle inside a manylinux_2_34 container (glibc 2.34 floor)
#   2. .tar.gz, AppImage, .deb and .rpm from that bundle
#   3. Validation of the desktop entry and AppStream metadata
#
# Needs: podman, dpkg-deb, rpmbuild, desktop-file-validate, appstreamcli, curl.
# Usage: bash packaging/linux/build.sh            (output in dist/linux/)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARCH="$(uname -m)"                       # x86_64 or aarch64
IMAGE="quay.io/pypa/manylinux_2_34_${ARCH}"
OUT="$ROOT/dist/linux"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/arqstudiocad-build"
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
LINUX="$ROOT/packaging/linux"

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }

rm -rf "$OUT" && mkdir -p "$OUT" "$CACHE"

step "1/6 Bundle in $IMAGE"
# Rootless podman maps the container's root to the calling user, so every
# file written to /out belongs to the user running this script.
podman run --rm --user root \
    -v "$ROOT:/src:ro" -v "$OUT:/out" -v "$CACHE:/cache" \
    "$IMAGE" bash /src/packaging/linux/container-build.sh

# shellcheck source=/dev/null
source "$OUT/metadata.env"            # APP_NAME, APP_ID, VERSION
BUNDLE="$OUT/bundle/$APP_NAME"
BASE="${APP_NAME}-${VERSION}-linux-${ARCH}"
PKG="arqstudiocad"                    # package and command name
ICON="$ROOT/resources/icons/${APP_ID}.svg"
DESKTOP="$LINUX/${APP_ID}.desktop"
METAINFO="$LINUX/${APP_ID}.metainfo.xml"

step "2/6 Validate desktop entry and AppStream metadata"
desktop-file-validate "$DESKTOP"
appstreamcli validate --no-net --explain "$METAINFO"

# Lay out a standard file system tree under $1: the bundle in /opt, a command
# in /usr/bin, and the desktop integration files in /usr/share.
install_tree() {
    local dest="$1"
    mkdir -p "$dest/opt" "$dest/usr/bin" "$dest/usr/share/applications" \
        "$dest/usr/share/metainfo" "$dest/usr/share/icons/hicolor/scalable/apps"
    cp -a "$BUNDLE" "$dest/opt/$PKG"
    ln -s "../../opt/$PKG/$APP_NAME" "$dest/usr/bin/$PKG"
    install -m644 "$DESKTOP" "$dest/usr/share/applications/"
    install -m644 "$METAINFO" "$dest/usr/share/metainfo/"
    install -m644 "$ICON" "$dest/usr/share/icons/hicolor/scalable/apps/"
}

step "3/6 Tarball"
tar -C "$OUT/bundle" -czf "$OUT/$BASE.tar.gz" "$APP_NAME"

step "4/6 AppImage"
APPDIR="$OUT/AppDir"
mkdir -p "$APPDIR/usr/lib"
cp -a "$BUNDLE" "$APPDIR/usr/lib/$PKG"
install -m644 "$DESKTOP" "$APPDIR/"
install -m644 "$ICON" "$APPDIR/"
install -Dm644 "$METAINFO" "$APPDIR/usr/share/metainfo/${APP_ID}.appdata.xml"
cat > "$APPDIR/AppRun" <<EOF
#!/bin/sh
HERE="\$(dirname "\$(readlink -f "\$0")")"
exec "\$HERE/usr/lib/$PKG/$APP_NAME" "\$@"
EOF
chmod +x "$APPDIR/AppRun"
TOOL="$CACHE/appimagetool-${ARCH}.AppImage"
[ -x "$TOOL" ] || { curl -fsSL -o "$TOOL" "$APPIMAGETOOL_URL"; chmod +x "$TOOL"; }
ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT/$BASE.AppImage" >/dev/null

step "5/6 .deb"
DEB_ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
DEBROOT="$OUT/debroot"
install_tree "$DEBROOT"
mkdir -p "$DEBROOT/DEBIAN"
cat > "$DEBROOT/DEBIAN/control" <<EOF
Package: $PKG
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: $APP_NAME contributors <https://github.com/3djlchb/arqstudiocad>
Homepage: https://github.com/3djlchb/arqstudiocad
Section: graphics
Priority: optional
Depends: libc6 (>= 2.34), libgl1, libegl1, libfontconfig1, libxkbcommon0, libxkbcommon-x11-0, libxcb-cursor0, libdbus-1-3
Description: Open-source 2D/3D CAD for architecture and engineering
 $APP_NAME is an open-source CAD application with DWG as its native format
 and DXF as an open alternative. Pre-alpha: it cannot draw yet.
EOF
dpkg-deb --root-owner-group --build "$DEBROOT" "$OUT/${PKG}_${VERSION}_${DEB_ARCH}.deb" >/dev/null

step "6/6 .rpm"
RPMTOP="$OUT/rpmbuild"
mkdir -p "$RPMTOP"/{BUILD,RPMS,SPECS,SOURCES,BUILDROOT}
sed -e "s/@VERSION@/$VERSION/" -e "s/@PKG@/$PKG/" -e "s/@APP_NAME@/$APP_NAME/" \
    -e "s/@APP_ID@/$APP_ID/" "$LINUX/arqstudiocad.spec.in" > "$RPMTOP/SPECS/$PKG.spec"
STAGE="$OUT/rpmstage"
install_tree "$STAGE"
rpmbuild -bb --quiet --dbpath "$RPMTOP/db" --define "_topdir $RPMTOP" --define "stage $STAGE" "$RPMTOP/SPECS/$PKG.spec"
find "$RPMTOP/RPMS" -name '*.rpm' -exec mv {} "$OUT/" \;

rm -rf "$APPDIR" "$DEBROOT" "$STAGE" "$RPMTOP"
step "Packages"
ls -lh "$OUT" | grep -E '\.(tar\.gz|AppImage|deb|rpm)$'
