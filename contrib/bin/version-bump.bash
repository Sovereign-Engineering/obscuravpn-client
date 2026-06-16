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

# TODO: https://linear.app/soveng/issue/OBS-3629/remove-requirement-to-bump-android-version-code
sed -i "" -Ee "s/        versionCode = [0-9]+/        versionCode = ${version#*.}/" android/app/build.gradle.kts

git commit -a -m "Tag v$version."
git tag -s "v/$version" -m "v/$version"
