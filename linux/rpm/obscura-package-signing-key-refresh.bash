#!/bin/bash
set -eu

keyfile=/etc/pki/rpm-gpg/RPM-GPG-KEY-obscura
revoked_keyfile=/etc/pki/rpm-gpg/RPM-GPG-KEY-obscura-revoked

rpm --import "$keyfile"

while read -r fpr; do
  rpm -e --allmatches "gpg-pubkey-$fpr" 2>/dev/null || :
  rpm -e --allmatches "gpg-pubkey-${fpr:32}" 2>/dev/null || :
done <"$revoked_keyfile"
