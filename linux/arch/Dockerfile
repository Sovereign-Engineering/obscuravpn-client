FROM archlinux:base-devel-20260621.0.546891@sha256:cf028aee853281ba9b3cba41b49f7ad4836275256226938ab619e0f05b1e6105
RUN pacman -Syu --noconfirm --needed namcap gtk4 webkitgtk-6.0 libsoup3 glib2 hicolor-icon-theme && \
    useradd -m builder && \
    printf 'builder ALL=(ALL) NOPASSWD: ALL\n' >>/etc/sudoers
USER builder
