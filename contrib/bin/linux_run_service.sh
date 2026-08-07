#!/usr/bin/env bash
set -eux

source contrib/shell/source-die.bash

if ! getent group obscura > /dev/null; then
  die "group 'obscura' does not exist, create it with: sudo groupadd --system obscura"
fi

TARGET_ARCH="${TARGET_ARCH:-$(uname -m)}"
./contrib/bin/linux-build-binaries.bash --target_arch "$TARGET_ARCH"
exec sudo --preserve-env=RUST_LOG sg obscura "umask 0007 && ./result-linux/target-$TARGET_ARCH/cli/debug/obscura service --config-dir /var/lib/obscura --log-dir /var/log/obscura"
