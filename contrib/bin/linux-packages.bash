#!/usr/bin/env bash
set -eu

./contrib/bin/package-deb.bash
./contrib/bin/package-rpm.bash
./contrib/bin/package-arch.bash
