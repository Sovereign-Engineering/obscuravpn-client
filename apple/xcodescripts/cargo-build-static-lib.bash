#!/usr/bin/env bash

# Originally generated with cargo-xcode 1.10.0, since modified heavily
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH:/usr/local/bin:/opt/homebrew/bin"
## don't use ios/watchos linker for build scripts and proc macros
#export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/usr/bin/ld
#export CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER=/usr/bin/ld

export OBSCURA_CLIENT_RUSTLIB_CBINDGEN_OUTPUT_HEADER_PATH="$SCRIPT_OUTPUT_FILE_1"
export OBSCURA_CLIENT_RUSTLIB_CBINDGEN_CONFIG_PATH="$SCRIPT_INPUT_FILE_2"

CARGO_XCODE_CARGO_MANIFEST_PATH="${SCRIPT_INPUT_FILE:-"$SCRIPT_INPUT_FILE_1"}"

# NOTE: We need the '-' paramaeter expansion because we're in bash's "set -u" mode
if [ -n "${OTHER_INPUT_FILE_FLAGS-}" ]; then
	read -r -a CARGO_XCODE_CARGO_EXTRA_FLAGS <<<"$OTHER_INPUT_FILE_FLAGS"
else
	CARGO_XCODE_CARGO_EXTRA_FLAGS=("--lib")
fi

case "$PLATFORM_NAME" in
"macosx")
	CARGO_XCODE_TARGET_OS=darwin
	if [ "${IS_MACCATALYST-NO}" = YES ]; then
		CARGO_XCODE_TARGET_OS=ios-macabi
	fi
	;;
"iphoneos") CARGO_XCODE_TARGET_OS=ios ;;
"iphonesimulator") CARGO_XCODE_TARGET_OS=ios-sim ;;
"appletvos" | "appletvsimulator") CARGO_XCODE_TARGET_OS=tvos ;;
"watchos") CARGO_XCODE_TARGET_OS=watchos ;;
"watchsimulator") CARGO_XCODE_TARGET_OS=watchos-sim ;;
*)
	CARGO_XCODE_TARGET_OS="$PLATFORM_NAME"
	echo >&2 "warning: cargo-xcode needs to be updated to handle $PLATFORM_NAME"
	;;
esac

declare -a CARGO_XCODE_TARGET_TRIPLES
declare -a CARGO_XCODE_TARGET_FLAGS
declare -a LIPO_INPUT_FILES
for arch in $ARCHS; do
	if [[ "$arch" == "arm64" ]]; then arch=aarch64; fi
	if [[ "$arch" == "i386" && "$CARGO_XCODE_TARGET_OS" != "ios" ]]; then arch=i686; fi
	triple="${arch}-apple-$CARGO_XCODE_TARGET_OS"
	CARGO_XCODE_TARGET_TRIPLES+=("$triple")
	CARGO_XCODE_TARGET_FLAGS+=("--target=${triple}")
	LIPO_INPUT_FILES+=("$CARGO_TARGET_DIR/$triple/$CARGO_XCODE_BUILD_PROFILE/$CARGO_XCODE_CARGO_FILE_NAME")
done

echo >&2 "Cargo $CARGO_XCODE_BUILD_PROFILE $ACTION for $PLATFORM_NAME $ARCHS =${CARGO_XCODE_TARGET_TRIPLES[*]}; using ${SDK_NAMES:-}. \$PATH is:"
tr >&2 : '\n' <<<"$PATH"

if command -v rustup &>/dev/null; then
	for triple in "${CARGO_XCODE_TARGET_TRIPLES[@]}"; do
		if ! rustup target list --installed | grep -Eq "^$triple$"; then
			echo >&2 "warning: this build requires rustup toolchain for $triple, but it isn't installed (will try rustup next)"
			rustup target add "$triple" || {
				echo >&2 "warning: can't install $triple, will try nightly -Zbuild-std"
				CARGO_XCODE_CARGO_EXTRA_FLAGS+=("-Zbuild-std")
				if [ -z "${RUSTUP_TOOLCHAIN:-}" ]; then
					export RUSTUP_TOOLCHAIN=nightly
				fi
				break
			}
		fi
	done
fi

if [ "$CARGO_XCODE_BUILD_PROFILE" = release ]; then
	CARGO_XCODE_CARGO_EXTRA_FLAGS+=("--release")
fi

if [ "$ACTION" = clean ]; then
	cargo clean --verbose --manifest-path="$CARGO_XCODE_CARGO_MANIFEST_PATH" "${CARGO_XCODE_TARGET_FLAGS[@]}" "${CARGO_XCODE_CARGO_EXTRA_FLAGS[@]}"
	rm -f "$SCRIPT_OUTPUT_FILE_0" "$SCRIPT_OUTPUT_FILE_1" "$SCRIPT_OUTPUT_FILE_2"
	exit 0
fi
cargo build --verbose --manifest-path="$CARGO_XCODE_CARGO_MANIFEST_PATH" --features="${CARGO_XCODE_FEATURES:-}" "${CARGO_XCODE_TARGET_FLAGS[@]}" "${CARGO_XCODE_CARGO_EXTRA_FLAGS[@]}" || {
	echo >&2 "error: cargo build failed"
	exit 1
}

lipo "${LIPO_INPUT_FILES[@]}" -create -output "$SCRIPT_OUTPUT_FILE_0"

if [ -n "${LD_DYLIB_INSTALL_NAME-}" ]; then
	install_name_tool -id "$LD_DYLIB_INSTALL_NAME" "$SCRIPT_OUTPUT_FILE_0"
fi

echo "success: $ACTION of $SCRIPT_OUTPUT_FILE_0 for ${CARGO_XCODE_TARGET_TRIPLES[*]}"

# Generate .modulemap file
cat <<EOF >"$SCRIPT_OUTPUT_FILE_2"
module libobscuravpn_client {
    header "$(basename "$SCRIPT_OUTPUT_FILE_1")"

    export *
}
EOF
