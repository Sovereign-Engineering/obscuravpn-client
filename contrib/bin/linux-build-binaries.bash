#!/usr/bin/env bash
set -eux

source contrib/shell/source-die.bash

main() {
  local release='' target_arch='' locked=''
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --release) release='--release'; shift ;;
      --locked) locked='--locked'; shift ;;
      --target_arch) target_arch="$2"; shift 2 ;;
      *) die "linux-build-binaries: unexpected argument '$1'" ;;
    esac
  done

  : "${target_arch:=$(uname -m)}"
  local platform
  case "$target_arch" in
    x86_64) platform=amd64 ;;
    aarch64) platform=arm64 ;;
    *) die "linux-build-binaries: unknown arch '$target_arch'" ;;
  esac

  local obscura_version
  obscura_version="$(cat "$(nix build '.#version' --no-link --print-out-paths)")"
  local gresources
  gresources="$(nix build '.#gui-gresources' --no-link --print-out-paths)"

  mkdir -p "result-linux/target-${target_arch}" result-linux/cargo

  local userns
  userns=(--user "$(id -u):$(id -g)")
  if docker -v 2>&1 | grep -iq "podman"; then
    userns=(--userns=keep-id)
  fi

  docker build --platform "linux/$platform" -f linux/build_Dockerfile -t "obscura-build:$platform" .

  docker run --rm "${userns[@]}" --security-opt label=disable \
    -e OBSCURA_VERSION="$obscura_version" \
    -v "$PWD:/src" \
    -v "$gresources:/gresources:ro" \
    -e OBSCURA_GRESOURCES_DIR=/gresources \
    -v "$PWD/result-linux/target-${target_arch}:/cargo-target" \
    -v "$PWD/result-linux/cargo:/cargo" \
    "obscura-build:$platform" bash -euxc "
      cd /src/rustlib
      CARGO_HOME=/cargo CARGO_TARGET_DIR=/cargo-target/cli cargo build $locked $release --bin obscura
      CARGO_HOME=/cargo CARGO_TARGET_DIR=/cargo-target/gui cargo build $locked $release --features gui --bin obscura-gui
    "
}

main "$@"
