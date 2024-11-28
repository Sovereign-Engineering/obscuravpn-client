#!/usr/bin/env bash
set -eo pipefail

./contrib/bin/find-shellcheck-files.bash -z | exec xargs --null -- shellcheck --
