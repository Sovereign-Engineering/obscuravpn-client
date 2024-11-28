#!/usr/bin/env bash

set -euo pipefail

cd "$SRCROOT/.."

apple/xcodescripts/set-build-info.bash

mkdir -pv obscura-ui/build
