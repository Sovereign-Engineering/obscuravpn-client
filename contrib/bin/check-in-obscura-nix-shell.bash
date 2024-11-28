#!/usr/bin/env bash
set -eo pipefail

source contrib/shell/source-die.bash

if [ -z "$OBSCURA_MAGIC_IN_NIX_SHELL" ]; then
	die "ERROR: Not running in Obscura Nix Shell, see README.md for setup"
fi
