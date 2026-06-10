#!/usr/bin/env bash
set -eu

if [[ $# != 1 ]]; then
	echo "Exactly one argument (the version number) is required. Got $#"
	exit 1
fi

version="$1"

nix build '.#hash'
hash="$(cat result)"

cat <<END >tag.json
{
  "sourceHash": "$hash",
  "version": "$version"
}
END

git commit -a -m "Tag v$version."
git tag -s "v/$version" -m "v/$version"
