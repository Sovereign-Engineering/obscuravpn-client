#!/usr/bin/env bash
exec nix run '.#gradle-deps-update' --print-build-logs
