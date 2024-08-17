#!/usr/bin/env bash

set -eEumo pipefail

has() {
	command -v "$1" >/dev/null 2>&1
}

handle_exit() {
	_exit=$?
	set +e
	trap '' SIGINT
	trap - EXIT

	if [ "$_exit" -ne 0 ]; then
		git restore --staged .
		git restore .
	fi
	exit "$_exit"
}

cleanup() {
	set +e
	trap '' SIGINT
	trap - EXIT

	jobs -p | xargs kill -SIGTERM
	git restore --staged .
	git restore .
	kill -- -$$ 2>/dev/null
}

if ! has git pnpm; then
	echo "missing required dependencies: git, pnpm" >&2
	exit 1
fi

__dirname="$(CDPATH='' cd "$(dirname "$0")" && pwd -P)"
cd "$__dirname/../.."

if [ -n "$(git diff --name-only HEAD)" ] || [ -n "$(git ls-files --others --exclude-standard)" ]; then
	echo "uncommitted changes found; please commit or stash your changes first." >&2
	exit 1
fi

if ! {
	if [ -n "${CI:-}" ]; then
		ancestor="$(git merge-base HEAD "HEAD~$((${PR_FETCH_DEPTH:?Missing PR_FETCH_DEPTH} - 1))")"
	else
		ancestor="$(git merge-base HEAD origin/main)"
	fi
}; then
	echo "failed to find the common ancestor of the current branch and main" >&2
	exit 1
fi

trap 'handle_exit' EXIT
trap 'cleanup' SIGINT

pnpm run format:es &
wait

if [ "${1:-}" != "no-cargo" ]; then
	cargo clippy --fix --all --all-targets --all-features --allow-dirty --allow-staged
	cargo fmt --all
fi

git diff --diff-filtered=d --cached --name-only "${ancestor:?Ancestor is not set}" | xargs git add

git restore .
