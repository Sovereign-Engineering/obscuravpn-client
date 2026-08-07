#!/usr/bin/env bash
set -eux

cd /repo
source contrib/shell/source-require-args.bash
source contrib/shell/source-gpg-helpers.bash

main() {
  local target_arches='' keys_dir=''
  require_args "target_arches keys_dir" "$@"

  gpg_packaging_container_setup
  local fingerprint
  fingerprint="$(key_fingerprints <"$keys_dir/current.public.asc")"

  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" >/out/obscura-archive.asc
  cd /out

  local arches dpkg_arches=() karch arch bindir
  read -ra arches <<<"$target_arches"
  for karch in "${arches[@]}"; do
    case "$karch" in
      x86_64) arch=amd64 ;;
      aarch64) arch=arm64 ;;
      *) die "unknown arch $karch" ;;
    esac
    dpkg_arches+=("$arch")
    bindir="dists/stable/main/binary-${arch}"
    mkdir -p "$bindir"
    dpkg-scanpackages --arch "$arch" pool >"$bindir/Packages"
    gzip -9c "$bindir/Packages" >"$bindir/Packages.gz"
  done

  local dep11_dir size components_yml
  appstreamcli validate --no-net /repo/linux/common/net.obscura.vpn.gui.metainfo.xml /repo/linux/common/appstream-catalog.xml
  components_yml="$(mktemp -d)/Components.yml"
  appstreamcli convert --format=yaml /repo/linux/common/appstream-catalog.xml "$components_yml"
  dep11_dir=dists/stable/main/dep11
  mkdir -p "$dep11_dir"
  for arch in "${dpkg_arches[@]}"; do
    gzip -9n -c "$components_yml" >"$dep11_dir/Components-${arch}.yml.gz"
  done
  for size in 64x64 128x128; do
    tar --owner=0 --group=0 -czf "$dep11_dir/icons-$size.tar.gz" -C "/repo/linux/common/icons/$size" .
  done

  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=Obscura \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Architectures="${dpkg_arches[*]}" \
    release dists/stable >dists/stable/Release
  gpg --batch --yes --local-user "$fingerprint" --clearsign -o dists/stable/InRelease dists/stable/Release
  gpg --batch --yes --local-user "$fingerprint" --detach-sign --armor -o dists/stable/Release.gpg dists/stable/Release

  cp pool/main/obscura-repository_*_all.deb obscura-repository.deb
}

main "$@"
