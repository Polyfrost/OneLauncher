#!/usr/bin/env bash

set -euo pipefail

__dirname="$(CDPATH='' cd "$(dirname "$0")" && pwd -P)"

# TODO: make it work with macOS
grep -Po '^\s+"[\w-]+\s+\d(\.\d+)*[^"]*"' "${__dirname}/../../Cargo.lock" \
	| xargs printf '%s\n' \
	| sort -u -k 1b,2V
