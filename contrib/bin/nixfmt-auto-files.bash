#!/usr/bin/env bash
set -eo pipefail

# NOTE: we can't use `nix fmt` because it doesn't have `--check` mode
./contrib/bin/find-nix-files.bash -z \
	| exec xargs --null -- \
		nixfmt --width=120 "$@" --
