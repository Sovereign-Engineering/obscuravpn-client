FROM debian:trixie-slim@sha256:020c0d20b9880058cbe785a9db107156c3c75c2ac944a6aa7ab59f2add76a7bd
RUN apt-get update && apt-get install -y --no-install-recommends \
      debhelper dh-apparmor build-essential lintian dpkg-dev apt-utils gpg gpg-agent \
      libgtk-4-1 libwebkitgtk-6.0-4 libsoup-3.0-0 \
      libtss2-esys-3.0.2-0t64 libtss2-mu-4.0.1-0t64 libtss2-sys1t64 libtss2-tctildr0t64 libtss2-tcti-device0t64 \
 && rm -rf /var/lib/apt/lists/*
