# NOTE: Must be first recipe to be default
set windows-shell := ["powershell", "-c"]

@_default:
	just --list

@_check-in-obscura-nix-shell:
	./contrib/bin/check-in-obscura-nix-shell.bash

# fix formatting
format-fix: _check-in-obscura-nix-shell
	cd android && gradle ktfmtFormat
	swiftformat .
	cd rustlib && cargo --offline fmt
	./contrib/bin/nixfmt-auto-files.bash
	taplo format
	cargo run --manifest-path xtask/Cargo.toml -- fix rustlib

# lint checks
lint: _check-in-obscura-nix-shell
	./contrib/bin/shellcheck-auto-files.bash

web-bundle-dir := "./obscura-ui/"

[unix]
web-bundle-build:
	just "{{web-bundle-dir}}"/build

licenses-windows := "./windows/webui-build/licenses.json"

[windows]
web-bundle-build: gen-license
	$env:OBS_WEB_PLATFORM = "windows"; $env:LICENSE_JSON = ".{{licenses-windows}}"; just "{{web-bundle-dir}}build"

web-bundle-start:
	just "{{web-bundle-dir}}"/start

xcode-open:
	open -a /Applications/Xcode.app apple/client.xcodeproj

# build notarized .dmg in current directory from APP
build-dmg: _check-in-obscura-nix-shell
	./contrib/bin/build-obscuravpn-dmg.bash

licenses-node-windows := "./windows/webui-build/licenses-node.json"
licenses-rust-windows := "./windows/webui-build/licenses-rust.json"

[windows]
gen-license: gen-license-node gen-license-rust
	#! pwsh
	pwsh contrib/skip-if-fresh.ps1 "{{licenses-windows}}" "{{licenses-node-windows}}" "{{licenses-rust-windows}}" contrib/licenses.mjs
	if ($LASTEXITCODE -eq 0) {
		Write-Host "gen-license: up to date"
		exit 0
	}
	$env:LICENSES_NODE = "{{licenses-node-windows}}"
	$env:LICENSES_RUST = "{{licenses-rust-windows}}"
	node contrib/licenses.mjs > "{{licenses-windows}}"
	Write-Host "gen-license: updated"

[windows]
gen-license-node:
	#! pwsh
	New-Item -Force -ItemType directory ("{{licenses-node-windows}}" | Split-Path) | Out-Null
	pwsh contrib/skip-if-fresh.ps1 "{{licenses-node-windows}}" obscura-ui/pnpm-lock.yaml obscura-ui/package.json
	if ($LASTEXITCODE -eq 0) {
		Write-Host "gen-license-node: up to date"
		exit 0
	}
	cd obscura-ui
	pnpm run --silent license-node >"../{{licenses-node-windows}}"
	Write-Host "gen-license-node: updated"

[windows]
gen-license-rust:
	#! pwsh
	pwsh contrib/skip-if-fresh.ps1 "{{licenses-rust-windows}}" rustlib/Cargo.lock rustlib/Cargo.toml rustlib/about.toml
	if ($LASTEXITCODE -eq 0) {
		Write-Host "gen-license-rust: up to date"
		exit 0
	}
	# Pinned: Latest rejects our about.toml (`no-clearly-defined` was removed).
	if (-not (Get-Command "cargo-about" -ErrorAction SilentlyContinue)) { cargo install --locked cargo-about@0.6.4 }
	cd rustlib; cargo-about generate --format=json --fail -o "../{{licenses-rust-windows}}"

fix-message-ids:
	cargo run --manifest-path xtask/Cargo.toml -- fix rustlib
