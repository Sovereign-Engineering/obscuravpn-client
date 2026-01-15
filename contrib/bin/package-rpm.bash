#!/usr/bin/env bash
set -eu

nix build .#rust-static

docker build -f linux/rpm_Dockerfile -t obscura-rpm .
docker run --rm --security-opt label=disable -v "$PWD:/wd" -v "$(realpath result/bin/obscura):/obscura" obscura-rpm sh -c '
    set -eu
    mkdir -p ~/rpmbuild/{SOURCES,SPECS,RPMS}
    cp /wd/linux/rpm_obscura.spec ~/rpmbuild/SPECS/obscura.spec
    cp /obscura ~/rpmbuild/SOURCES/
    cp /wd/linux/obscura.service ~/rpmbuild/SOURCES/
    cp /wd/linux/obscura-sysusers.conf ~/rpmbuild/SOURCES/
    cp /wd/linux/obscura-preset.conf ~/rpmbuild/SOURCES/
    rpmbuild -bb ~/rpmbuild/SPECS/obscura.spec
    rpmlint -r /wd/linux/rpm_rpmlintrc ~/rpmbuild/RPMS/*/*.rpm
    cp ~/rpmbuild/RPMS/*/*.rpm /wd/
'
