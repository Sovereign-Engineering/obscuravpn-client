#!/usr/bin/env bash
set -eo pipefail # No -u since we're sourcing external things

pushd "${SRCROOT}/../"

source contrib/shell/source-nix.sh

# TODO: remove magic 1420 port
PORT=1420

"$SRCROOT/xcodescripts/nix-web-dev-server-stop.bash" || true

WK_WEB_VIEW=1 nix develop ".#web" --print-build-logs -c just web-bundle-start &

while jobs %% && ! nc -z localhost $PORT; do
	sleep 0.05
done
disown %%
