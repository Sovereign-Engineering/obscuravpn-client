#!/usr/bin/env bash
set -eux

TARGET_ARCH="${TARGET_ARCH:-$(uname -m)}"
./contrib/bin/linux-build-binaries.bash --target_arch "$TARGET_ARCH"
exec ./result-linux/target-"$TARGET_ARCH"/cli/debug/obscura "$@"
