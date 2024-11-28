#!/usr/bin/env bash
set -eo pipefail # No -u since we're sourcing external things

pushd "${SRCROOT}/../"

source contrib/shell/source-nix.sh

cmdline=(cbindgen "$@")
if [ -z "$OBSCURA_MAGIC_NO_NIX" ]; then
	cmdline=(nix shell .#rust-cbindgen --print-build-logs --command "${cmdline[@]}")
fi

exec "${cmdline[@]}"
