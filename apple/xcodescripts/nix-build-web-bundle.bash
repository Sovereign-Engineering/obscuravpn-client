#!/usr/bin/env bash
set -eo pipefail # No -u since we're sourcing external things

pushd "${SRCROOT}/../"

source contrib/shell/source-nix.sh

exec nix develop ".#web" --print-build-logs -c just web-bundle-build
