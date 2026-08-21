#!/usr/bin/env bash
set -euo pipefail

# Rotates the repository signing keys: promote next -> current, generate a fresh
# next, and revoke the outgoing current key so it is no longer trusted.
#
# Runs on an ephemeral machine: the secret keys are decrypted on the fly from
# ENCRYPTED_PRIVKEYS_DIR (e.g. a USB stick) with the recipient key (e.g. a
# YubiKey), and the newly generated next key is added there the same way.

extract_fingerprint() {
	awk -F: '/^pub:/{p=1} /^fpr:/&&p{print $10; p=0}'
}

encrypted_privkey_file() {
	local fingerprint="$1"
	local dir="$2"
	local recipient_fingerprint="$3"

	local file="$dir/$fingerprint-privkey-encrypted-to-$recipient_fingerprint.asc.asc"
	if ! [ -f "$file" ]; then
		echo "missing encrypted export '$file'" >&2
		return 1
	fi
	printf '%s\n' "$file"
}

export_privkey_encrypted() {
	local fingerprint="$1"
	local recipient_file="$2"

	local privkey
	privkey="$(gpg --armor --export-secret-keys "$fingerprint")"
	if [ -z "$privkey" ]; then
		echo "no secret key $fingerprint in keyring" >&2
		return 1
	fi

	printf '%s\n' "$privkey" | gpg --armor --encrypt --recipient-file "$recipient_file"
}

usage() {
	cat >&2 <<EOF
usage: $0 ENCRYPTED_PRIVKEYS_DIR PRIVKEY_RECIPIENT_FILE

ENCRYPTED_PRIVKEYS_DIR   existing directory holding the secret keys, one file
                         per key, encrypted to the recipient key; the newly
                         generated next key is added to it the same way
PRIVKEY_RECIPIENT_FILE   file holding exactly one public key; the exported
                         secret keys are encrypted to it
EOF
	exit 2
}

main() {
	if [ "$#" -ne 2 ]; then
		usage
	fi

	local encrypted_privkeys_dir
	encrypted_privkeys_dir="$(realpath -m "$1" 2>/dev/null)" || encrypted_privkeys_dir=""
	if ! [ -d "$encrypted_privkeys_dir" ] || ! [ -w "$encrypted_privkeys_dir" ]; then
		echo "ENCRYPTED_PRIVKEYS_DIR '$1' is not a writable directory" >&2
		exit 1
	fi

	local privkey_recipient_file
	privkey_recipient_file="$(realpath -m "$2" 2>/dev/null)" || privkey_recipient_file=""
	if ! [ -f "$privkey_recipient_file" ]; then
		echo "PRIVKEY_RECIPIENT_FILE '$2' is not a file" >&2
		exit 1
	fi

	local privkey_recipient_fingerprint
	privkey_recipient_fingerprint="$(gpg --with-colons --show-keys "$privkey_recipient_file" | extract_fingerprint)" || true
	case "$privkey_recipient_fingerprint" in
	"")
		echo "PRIVKEY_RECIPIENT_FILE '$2' contains no public key" >&2
		exit 1
		;;
	*$'\n'*)
		echo "PRIVKEY_RECIPIENT_FILE '$2' contains more than one public key" >&2
		exit 1
		;;
	esac

	local keys_dir
	keys_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

	local reply
	read -rp $'Rotating again before the previous rotation reaches users strands clients that\nhave not fetched the repositories since then (next buys one rotation only).\nHas the previous rotation reached all users? [y/N] ' reply || true
	case "$reply" in
	y | Y) ;;
	*)
		echo "aborted" >&2
		exit 1
		;;
	esac

	echo "reading current and next key fingerprints..." >&2
	local outgoing_fingerprint
	outgoing_fingerprint="$(gpg --with-colons --show-keys "$keys_dir/current.public.asc" | extract_fingerprint)"
	local new_current_fingerprint
	new_current_fingerprint="$(gpg --with-colons --show-keys "$keys_dir/next.public.asc" | extract_fingerprint)"

	# Both are necessary for `gpg` to recognize the secret key for the next steps
	gpg --import "$privkey_recipient_file"
	gpg --card-status >/dev/null 2>&1 || true

	echo "decrypting outgoing current privkey (needed to revoke it)..." >&2
	local outgoing_privkey_file
	outgoing_privkey_file="$(encrypted_privkey_file "$outgoing_fingerprint" "$encrypted_privkeys_dir" "$privkey_recipient_fingerprint")"
	local outgoing_privkey
	outgoing_privkey="$(gpg --quiet --decrypt "$outgoing_privkey_file")"

	echo "verifying that the new current privkey decrypts..." >&2
	local new_current_privkey_file
	new_current_privkey_file="$(encrypted_privkey_file "$new_current_fingerprint" "$encrypted_privkeys_dir" "$privkey_recipient_fingerprint")"
	gpg --quiet --decrypt "$new_current_privkey_file" >/dev/null

	echo "generating new next key..." >&2
	local new_next_fingerprint
	new_next_fingerprint="$(gpg --yes --status-fd 1 --pinentry-mode loopback --passphrase '' --quick-generate-key 'Obscura Repository Signer <packages@obscura.com>' rsa4096 sign 10y | awk '$1 == "[GNUPG:]" && $2 == "KEY_CREATED" {print $4}')"

	echo "exporting new next public key..." >&2
	local new_next_public
	new_next_public="$(gpg --armor --export "$new_next_fingerprint")"

	echo "exporting and encrypting new next privkey..." >&2
	local new_next_privkey_encrypted_data
	new_next_privkey_encrypted_data="$(export_privkey_encrypted "$new_next_fingerprint" "$privkey_recipient_file")"

	echo "revoking outgoing current key $outgoing_fingerprint (answer gpg's prompts)..." >&2
	printf '%s\n' "$outgoing_privkey" | gpg --quiet --import
	gpg --armor --gen-revoke "$outgoing_fingerprint" | gpg --import
	local outgoing_public_revoked
	outgoing_public_revoked="$(gpg --armor --export "$outgoing_fingerprint")"

	echo "writing rotated keys..." >&2
	local new_next_privkey_encrypted="${encrypted_privkeys_dir}/${new_next_fingerprint}-privkey-encrypted-to-${privkey_recipient_fingerprint}.asc.asc"
	printf '%s\n' "$new_next_privkey_encrypted_data" >"$new_next_privkey_encrypted"
	printf '%s\n' "$outgoing_public_revoked" >>"$keys_dir/revocation.asc"
	mv "$keys_dir/next.public.asc" "$keys_dir/current.public.asc"
	printf '%s\n' "$new_next_public" >"$keys_dir/next.public.asc"

	cat <<EOF

rotated:
  revoked old current:    $outgoing_fingerprint
  new current (was next): $new_current_fingerprint
  new next:               $new_next_fingerprint

Send the new current secret key to the signer (it signs releases):
  gpg --decrypt '$new_current_privkey_file' | gpg --armor --encrypt --recipient ...

When copying the directory back to commit and publish, copy only:
  current.public.asc  next.public.asc  revocation.asc
EOF
}

main "$@"
