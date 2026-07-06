#!/usr/bin/env bash
set -eux

cd /repo
source contrib/shell/source-require-args.bash
source contrib/shell/source-gpg-helpers.bash

main() {
  local version='' target_arch='' keys_dir=''
  require_args "version target_arch keys_dir" "$@"

  gpg_packaging_container_setup
  local fingerprint
  fingerprint="$(key_fingerprints <"$keys_dir/current.public.asc")"

  version="${version#v}"
  version="${version%%-*}"

  local work="$HOME/build"
  mkdir -p "$work/src"
  cd "$work"
  sed -e "s/@VERSION@/${version}/g" -e "s/@TARGET_ARCH@/${target_arch}/g" /repo/linux/arch/PKGBUILD >PKGBUILD
  cp /repo/linux/arch/obscura-keyring.install .
  cp "/repo/result-linux/target-$target_arch/cli/release/obscura" src/obscura
  cp "/repo/result-linux/target-$target_arch/gui/release/obscura-gui" src/obscura-gui

  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" "$keys_dir/revocation.asc" | gpg --dearmor >src/obscura.gpg
  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" | key_fingerprints | sed 's/$/:4:/' >src/obscura-trusted
  if [ -s "$keys_dir/revocation.asc" ]; then
    key_fingerprints <"$keys_dir/revocation.asc" >src/obscura-revoked
  else
    : >src/obscura-revoked
  fi

  makepkg -f
  local namcap_out
  namcap_out="$(namcap ./*.pkg.tar.zst)"
  printf '%s\n' "$namcap_out"
  if printf '%s\n' "$namcap_out" | grep -qE '^\S+ E: '; then
    die "namcap reported errors; failing build"
  fi
  local p
  for p in ./*.pkg.tar.zst; do gpg --batch --yes --local-user "$fingerprint" --detach-sign -o "${p}.sig" "$p"; done

  local repo_dir="$HOME/repo"
  mkdir -p "$repo_dir"
  cp ./*.pkg.tar.zst ./*.pkg.tar.zst.sig "$repo_dir/"
  cat "$keys_dir/current.public.asc" "$keys_dir/next.public.asc" >"$repo_dir/obscura-archive.asc"
  ( cd "$repo_dir"
    repo-add obscura.db.tar.zst ./*.pkg.tar.zst )
  sudo mkdir -p "/out/${target_arch}"
  sudo cp -a --no-preserve=ownership "$repo_dir/." "/out/${target_arch}/"
}

main "$@"
