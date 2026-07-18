FROM debian:bookworm-slim@sha256:60eac759739651111db372c07be67863818726f754804b8707c90979bda511df
RUN apt-get update && apt-get install -y --no-install-recommends \
      debhelper dh-apparmor build-essential lintian dpkg-dev apt-utils gpg gpg-agent \
      libgtk-4-1 libwebkitgtk-6.0-4 libsoup-3.0-0 \
      libtss2-esys-3.0.2-0 libtss2-mu0 libtss2-sys1 libtss2-tctildr0 libtss2-tcti-device0 \
 && rm -rf /var/lib/apt/lists/*
