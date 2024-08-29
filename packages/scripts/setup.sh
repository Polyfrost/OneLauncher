#!/usr/bin/env bash

set -euo pipefail

if [ "${CI:-}" = "true" ]; then
	set -x
fi

# displays an error message and exits the script
err() {
	for _line in "$@"; do
		echo "$_line" >&2
	done
	exit 1
}

# check if a specified command exists in path
has() {
	for prog in "$@"; do
		if ! command -v "$prog" 1>/dev/null 2>&1; then
			return 1
		fi
	done
}

# runs a specified command with sudo
sudo() {
	if [ "$(id -u)" -eq 0 ]; then
		"$@"
	else
		env sudo "$@"
	fi
}

# fails the script at a specific line with an unknown error
script_failure() {
	if [ -n "${1:-}" ]; then
		_line="on line $1"
	else
		_line="(unknown)"
	fi
	err "an error occurred at $_line." "setup failed!"
}

# traps any errors in script_failure and passes in the line number
trap 'script_failure ${LINENO:-}' ERR

# checks if we are running in a bash for windows environment
case "${OSTYPE:-}" in
	'msys' | 'mingw' | 'cygwin')
		err 'bash for windows is not supported, please use powershell or cmd and run setup.ps1'
		;;
esac

if [ "${CI:-}" != "true" ]; then
	echo 'onelauncher development environment setup:'
	echo 'to set up your machine for onelauncher development, this script will install some required dependencies with your system package manager'
	echo
	echo 'press enter to continue'
	read -r

	# checks if pnpm is installed
	if ! has pnpm; then
	err 'pnpm was not found.' \
		"ensure the 'pnpm' command is in your \$PATH." \
		'you must use pnpm for this project; yarn and npm will not work:' \
		'https://pnpm.io/installation'
	fi

	# checks if rustc and cargo are installed
	if ! has rustc cargo; then
	err 'rust was not found!' \
		"ensure the 'rustc' and 'cargo' binaries are in your \$PATH:" \
		'https://rustup.rs'
	fi

	echo
fi

# installs system-specific dependencies
case "$(uname)" in
	"Darwin")
		if [ "$(uname -m)" = 'x86_64' ] && ! [ "${CI:-}" = "true" ]; then
			brew install nasm
		fi

		echo
		;;
	"Linux")
		# https://v2.tauri.app/start/prerequisites/
		if has apt-get; then
			echo "detected apt!"
			echo "installing dependencies with apt..."

			# Tauri dependencies
			set -- build-essential curl wget file libssl-dev libgtk-3-dev librsvg2-dev \
				libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev libdbus-1-dev libvips42 \
				llvm-dev libclang-dev clang nasm perl pkg-config

			sudo apt-get -y update
			sudo apt-get -y install "$@"
		elif has pacman; then
			echo "detected pacman!"
			echo "installing dependencies with pacman..."

			# Tauri dependencies
			set -- appmenu-gtk-module libappindicator-gtk3 base-devel curl wget file openssl \
				gtk3 librsvg webkit2gtk-4.1 libayatana-appindicator dbus xdotool libvips clang nasm perl

			sudo pacman -Sy --needed "$@"
		elif has dnf; then
			echo "detected dnf!"
			echo "installing dependencies with dnf..."

			# For Enterprise Linux, you also need "Development Tools" instead of "C Development Tools and Libraries"
			if ! { sudo dnf group install "C Development Tools and Libraries" || sudo dnf group install "Development Tools"; }; then
				err 'We were unable to install the "C Development Tools and Libraries"/"Development Tools" package.' \
				'Please open an issue if you feel that this is incorrect.'
			fi

			# Tauri dependencies
			set -- openssl webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel \
				librsvg2-devel libxdo-devel dbus vips clang clang-devel nasm perl-core

			sudo dnf install "$@"
		elif has apk; then
			echo "detected apk!"
			echo "installing dependencies with apk..."
			echo "note: alpine suport is experimental" >&2

			# Tauri dependencies
			set -- build-base curl wget file openssl-dev gtk+3.0-dev librsvg-dev \
				webkit2gtk-4.1-dev libayatana-indicator-dev xdotool-dev dbus-dev vips \
				llvm16-dev clang16 nasm perl

			sudo apk add "$@"
		elif has emerge; then
			echo "detected emerge!"
			echo "installing dependencies with emerge..."
			echo "note: gentoo support is experiemntal" >&2

			# Tauri Dependencies
			set -- net-libs/webkit-gtk:4.1 dev-libs/libappindicator net-misc/curl net-misc/wget sys-apps/file

			sudo emerge --ask "$@"
		elif has zypper; then
			echo "detected zypper!"
			echo "installing dependencies with zypper..."
			echo "note: openSUSE support is experimental" >&2

			set -- libopenssl-devel webkit2gtk3-devel curl wget file libappindicator3-1 librsvg-devel

			sudo zypper in "$@"
			sudo zypper in -t pattern devel_basis
		elif has xbps-install; then
			echo "detected xbps-install!"
			echo "installing dependencies with xbps-install..."
			echo "note: void support is experimental" >&2

			set -- webkit2gtk-devel curl wget file openssl gtk+3-devel libappindicator \
				librsvg-devel gcc pkg-config

			sudo xbps-install -Sy --needed "$@"
		else
			if has lsb_release; then
				_distro="'$(lsb_release -s -d)' "
			fi
			err "your linux distro ${_distro:-}is not supported by this script!" \
				'We would welcome a pr or some help adding your os to this script:' \
				'https://github.com/polyfrost/onelauncher/issues'
		fi
		;;
	*)
		err "your os ($(uname)) is not supported by this script!" \
			'we would welcome a pr or some help adding your os to this script:' \
			'https://github.com/polyfrost/onelauncher/issues'
		;;
esac

# installs cargo-watch for development purposes
if [ "${CI:-}" != "true" ]; then
	echo "installing rust tools..."
	_tools="cargo-watch"
	echo "$_tools" | xargs cargo install
fi

echo 'your machine has been setup for onelauncher development!'
