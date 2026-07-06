# shellcheck shell=bash

source contrib/shell/source-die.bash

key_fingerprints() {
    gpg --batch --with-colons --show-keys | awk -F: '/^(pub|sec):/{k=1} /^fpr:/ && k {print $10; k=0}'
}

gpg_packaging_container_setup() {
    if [ -z "${GPG_PRIVATE_KEY:+x}" ]; then
        die "gpg_packaging_container_setup: GPG_PRIVATE_KEY is empty"
    fi
    GNUPGHOME="$(mktemp -d)"
    export GNUPGHOME
    gpg --batch --quiet --import <<<"$GPG_PRIVATE_KEY"
    local fingerprint
    fingerprint="$(key_fingerprints <<<"$GPG_PRIVATE_KEY" | head -1)"
    if ! echo probe | gpg --batch --yes --pinentry-mode loopback --passphrase '' --local-user "$fingerprint" --detach-sign -o /dev/null - 2>/dev/null; then
        if [ -z "${GPG_PASSPHRASE:+x}" ]; then
            die "signing key is passphrase protected but no passphrase was provided"
        fi
        gpg --no-greeting --command-fd 0 --pinentry-mode loopback --change-passphrase "$fingerprint" <<EOF
$GPG_PASSPHRASE


y
EOF
    fi
    echo probe | gpg --batch --yes --local-user "$fingerprint" --detach-sign -o /dev/null - \
        || die "signing key is unusable without a passphrase prompt; wrong passphrase?"
}
