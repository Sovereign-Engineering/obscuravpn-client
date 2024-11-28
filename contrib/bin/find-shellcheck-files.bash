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

shell_script_git_patterns=(
	'*.sh'
	'*.bash'
)

./contrib/bin/ls-non-ignored-files.bash ${NULL_OUTPUT:+-z} -- "${shell_script_git_patterns[@]}"
