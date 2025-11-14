#!/bin/bash
set -ex

# expects to run inside nix develop .#android shell

cd rustlib

rustup target add aarch64-linux-android

cargo ndk -t arm64-v8a build --release

cd ../android

./gradlew --no-daemon spotlessCheck build
