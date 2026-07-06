source contrib/shell/source-die.bash

require_args() {
    local required_keys="$1"
    shift

    local seen_keys=" "
    while [[ "$#" -gt 0 ]]; do
        if [[ "$1" == --* ]]; then
            printf -v "${1#--}" '%s' "$2" # assigns the caller's pre-declared local
            seen_keys+="${1#--} "
            shift 2
        else
            die "Error: Unexpected parameter '$1'. Expected --key value."
        fi
    done
    local req
    for req in $required_keys; do
        if [[ "$seen_keys" != *" $req "* ]]; then
            die "Error: Missing required argument '--$req'"
        fi
    done
}
