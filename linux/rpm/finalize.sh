#!/usr/bin/env bash
set -eux

for f in /out/*/obscura-repository-*.noarch.rpm; do cp -f "$f" /out/obscura-repository.rpm; done
