#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

# Identify the single updater artifact for this platform (the file with a
# sibling .sig): -setup.exe (nsis), *.app.tar.gz (app), *.AppImage (appimage).
# NOTE: nullglob is on, so an unmatched glob expands to nothing. Use arrays
# rather than `ls <glob> | head` - the latter falls back to listing the cwd
# when nothing matches, silently picking an unrelated file.
fmt=""; ext=""; matches=()
case "$OS_KEY" in
  windows) matches=("$OUT_DIR"/*-setup.exe); fmt="nsis"; ext="exe" ;;
  darwin)  matches=("$OUT_DIR"/*.app.tar.gz); fmt="app"; ext="app.tar.gz" ;;
  linux)   matches=("$OUT_DIR"/*.AppImage); fmt="appimage"; ext="AppImage" ;;
esac
updater_file="${matches[0]:-}"
if [ -z "$updater_file" ]; then
  echo "::error::No updater artifact found in $OUT_DIR for $OS_KEY"
  exit 1
fi

# The updater bundle isn't always arch-tagged by cargo-packager, so the two
# macOS legs would collide on the release. Rename the updater artifact + its .sig
# to an arch-unique name before upload, and point the manifest at that name.
unique="OneClient_${VERSION}_${OS_KEY}_${ARCH}.${ext}"
cp "$updater_file" "$OUT_DIR/$unique"
cp "$updater_file.sig" "$OUT_DIR/$unique.sig"

# cargo-packager names the .deb after the binary (oneclient_app_<ver>_amd64.deb).
# Rename it to the same OneClient_<ver>_<os>_<arch> scheme as the other assets so
# the AUR oneclient-bin package can point at a stable, predictable URL.
deb_unique=""
deb_matches=("$OUT_DIR"/*.deb)
if [ -e "${deb_matches[0]:-}" ]; then
  deb_unique="OneClient_${VERSION}_${OS_KEY}_${ARCH}.deb"
  cp "${deb_matches[0]}" "$OUT_DIR/$deb_unique"
fi

# Same treatment for the .rpm (cargo-generate-rpm names it
# oneclient_app-<ver>-1.x86_64.rpm).
rpm_unique=""
rpm_matches=("$OUT_DIR"/*.rpm)
if [ -e "${rpm_matches[0]:-}" ]; then
  rpm_unique="OneClient_${VERSION}_${OS_KEY}_${ARCH}.rpm"
  cp "${rpm_matches[0]}" "$OUT_DIR/$rpm_unique"
fi

# Upload human-facing installers (dmg/renamed deb/renamed rpm/AppImage/signed exe) +
# the renamed updater bundle and its .sig. Skip the un-renamed updater bundle to avoid clashes.
for f in "$OUT_DIR"/*.dmg ${deb_unique:+"$OUT_DIR/$deb_unique"} ${rpm_unique:+"$OUT_DIR/$rpm_unique"} "$OUT_DIR"/*.AppImage "$OUT_DIR"/*-setup.exe "$OUT_DIR/$unique" "$OUT_DIR/$unique.sig"; do
  [ -e "$f" ] || continue
  gh release upload "$TAG" "$f" --clobber
done

base="$unique"
mkdir -p fragments
case "$OS_KEY" in
  darwin) KEYS="darwin-${ARCH} macos-${ARCH}" ;;
  *)      KEYS="${OS_KEY}-${ARCH}" ;;
esac
SIG="$(cat "$updater_file.sig")" \
URL="https://github.com/${GITHUB_REPOSITORY}/releases/download/${TAG}/${base}" \
FMT="$fmt" \
KEYS="$KEYS" \
FRAG_NAME="${OS_KEY}-${ARCH}" \
python3 - <<'PY'
import json, os
entry = {
  "signature": os.environ["SIG"],
  "url": os.environ["URL"],
  "format": os.environ["FMT"],
}
frag = {key: entry for key in os.environ["KEYS"].split()}
with open(f"fragments/{os.environ['FRAG_NAME']}.json", "w") as f:
    json.dump(frag, f)
PY
