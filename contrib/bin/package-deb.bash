#!/usr/bin/env bash
set -eu

nix build .#rust-static

docker build -f linux/deb_Dockerfile -t obscura-deb .
docker run --rm --security-opt label=disable -v "$PWD:/wd" -v "$(realpath result/bin/obscura):/obscura" obscura-deb sh -c '
    set -eu
    mkdir -p /build/obscura/debian
    cd /build/obscura
    cp /wd/linux/deb_control debian/control
    cp /wd/linux/deb_rules debian/rules
    cp /wd/linux/deb_install debian/obscura.install
    echo "obscura (0.0.1) unstable; urgency=low" > debian/changelog
    echo "" >> debian/changelog
    echo "  * Release" >> debian/changelog
    echo "" >> debian/changelog
    echo " -- obscura authors <support@obscura.net>  Thu, 01 Jan 1970 00:00:00 +0000" >> debian/changelog
    cp /wd/linux/obscura.service debian/obscura.service
    cp /wd/linux/obscura-sysusers.conf debian/obscura.sysusers
    install -m755 /obscura obscura
    chmod +x debian/rules
    dpkg-buildpackage -us -uc -b
    lintian --allow-root --suppress-tags no-copyright-file,no-manual-page,shared-library-lacks-prerequisites,description-starts-with-package-name /build/*.deb
    cp /build/*.deb /wd/
'
