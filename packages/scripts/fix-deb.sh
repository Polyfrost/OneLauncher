#!/usr/bin/env bash

set -eEuo pipefail

if [ "${CI:-}" = "true" ]; then
	set -x
fi

if [ "$(id -u)" -ne 0 ]; then
	echo "this script requires root privileges:" >&2
	exec sudo -E env _UID="$(id -u)" _GID="$(id -g)" "$0" "$@"
fi

echo "fixing .deb onelauncher bundle..." >&2

umask 0

err() {
	for _line in "$@"; do
		echo "$_line" >&2
	done
	exit 1
}

has() {
	for prog in "$@"; do
		if ! command -v "$prog" 1>/dev/null 2>&1; then
			return 1
		fi
	done
}

if ! has tar curl gzip strip; then
	err 'dependencies missing!' \
		"this script requires 'tar', 'curl', 'gzip' and 'strip' to be installed and available on \$PATH."
fi

CDPATH='' cd -- "$(dirname "$0")"
_root="$(pwd -P)"

if [ -n "${TARGET:-}" ]; then
	cd "../target/${TARGET}/release/bundle/deb" || err 'failed to find deb bundle'
else
	cd ../target/release/bundle/deb || err 'failed to find deb bundle'
fi

_deb="$(find . -type f -name '*.deb' | sort -t '_' -k '2,2' -V | tail -n 1)"
rm -rf "$(basename "$_deb" .deb)"
cp "$_deb" "$_deb.bak"

_tmp="$(mktemp -d)"
cleanup() {
	_err=$?
	rm -rf "$_tmp"
	if [ $_err -ne 0 ]; then
		mv "${_deb:?}.bak" "$_deb"
	fi

	chown "${_UID:-0}:${_GID:-0}" "$_deb" 2>/dev/null || true

	rm -f "${_deb:?}.bak"
	exit "$_err"
}
trap 'cleanup' EXIT

ar x "$_deb" --output="$_tmp"
mkdir -p "${_tmp}/data"
tar -xzf "${_tmp}/data.tar.gz" -C "${_tmp}/data"
mkdir -p "${_tmp}/control"
tar -xzf "${_tmp}/control.tar.gz" -C "${_tmp}/control"
chown -R root:root "$_tmp"

find "${_tmp}" -name 'onelauncher_gui' -o \( -type f -name 'onelauncher_gui.*' \) | while IFS= read -r file
do
	filename="$(basename "$file")"
	if [ "$filename" = "onelauncher_gui" ]; then
		mv "$file" "$(dirname "$file")/onelauncher"
	else
		mv "$file" "$(dirname "$file")/onelauncher.${filename#*.}"
	fi
done

mkdir -p "$_tmp"/data/usr/share/{doc/onelauncher,man/man1}

curl -LSs 'https://raw.githubusercontent.com/Polyfrost/OneLauncher/main/CHANGELOG.md' \
	| gzip -9 >"${_tmp}/data/usr/share/doc/onelauncher/changelog.gz"

cp "${_root}/../../LICENSE" "${_tmp}/data/usr/share/doc/onelauncher/copyright"

# TODO
# curl -LSs 'https://raw.githubusercontent.com/Polyfrost/OneLauncher/main/packages/distribution/onelauncher.1' \
# 	| gzip -9 >"${_tmp}/data/usr/share/man/man1/onelauncher.1.gz"

sed -i 's/^Categories=.*/Categories=Games;/' "${_tmp}/data/usr/share/applications/onelauncher.desktop"
sed -i 's/=onelauncher_gui/=onelauncher/' "${_tmp}/data/usr/share/applications/onelauncher.desktop"

find "${_tmp}/data" -type d -exec chmod 755 {} +
find "${_tmp}/data" -type f -exec chmod 644 {} +
chmod 755 "${_tmp}/data/usr/bin/onelauncher"

find "${_tmp}/data/usr/lib" -type f -name '*.so.*' -exec sh -euc \
	'for _lib in "$@"; do _link="$_lib" && while { _link="${_link%.*}" && [ "$_link" != "${_lib%.so*}" ]; }; do if [ -f "$_link" ]; then ln -sf "$(basename "$_lib")" "$_link"; fi; done; done' \
	sh {} +

find "${_tmp}/data/usr/bin" "${_tmp}/data/usr/lib" -type f -exec strip --strip-unneeded {} \;

if ! grep -q '^Section:' "${_tmp}/control/control"; then
	echo 'Section: contrib/games' >>"${_tmp}/control/control"
fi

(cd "${_tmp}/data" && find . -type f -exec md5sum {} + >"${_tmp}/control/md5sums")

find "${_tmp}/control" -type f -exec chmod 644 {} +
tar -czf "${_tmp}/data.tar.gz" -C "${_tmp}/data" .
tar -czf "${_tmp}/control.tar.gz" -C "${_tmp}/control" .
ar rcs "$_deb" "${_tmp}/debian-binary" "${_tmp}/control.tar.gz" "${_tmp}/data.tar.gz"
