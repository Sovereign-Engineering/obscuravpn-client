# shellcheck shell=bash

source contrib/shell/source-echoerr.bash

die() {
	echoerr "$@"
	exit 1
}
