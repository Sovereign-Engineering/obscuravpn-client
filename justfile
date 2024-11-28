# NOTE: Must be first recipe to be default
@_default:
	just --list

@_check-in-obscura-nix-shell:
	./contrib/bin/check-in-obscura-nix-shell.bash

# check formatting
format-check: _check-in-obscura-nix-shell
	swiftformat --lint .
	cargo --offline fmt --check
	./contrib/bin/nixfmt-auto-files.bash --check

# fix formatting
format-fix: _check-in-obscura-nix-shell
	swiftformat .
	cargo --offline fmt
	./contrib/bin/nixfmt-auto-files.bash

# lint checks
lint: _check-in-obscura-nix-shell
	./contrib/bin/shellcheck-auto-files.bash

web-bundle-dir := "./obscura-ui/"

web-bundle-build:
	just "{{web-bundle-dir}}"/build

web-bundle-start:
	just "{{web-bundle-dir}}"/start

xcode-open:
	open -a /Applications/Xcode.app apple/client.xcodeproj

# build notarized .dmg in current directory from APP
build-dmg: _check-in-obscura-nix-shell
	./contrib/bin/build-obscuravpn-dmg.bash
