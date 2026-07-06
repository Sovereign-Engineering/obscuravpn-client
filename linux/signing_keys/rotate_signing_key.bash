#!/usr/bin/env bash
set -euo pipefail

# Rotates the repository signing keys: promote next -> current, generate a fresh
# next, and revoke the outgoing current key so it is no longer trusted.
#
# Run on the isolated machine that holds the secret keys; the whole directory can
# be copied there and back.

extract_fingerprint() {
	awk -F: '/^pub:/{p=1} /^fpr:/&&p{print $10; p=0}'
}

main() {
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

	echo "generating new next key..." >&2
	local new_next_fingerprint
	new_next_fingerprint="$(gpg --yes --with-colons --quick-generate-key 'Obscura Repository Signer <packages@obscura.com>' rsa4096 sign 10y | extract_fingerprint)"

	echo "revoking outgoing current key $outgoing_fingerprint (answer gpg's prompts)..." >&2
	gpg --armor --gen-revoke "$outgoing_fingerprint" | gpg --import
	gpg --armor --export "$outgoing_fingerprint" >>"$keys_dir/revocation.asc"

	echo "promoting next to current..." >&2
	mv "$keys_dir/next.public.asc" "$keys_dir/current.public.asc"

	echo "writing new next key..." >&2
	gpg --armor --export "$new_next_fingerprint" >"$keys_dir/next.public.asc"

	cat <<EOF

rotated:
  revoked old current:    $outgoing_fingerprint
  new current (was next): $new_current_fingerprint
  new next:               $new_next_fingerprint

Send the new current secret key to the signer (it signs releases):
  gpg --export-secret-keys --armor $new_current_fingerprint

Back up the new next secret key separately (it signs releases after the next rotation):
  gpg --export-secret-keys --armor $new_next_fingerprint

When copying the directory back to commit and publish, copy only:
  current.public.asc  next.public.asc  revocation.asc
EOF
}

main "$@"
