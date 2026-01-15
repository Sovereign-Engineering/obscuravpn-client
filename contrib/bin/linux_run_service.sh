#!/usr/bin/env bash
set -eux

(cd rustlib && cargo build)
sudo --preserve-env=RUST_LOG sg obscura "umask 002 && ./rustlib/target/debug/obscura service"
