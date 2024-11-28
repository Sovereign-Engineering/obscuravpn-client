#!/usr/bin/env bash
set -euo pipefail

source contrib/shell/source-echoerr.bash
source contrib/shell/source-die.bash

if [ "$#" -ne 0 ]; then
	die "No parameters accepted."
fi

KEYCHAIN_PROFILE="notarytool-password"
CERT='Developer ID Application: Sovereign Engineering Inc. (5G943LR562)'

APP_NAME="Obscura VPN"
APP_BASENAME="$APP_NAME.app"
DMG_FILE_NAME="$APP_NAME.dmg"
VOLUME_NAME="$APP_NAME"

BACKGROUND="apple/dmg-building/installer_background.tiff"

# Temp directory setup
TMP_DIR=$(mktemp -d)

cleanup() {
	echo "Cleaning up temporary directory: $TMP_DIR"
	rm -rf "$TMP_DIR"
}

trap cleanup EXIT

ARCHIVE_DIR="$TMP_DIR/client-prod.xcarchive"
EXPORT_DIR="$TMP_DIR/client-prod-export"
APP_PATH="$EXPORT_DIR/$APP_BASENAME"

SOURCE_DIR="$TMP_DIR/dmg-contents"
mkdir "$SOURCE_DIR"

# Size of the Finder window toolbar
WINDOW_TOP_PADDING=28

# NOTE: Keep in sync with "$BACKGROUND"
ICON_SIZE=120

# We need to specify the center location of the icons
ICON_CENTERING_DELTA=$((ICON_SIZE / 2))

ICONS_Y_FROM_TOP=277

OBSCURA_APP_ICON_X_FROM_LEFT=287
APPLICATIONS_DROP_ICON_X_FROM_LEFT=553

BACKGROUND_IMAGE_HEIGHT=601
BACKGROUND_IMAGE_WIDTH=960

WINDOW_POS_X=200
WINDOW_POS_Y=120

set -x

xcodebuild archive \
	-workspace apple/client.xcodeproj/project.xcworkspace \
	-scheme 'Prod Client' \
	-archivePath "$ARCHIVE_DIR"

xcodebuild -exportArchive \
	-archivePath "$ARCHIVE_DIR" \
	-exportOptionsPlist apple/ExportOptions.plist \
	-exportPath "$EXPORT_DIR"

NOTARIZE_ZIP="$TMP_DIR/obscura-notarize.zip"
ditto -c -k --keepParent "$APP_PATH" "$NOTARIZE_ZIP"
xcrun notarytool submit \
	--keychain-profile "$KEYCHAIN_PROFILE" \
	--verbose \
	--wait \
	"$NOTARIZE_ZIP"

xcrun stapler staple -v "$APP_PATH"
xcrun stapler validate -v "$APP_PATH"

# Ref: https://developer.apple.com/forums/thread/130560
spctl -a -t exec -vvv "$APP_PATH"

mv "$APP_PATH" "$SOURCE_DIR/$APP_BASENAME"

# Create the DMG
rm -vf "$DMG_FILE_NAME"
create-dmg \
	--volname "${VOLUME_NAME}" \
	--background "$BACKGROUND" \
	--window-pos "$WINDOW_POS_X" "$WINDOW_POS_Y" \
	--window-size "$BACKGROUND_IMAGE_WIDTH" $(( BACKGROUND_IMAGE_HEIGHT + WINDOW_TOP_PADDING )) \
	--icon-size "$ICON_SIZE" \
	--icon "$APP_BASENAME" $(( OBSCURA_APP_ICON_X_FROM_LEFT + ICON_CENTERING_DELTA )) $(( ICONS_Y_FROM_TOP + ICON_CENTERING_DELTA )) \
	--hide-extension "$APP_BASENAME" \
	--app-drop-link $(( APPLICATIONS_DROP_ICON_X_FROM_LEFT + ICON_CENTERING_DELTA )) $(( ICONS_Y_FROM_TOP + ICON_CENTERING_DELTA )) \
	--no-internet-enable \
	"$DMG_FILE_NAME" \
	"$SOURCE_DIR"

# Codesign the DMG
# Ref: https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html
codesign --sign "$CERT" "$DMG_FILE_NAME"
xcrun notarytool submit \
	--keychain-profile "$KEYCHAIN_PROFILE" \
	--verbose \
	--wait \
	"$DMG_FILE_NAME"

xcrun stapler staple -v "$DMG_FILE_NAME"
xcrun stapler validate -v "$DMG_FILE_NAME"

# Ref: https://developer.apple.com/library/archive/technotes/tn2206/_index.html
spctl -a -t open --context context:primary-signature -v "$DMG_FILE_NAME"

# Ref: https://wiki.freepascal.org/Notarization_for_macOS_10.14.5%2B#Step_7_-_Verify_notarization_of_the_disk_image
spctl -a -vv -t install "$DMG_FILE_NAME"
