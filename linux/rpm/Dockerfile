FROM almalinux:10@sha256:cc24bc5b6ac7e284f2f62a07bdaa1b15d3319fdcf46413c6b8fe9fa245068ddd
RUN dnf install -y epel-release \
 && dnf install -y --setopt=install_weak_deps=False \
      rpm-build rpm-sign rpmlint systemd-rpm-macros createrepo_c gnupg2 \
 && dnf clean all
