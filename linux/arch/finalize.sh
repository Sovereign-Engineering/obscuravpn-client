#!/usr/bin/env bash
set -eux

for f in /out/*/obscura-keyring-*.pkg.tar.zst; do sudo cp -f --no-preserve=ownership "$f" /out/obscura-keyring.pkg.tar.zst; done
