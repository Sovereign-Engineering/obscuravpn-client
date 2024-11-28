#!/usr/bin/env bash

set -euo pipefail

cd "$SRCROOT/.."

source contrib/shell/source-echoerr.bash

git_commit=$(git rev-parse HEAD)
if ! git diff --quiet; then
	git_commit="$git_commit-dirty"
fi

git_describe=$(git describe --match "v/*" --abbrev=12 --dirty)
echoerr "git describe: $git_describe"
git_tag="${git_describe%%-*}"

build_version=$(date -u '+1.%Y%m%d.%H%M%S')

marketing_version="${git_tag#v/}"
if [[ "$git_tag" != "$git_describe" ]]; then
	# For builds that don't exactly match a tag add a `.1` to indicate a "dev" build.
	marketing_version="${marketing_version}.1"
fi

source_version="v${git_describe#v/}"

tee apple/Configurations/buildversion.xcconfig <<END
// NOTE: This file is generated prior to each build, and is git-ignored

CURRENT_PROJECT_VERSION = $build_version
MARKETING_VERSION = $marketing_version
OBSCURA_SOURCE_ID = $git_commit
OBSCURA_SOURCE_VERSION = $source_version
END
