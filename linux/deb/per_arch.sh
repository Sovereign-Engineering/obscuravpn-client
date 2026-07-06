#!/usr/bin/env bash
set -eux

cd /repo
source contrib/shell/source-require-args.bash

main() {
  local version='' repo_url='' target_arch='' keys_dir=''
  require_args "version repo_url target_arch keys_dir" "$@"

  local arch
  case "$target_arch" in
    x86_64) arch=amd64 ;;
    aarch64) arch=arm64 ;;
    *) die "unknown arch $target_arch" ;;
  esac

  version="${version#v}"
  version="${version%%-*}"

  local build_dir=/build/obscura
  mkdir -p "$build_dir/debian"
  cd "$build_dir"

  sed "s/@ARCH@/${arch}/g" /repo/linux/deb/control >debian/control
  install -m755 /repo/linux/deb/rules debian/rules
  cp /repo/linux/common/obscura-sysusers.conf debian/obscura-cli.sysusers
  { echo "Copyright 2025 Sovereign Engineering Inc. All rights reserved."; echo; cat /repo/LICENSE.md; } >debian/copyright
  cat >debian/changelog <<EOF
obscura (${version}) stable; urgency=low

  * Release ${version}

 -- Obscura Repository Signer <packages@obscura.com>  $(date -uR)
EOF

  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" | gpg --dearmor >obscura-archive-keyring.gpg

  cat >obscura.sources <<EOF
Types: deb
URIs: ${repo_url}/deb
Suites: stable
Components: main
Architectures: ${arch}
Signed-By: /usr/share/keyrings/obscura-archive-keyring.gpg
EOF

  cp "/repo/result-linux/target-$target_arch/cli/release/obscura" obscura
  cp "/repo/result-linux/target-$target_arch/gui/release/obscura-gui" obscura-gui

  DEB_BUILD_OPTIONS=nostrip dpkg-buildpackage -us -uc -b
  if ! lintian --allow-root --fail-on error,warning --suppress-tags no-manual-page,description-starts-with-package-name,package-installs-apt-sources,unstripped-binary-or-object /build/*.deb; then
    die "lintian reported findings; failing build"
  fi

  mkdir -p /out/pool/main
  cp /build/obscura-cli_*_"${arch}".deb /build/obscura-gui_*_"${arch}".deb \
    /build/obscura_*_all.deb /build/obscura-repository_*_all.deb /out/pool/main/
}

main "$@"
