#!/usr/bin/env bash
set -eux

source contrib/shell/source-require-args.bash
source contrib/shell/source-die.bash
source contrib/shell/source-gpg-helpers.bash

pkg_container() {
  local docker_args='' dist='' package_format='' cmd=''
  require_args "docker_args dist package_format cmd" "$@"
  read -ra docker_args <<<"$docker_args"
  docker run --rm --network none --security-opt label=disable \
    "${docker_args[@]}" \
    -e GPG_PRIVATE_KEY \
    -e GPG_PASSPHRASE \
    -v "$PWD:/repo:ro" \
    -v "$PWD/result-linux/$dist/$package_format:/out" \
    "obscura-$package_format" sh -c "$cmd"
}

check_key_expiry() {
  local key='' years=''
  require_args "key years" "$@"
  local exp
  exp="$(gpg --batch --with-colons --show-keys "$key" | awk -F: '/^pub:/{print $7; exit}')"
  if [ -n "$exp" ] && [ "$exp" -lt "$(( $(date +%s) + years * 365 * 24 * 60 * 60 ))" ]; then
    die "$key must be valid for at least $years year(s)"
  fi
}

check_not_revoked() {
  local key='' revocations=''
  require_args "key revocations" "$@"
  [ -s "$revocations" ] || return 0
  local fingerprint
  fingerprint="$(key_fingerprints <"$key" | head -1)"
  if key_fingerprints <"$revocations" | grep -qx "$fingerprint"; then
    die "$key is revoked"
  fi
}

load_signing_key() {
  local keys_dir=''
  require_args "keys_dir" "$@"
  set +x
  if [ -f "$PWD/$keys_dir/current.private.asc" ]; then
    GPG_PRIVATE_KEY="$(cat "$PWD/$keys_dir/current.private.asc")"
    GPG_PASSPHRASE=
  else
    local signer
    signer="$(key_fingerprints <"$PWD/$keys_dir/current.public.asc" | head -1)"
    GPG_PRIVATE_KEY="$(gpg --batch --armor --export-secret-keys "$signer" 2>/dev/null || true)"
    [ -n "$GPG_PRIVATE_KEY" ] || die "no secret key for the current signing key in gpg; import it or use --test"
    read -rsp "Signing key passphrase (empty if none): " GPG_PASSPHRASE || GPG_PASSPHRASE=
    echo
  fi
  set -x
}

main() {
  local version
  version="$(cat "$(nix build '.#version' --no-link --print-out-paths)")"
  version="${version/.1-/.$(date +%s)-}"

  local dist repo_url keys_dir
  if [ "${1:-}" = "--test" ]; then
    dist="dist-test"
    repo_url=http://10.0.2.2:54321
    keys_dir=linux/signing_keys_test
  elif [[ "$version" == *-* ]]; then
    die "refusing to build production packages: ${version} is not a clean tagged release; pass --test for test packages"
  else
    dist="dist-prod"
    repo_url=https://linux-pkgs.obscura.com
    keys_dir=linux/signing_keys
  fi

  local -x GPG_PRIVATE_KEY GPG_PASSPHRASE
  load_signing_key --keys_dir "$keys_dir"

  check_key_expiry --key "$PWD/$keys_dir/current.public.asc" --years 1
  check_key_expiry --key "$PWD/$keys_dir/next.public.asc" --years 5
  check_not_revoked --key "$PWD/$keys_dir/current.public.asc" --revocations "$PWD/$keys_dir/revocation.asc"
  check_not_revoked --key "$PWD/$keys_dir/next.public.asc" --revocations "$PWD/$keys_dir/revocation.asc"
  [ "$(cat "$PWD/$keys_dir/current.public.asc" "$PWD/$keys_dir/next.public.asc" | key_fingerprints | wc -l)" = 2 ] \
    || die "expected exactly 2 keys in current+next"
  if [ -s "$PWD/$keys_dir/revocation.asc" ]; then
    [ "$(key_fingerprints <"$PWD/$keys_dir/revocation.asc" | wc -l)" = "$(grep -c 'BEGIN PGP' "$PWD/$keys_dir/revocation.asc")" ] \
      || die "revocation.asc must contain full public keys"
  fi

  local arches target_arch package_format
  arches=(x86_64)

  for target_arch in "${arches[@]}"; do
    ./contrib/bin/linux-build-binaries.bash --release --locked --target_arch "$target_arch"
  done

  for package_format in deb rpm arch; do
    docker build -t "obscura-$package_format" "linux/$package_format"
    mkdir -p "$PWD/result-linux/$dist/$package_format"
    pkg_container --docker_args "--user 0" --dist "$dist" --package_format "$package_format" \
      --cmd "rm -rf /out/*"
    for target_arch in "${arches[@]}"; do
      pkg_container --docker_args "" --dist "$dist" --package_format "$package_format" \
        --cmd "/repo/linux/$package_format/per_arch.sh --version $version --repo_url $repo_url --target_arch $target_arch --keys_dir /repo/$keys_dir"
    done
    pkg_container --docker_args "" --dist "$dist" --package_format "$package_format" \
      --cmd "/repo/linux/$package_format/finalize.sh --target_arches \"${arches[*]}\" --keys_dir /repo/$keys_dir"
    # Real docker writes /out as root; hand the repo back to the caller (no-op on podman).
    # TODO: Podman is widely available, should we just use rootless podman on all host distros?
    docker -v 2>&1 | grep -iq "podman" || pkg_container --docker_args "--user 0" --dist "$dist" --package_format "$package_format" \
      --cmd "chown -R $(id -u):$(id -g) /out"
  done

  echo "Built signed deb/rpm/arch repositories in result-linux/$dist"
}

main "$@"
