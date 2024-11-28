#!/usr/bin/env bash
set -eo pipefail

exec -- \
	git ls-files \
		--exclude-standard \
		--others \
		--cached \
		"$@"
