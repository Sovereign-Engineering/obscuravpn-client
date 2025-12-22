#!/usr/bin/env bash
set -eu

# This is a temporary packaging script to simplify automated distro testing until proper packaging is implemented.

(cd rustlib && cargo build --release --target x86_64-unknown-linux-musl)
~/go/bin/nfpm package -f linux/nfpm.yaml -p deb
~/go/bin/nfpm package -f linux/nfpm.yaml -p rpm
~/go/bin/nfpm package -f linux/nfpm.yaml -p archlinux
