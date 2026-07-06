#!/usr/bin/env bash
set -eux

cd /repo
source contrib/shell/source-require-args.bash
source contrib/shell/source-gpg-helpers.bash

main() {
  local version='' repo_url='' target_arch='' keys_dir=''
  require_args "version repo_url target_arch keys_dir" "$@"

  gpg_packaging_container_setup
  local fingerprint
  fingerprint="$(key_fingerprints <"$keys_dir/current.public.asc")"
  echo "%_gpg_name ${fingerprint}" >"$HOME/.rpmmacros"

  version="${version#v}"
  version="${version%%-*}"

  local sources="$HOME/rpmbuild/SOURCES"
  mkdir -p "$sources" "$HOME/rpmbuild/SPECS"
  cp "/repo/result-linux/target-$target_arch/cli/release/obscura" "$sources/obscura"
  cp "/repo/result-linux/target-$target_arch/gui/release/obscura-gui" "$sources/obscura-gui"
  sed -e "s/@VERSION@/${version}/g" -e "s/@DATE@/$(date "+%a %b %d %Y")/g" /repo/linux/rpm/obscura.spec >"$HOME/rpmbuild/SPECS/obscura.spec"

  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" >"$sources/RPM-GPG-KEY-obscura"
  if [ -s "$keys_dir/revocation.asc" ]; then
    key_fingerprints <"$keys_dir/revocation.asc" | tr '[:upper:]' '[:lower:]' >"$sources/RPM-GPG-KEY-obscura-revoked"
  else
    : >"$sources/RPM-GPG-KEY-obscura-revoked"
  fi

  cat >"$sources/obscura.repo" <<EOF
[obscura]
name=Obscura VPN
baseurl=${repo_url}/rpm/\$basearch
enabled=1
gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-obscura ${repo_url}/rpm/RPM-GPG-KEY-obscura
EOF

  rpmbuild -bb "$HOME/rpmbuild/SPECS/obscura.spec"
  if ! rpmlint -r /repo/linux/rpm/rpmlintrc "$HOME"/rpmbuild/RPMS/*/*.rpm; then
    die "rpmlint reported findings; failing build"
  fi
  find "$HOME/rpmbuild/RPMS" -name '*.rpm' -exec rpm --addsign {} +

  local out_dir="/out/${target_arch}"
  mkdir -p "$out_dir"
  find "$HOME/rpmbuild/RPMS" -name '*.rpm' -exec cp {} "$out_dir/" \;
  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" >/out/RPM-GPG-KEY-obscura
  createrepo_c "$out_dir"
}

main "$@"
