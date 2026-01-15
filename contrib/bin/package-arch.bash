#!/usr/bin/env bash
set -eu

nix build .#rust-static

docker build -f linux/arch_Dockerfile -t obscura-arch .
docker run --rm --security-opt label=disable -v "$PWD:/wd" -v "$(realpath result/bin/obscura):/obscura" obscura-arch sh -c '
    set -eu
    mkdir -p ~/build/src && cd ~/build
    cp /wd/linux/arch_PKGBUILD PKGBUILD
    cp /obscura src/obscura
    cp /wd/linux/obscura.service src/
    cp /wd/linux/obscura-sysusers.conf src/
    cp /wd/LICENSE.md src/
    makepkg -f
    namcap -e elfnoshstk *.pkg.tar.zst
    sudo cp *.pkg.tar.zst /wd/
'
