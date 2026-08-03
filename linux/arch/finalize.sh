#!/usr/bin/env bash
set -eux

for f in /out/*/obs-keys.asc; do sudo cp -f --no-preserve=ownership "$f" /out/obs-keys.asc; done
for f in /out/*/obs-fingerprint.txt; do sudo cp -f --no-preserve=ownership "$f" /out/obs-fingerprint.txt; done
