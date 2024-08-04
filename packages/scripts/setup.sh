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

# checks if a command/program exists
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
  err "An error occurred $_line." "Setup failed."
}

trap 'script_failure ${LINENO:-}' ERR

# checks if we are running in a windows environment
case "${OSTYPE:-}" in
  'msys' | 'mingw' | 'cygwin')
    err 'Bash for Windows is not supported, please use Powershell or CMD and run setup.ps1'
    ;;
esac

if [ "${CI:-}" != "true" ]; then
  echo 'OneLauncher Development Environment Setup'
  echo 'To set up your machine for OneLauncher development, this script will install some required dependencies with your system package manager'
  echo
  echo 'Press Enter to continue'
  read -r

  # checks if pnpm is installed
  if ! has pnpm; then
    err 'pnpm was not found.' \
      "Ensure the 'pnpm' command is in your \$PATH." \
      'You must use pnpm for this project; yarn and npm are not allowed.' \
      'https://pnpm.io/installation'
  fi

  # checks if rustc and cargo are installed
  if ! has rustc cargo; then
    err 'Rust was not found.' \
      "Ensure the 'rustc' and 'cargo' binaries are in your \$PATH." \
      'https://rustup.rs'
  fi

  echo
fi

# installs system deps
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
      echo "Detected apt!"
      echo "Installing dependencies with apt..."

      # Tauri dependencies
      set -- build-essential curl wget file libssl-dev libgtk-3-dev librsvg2-dev \
        libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev libdbus-1-dev libvips42 \
		llvm-dev libclang-dev clang nasm perl

      sudo apt-get -y update
      sudo apt-get -y install "$@"
    elif has pacman; then
      echo "Detected pacman!"
      echo "Installing dependencies with pacman..."

      # Tauri dependencies
      set -- appmenu-gtk-module libappindicator-gtk3 base-devel curl wget file openssl gtk3 librsvg webkit2gtk-4.1 libayatana-appindicator dbus xdotool libvips clang nasm perl

      sudo pacman -Sy --needed "$@"
    elif has dnf; then
      echo "Detected dnf!"
      echo "Installing dependencies with dnf..."

      # For Enterprise Linux, you also need "Development Tools" instead of "C Development Tools and Libraries"
      if ! { sudo dnf group install "C Development Tools and Libraries" || sudo dnf group install "Development Tools"; }; then
        err 'We were unable to install the "C Development Tools and Libraries"/"Development Tools" package.' \
          'Please open an issue if you feel that this is incorrect.'
      fi

      # Tauri dependencies
      set -- openssl webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel dbus vips clang clang-devel nasm perl-core

      sudo dnf install "$@"
    elif has apk; then
      echo "Detected apk!"
      echo "Installing dependencies with apk..."
      echo "Alpine suport is experimental" >&2

      # Tauri dependencies
      set -- build-base curl wget file openssl-dev gtk+3.0-dev librsvg-dev \
        webkit2gtk-4.1-dev libayatana-indicator-dev xdotool-dev dbus-dev vips \
		llvm16-dev clang16 nasm perl

      sudo apk add "$@"
    elif has emerge; then
      echo "Detected emerge!"
      echo "Installing dependencies with emerge..."
      echo "Gentoo support is experiemntal" >&2

      # Tauri Dependencies
      set -- net-libs/webkit-gtk:4.1 dev-libs/libappindicator net-misc/curl net-misc/wget sys-apps/file

      sudo emerge --ask "$@"
    elif has zypper; then
      echo "Detected zypper!"
      echo "Installing dependencies with zypper..."
      echo "openSUSE support is experimental" >&2

      set -- libopenssl-devel webkit2gtk3-devel curl wget file libappindicator3-1 librsvg-devel

      sudo zypper in "$@"
      sudo zypper in -t pattern devel_basis
    else
      if has lsb_release; then
        _distro="'$(lsb_release -s -d)' "
      fi
      err "Your Linux distro ${_distro:-}is not supported by this script." \
        'We would welcome a PR or some help adding your OS to this script:' \
        'https://github.com/polyfrost/onelauncher/issues'
    fi
    ;;
  *)
    err "Your OS ($(uname)) is not supported by this script." \
      'We would welcome a PR or some help adding your OS to this script.' \
      'https://github.com/polyfrost/onelauncher/issues'
    ;;
esac

# installs cargo-watch for development purposes
if [ "${CI:-}" != "true" ]; then
  echo "Installing Rust tools..."

  _tools="cargo-watch"

  echo "$_tools" | xargs cargo install
fi

echo 'Your machine has been setup for OneLauncher development!'
