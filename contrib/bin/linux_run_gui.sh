#!/usr/bin/env bash
set -eux

TARGET_ARCH="${TARGET_ARCH:-$(uname -m)}"
./contrib/bin/linux-build-binaries.bash --target_arch "$TARGET_ARCH"
WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 exec ./result-linux/target-"$TARGET_ARCH"/gui/debug/obscura-gui "$@"
