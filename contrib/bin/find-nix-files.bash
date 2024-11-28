#!/usr/bin/env bash
set -eo pipefail

source contrib/shell/source-die.bash

# Parse command line options
while getopts ":z" opt; do
  case $opt in
    z)
      NULL_OUTPUT=true
      ;;
    \?)
      die "Invalid option: -${OPTARG}"
      ;;
  esac
done

nix_file_git_patterns=(
  '*.nix'
)

./contrib/bin/ls-non-ignored-files.bash ${NULL_OUTPUT:+-z} -- "${nix_file_git_patterns[@]}"
