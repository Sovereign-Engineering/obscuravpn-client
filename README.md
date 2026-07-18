# Obscura VPN Client

Obscura VPN library, CLI client, and App

## Support

No support is provided for this code directly. However, if you are experiencing issues with your Obscura VPN service please contact <support@obscura.net>.

## Contributions

At this time we are unable to accept external contributions. This is something that we plan to resolve soon. However until we finish the paperwork we are unable to look at any patches and will close all PRs without looking at them.

## macOS App

On macOS the app installs and manages a [network extension](https://developer.apple.com/documentation/networkextension) (system extension).
The network extension manages the virtual device and maintains the tunnel using the Rust code as library.

### Setup

1. [Setup Nix](#nix-setup)
1. Install dependencies: `nix-env -iA nixpkgs.{cmake,rustup}`
1. Open the main Xcode project
    ```bash
    nix develop --print-build-logs --command just xcode-open
    ```
1. In Xcode, login with an account with membership in "Sovereign Engineering Inc."
1. Register development machine in Apple Developer portal (can be done in Xcode)
1. [Enable system extension developer mode](#enabling-system-extension-developer-mode)
1. Setup Developer ID provisioning profile and codesigning for `Prod Client` build scheme
    1. Go to https://developer.apple.com/account/resources/profiles/list
        - Download "Developer ID: System Network Extension"
        - Download "Developer ID: VPN Client App"
    1. Install both provisioning profiles by double-clicking them.
    1. Ask Carl to send the Developer ID codesigning certificate and the corresponding password
    1. Double click the certificate, enter the password, and install it to your "login" keychain

## Building and Running

### For macOS and iOS

1. Open the main Xcode project:
    ```bash
    nix develop --print-build-logs --command just xcode-open
    ```
1. Pick a build scheme using Xcode's GUI, one of:

    ℹ️ **INFO**: Xcode differentiates between "build schemes" and "build configurations", see [Apple's docs on this](https://developer.apple.com/documentation/xcode/build-system) for more details.

    1. `Dev Client`: Development Client

        General purpose for development. Uses the main UI with additional developer and pre-release features exposed.

        Uses the `Debug*` build configurations. Codesigned with the `Apple Development` xcode-managed identity.

        ⚠️ **WARNING**: When using this build scheme, make sure you are quitting the app via the top-right status menu bar and **NOT** using Xcode's "Stop" as doing so does not actually stop the dev server. This is because stopping via Xcode doesn't run the build scheme's "Run → Post-actions"

    1. `Prod Client`: The App with a static web bundle

        Useful for reproducing what the final shippable app will look like and be built as.

        Uses the `Release*` build configurations. Codesigned with the `Developer ID Application: Sovereign Engineering Inc. (5G943LR562)` manually-managed identity.

        The static web bundle built with the build scheme's "Build → Pre-actions".

        If you encounter trouble with this build scheme, especially with codesigning or provisioning profiles:

        1. Make sure that you've completed the relevant steps in [setup](#setup)
        1. See additional instructions in [Confirming "Developer ID" Setup](#confirming-developer-id-setup)

    1. `Bare Client`: The App with a minimal HTML UI

        Useful for fine-grain control and debugging.

        Uses the `Debug*` build configurations. Codesigned with the `Apple Development` xcode-managed identity.

1. Build or Run the App

    - `⌘ + B` (Build), or
    - `⌘ + R` (Run)

    💡 **TIP**: It may initially _seem_ like Xcode is doing nothing when you run or build, but it may just be running the build scheme's "Pre-actions", see the "Report navigator" in Xcode's top-left app menu: "View → Navigators → Reports" to track the actual status.

    💡 **TIP**: If a build fails with `could not find included file 'buildversion.xcconfig' in search paths`, see the [relevant troubleshooting entry](#error-on-build-in-clean-repo-could-not-find-included-file-buildversionxcconfig-in-search-paths).

    -----

    Xcode places built products in a deeply nested directory structure that it controls, with seperate folders for each build configuration. The easiest way to locate where the app is:

    1. "Run" the app
    1. Once the app's icon appears on the macOS Dock, `⌘-Click` the app icon to reveal it in the finder.

💡 **TIP**: It is highly recommended to read through various sections in [Development Tips](#development-tips) to better understand the various ways we've configured the Xcode build system to work with our development process.

### For Android

#### Nix Builds

Nix builds provide an easy way to get a fully built APK. They are hermetic and reliable. However, they provide only coarse grained caching so if you are iterating during development you may prefer to use [Incremental Builds](#incremental-builds).

```sh
nix build '.#apks-foss'
apksigner sign --ks your-keystore.jks --ks-pass pass:hunter2 --out=obscura-signed.apk result/app-foss-release-unsigned.apk # Sign.
adb install obscura-signed.apk # Push to your device.
```

Instead of `app-foss-release-unsigned` you can also use `app-foss-debug` for the debug build. Note that just the Android portion is a debug build, the Rust core and UI are still release builds.

#### Incremental Builds

The Android app requires a special build of the Rust library and Obscura UI. These are built using Nix, while the Android app itself can be built using [Android Studio](https://developer.android.com/studio) for local development, or the Gradle build system to create an official build.

1. Build the Obscura UI
   ```bash
   OBS_WEB_PLATFORM="android" nix develop '.#web' --print-build-logs -c just web-bundle-build
   ```
2. Build the Rust library
   ```bash
   nix develop '.#android' --command bash -c 'cd rustlib && cargo ndk -t arm64-v8a build --release'
   ```
3. Open Android Studio and point it at the `android` directory, or
4. Use Gradle to build everything
    ```bash
    nix develop '.#android' --command bash -c 'cd android && gradle --no-daemon $GRADLE_OPTS build'
    ```

In order to iterate you can just repeat the steps. 1 and 2 are only required if you changed the UI or Rust core respectively but the final APK build must always be re-run.

#### Gradle Dependencies

To ensure hermetic builds we pin our Gradle dependencies. If you change the dependencies you will need to regenerate the pin file.

```
bin/gradle-deps-update.sh
```

### For Windows

Install [Visual Studio](https://visualstudio.microsoft.com/downloads/) with the following Workloads:

- Desktop development with C++ (for Rust)
- WinUI application development

Install [Rust](https://rust-lang.org/learn/get-started/).

You may also need to install [Windows App SDK](https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads) manually to get the client app running.

Install [Powershell 7](https://learn.microsoft.com/powershell/scripting/install/install-powershell-on-windows)

On Windows, definitely ARM64 machines, you need to add `C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\Llvm\ARM64\bin` to path.

Download the signed [wintun 0.14.1 DLLs](https://www.wintun.net/).

You can use `Get-FileHash -Path .\wintun-0.14.1.zip -Algorithm SHA256` to verify the hash against `SHA2-256: 07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51`.

Extract to `windows/wintun-0.14.1` such that `windows/wintun-0.14.1/bin/arm64/wintun.dll` is a file.

To test the service, run `cargo build --bin obscura` and then `sudo .\target\debug\obscura.exe service`. You need to enable `sudo` under System > Advanced settings. Alternatively, you can run `.\target\debug\obscura.exe service` in an administrative terminal.

The default config directory is `%APPDATA%\Obscura`. When testing the service, you may find it beneficial to manually add in an account number.

A helpful command to clean DNS query manually is `Remove-DnsClientNrptRule -Name "{fb157da8-6578-4f53-81ea-0a9168e96c1f}"`.

To run the desktop app, you need to install [nvm-windows](https://github.com/coreybutler/nvm-windows/releases) (`winget install nvm-windows`) and then run `nvm install lts && nvm use lts && corepack enable`.

### Tips

The [WinUI 3 Gallery](https://apps.microsoft.com/detail/9p3jfpwwdzrc) app is very useful at showcasing features currently available with code snippets.

If for some reason you don't want to modify `tag.json` to set the version of the app, you can also add the following to manually create the environment variable (assuming non-CLI build).

```csproj
<OBSCURA_VERSION Condition="'$(OBSCURA_VERSION)' == ''">1.156</OBSCURA_VERSION>
```

[Segoe Fluent Icons](https://learn.microsoft.com/windows/apps/design/iconography/segoe-ui-symbol-font#icon-list)

## Swift unit tests

"Swift Testing" tests are placed in `*Test.swift` files, which need to be a member of the `Tests` target. Testing (not running) with the `Tests` scheme builds and executes all tests.

## Debugging

### Logs

Both app and network extension logs are available via [Apple's unified logging system](https://developer.apple.com/documentation/os/logging).

#### Analyzing Logs

There are tools for analyzing logs available as `bin/log-*`. They accept log files in JSON lines format. This can be found in the app's Debug Bundle or from the Apple `log` command by specifying `--style=ndjson`.

The main tool is `bin/log-text.py` which just turns the logs into a readable text format as well as applying some basic filtering with a few CLI options to apply more filters. Other tools are available, run with `--help` to get information about what they do.

For more in-depth analysis you are likely best using the tools as a starting point and modifying them as needed or using other tools like `jq`, `sqlite` or `duckdb`. If your analysis is generally useful consider committing it.

#### Stream Logs

This will output logs starting at the point in time when you run this command:

```bash
log stream --info --debug --predicate 'process CONTAINS[c] "obscura" || subsystem CONTAINS[c] "obscura"'
```

#### View Past Logs

> [!WARNING]
> Since Apple may or may not persist logs at the `INFO` or `DEBUG` level, logs at these level might be lost. See [Apple's developer docs on this](https://developer.apple.com/documentation/os/logging/generating_log_messages_from_your_code#3665947) for more information.
>
> You may be able to set a log configuration to ensure that these logs are persisted, though this has not been tested, please update this `README` with instructions if you successfully test this. See [Apple's docs on "Customizing Logging Behavior While Debugging"](https://developer.apple.com/documentation/os/logging/customizing_logging_behavior_while_debugging) for more information.

```bash
log show --last 200 --info --debug --color always --predicate 'process CONTAINS[c] "obscura" || subsystem CONTAINS[c] "obscura"' | less +G -R
```

#### UserDefaults

```sh
defaults read "net.obscura.vpn-client-app"
# delete all defaults including Sparkle related keys (SU*)
defaults delete-all "net.obscura.vpn-client-app"
# delete keys individually
defaults delete "net.obscura.vpn-client-app" <key>
```

## Running Checks

### Linting

```bash
nix develop --print-build-logs --command just lint
```

### Formatting

#### Checking

```bash
nix flake check
```

#### Auto-fixing

```bash
nix develop --print-build-logs --command just format-fix
```

## Building a Notarized Disk Image

1. Save authentication credentials for the Apple notary service (only need to do once)

    ```bash
    xcrun notarytool store-credentials "notarytool-password" --team-id 5G943LR562
    ```

    Use [appleid.apple.com](https://appleid.apple.com/account/manage) --> App-Specific Passwords

1. (OPTIONAL) If we're doing a release, tag the version `git tag -s v/1.23 -m v/1.23 && git push --tags`.
1. Unlock the "Login" keychain: `security unlock-keychain`
1. Build the signed and notarized disk image: `just build-dmg`

    💡 **TIP**: This command uses AppleScript automation of Finder to change the background of Disk Images, so Finder windows may open.

    The built disk image will appear in the current working directory as "Obscura VPN.dmg"

## Troubleshooting

### `cargo` not rebuilding when it should

A lot of Xcode-set properties don't properly trigger a rebuild from `cargo` even
though they're supposed to. The most prominent of which is `MACOSX_DEPLOYMENT_TARGET`.

This is easily worked-around by "Product → Clean Build Folder..." in Xcode then rerunning the build.

Upstream status on this:
- https://github.com/rust-lang/cc-rs/issues/906
- https://github.com/rust-lang/rust/issues/118204

## Development Tips

### Enabling system extension developer mode

This is necessary for:
- The `systemextensionsctl` commands to work, and
- To allow installing and running system extensions from places other than `/Applications`

According to [Apple's docs for system extensions](https://developer.apple.com/documentation/driverkit/debugging_and_testing_system_extensions#3557204), as of 2024-07-04:

> You must place all system extensions in the `Contents/Library/SystemExtensions` directory of your app bundle, and the app itself must be installed in one of the system’s `Applications` directories. To allow development of your app outside of these directories, use the `systemextensionsctl` command-line tool to enable developer mode. When in developer mode, the system doesn't check the location of your system extension prior to loading it, so you can load it from anywhere in the file system.

To accomplish this:
1. [Disable system integrity protection](https://developer.apple.com/documentation/security/disabling_and_enabling_system_integrity_protection)
1. Then, run
    ```bash
    systemextensionsctl developer on
    ```

### Removing network extension (system extension)

1. Ensure that [system extension developer mode is enabled](#enabling-system-extension-developer-mode)
1. Then, run
    ```bash
    systemextensionsctl uninstall 5G943LR562 net.obscura.vpn-client-app.system-network-extension
    ```

### Nix Setup

- Install [`nix`](https://nixos.org/download/) (only the package manager is needed)
- Enable [`flake`s](https://nixos.wiki/wiki/Flakes)

    Add the following to `~/.config/nix/nix.conf` or `/etc/nix/nix.conf`:

    ```
    experimental-features = nix-command flakes
    ```

- Optional, but strongly recommended: Set up [`nix-direnv`](https://github.com/nix-community/nix-direnv) and integrate it with your preferred shell

  If you do this, you can omit the `nix develop ... --command` parts, as `cd`-ing into the repository directory will set up your environment variables with the correct tools as long as you've `direnv allow`-ed the directory.

### Confirming "Developer ID" Setup

To confirm that the Developer ID provisioning profile and codesigning are set up correctly (required for the `Prod Client` build scheme):

1. Pick the `Prod Client` build scheme in Xcode
1. Create an Archive
    Choose from Xcode's top-left app menu: "Product → Archive"
1. Ensure that the "Archive" action succeeds in the "Report navigator"
    Choose from Xcode's top-left app menu: "View → Navigators → Reports"

## Linux

For local development, build and run any of the binaries (each builds in the same Debian
container as the release):

- `contrib/bin/linux_run_gui.sh`: builds and runs the GUI.
- `contrib/bin/linux_run_cli.sh`: builds and runs the `obscura` CLI, passing its arguments through.
- `contrib/bin/linux_run_service.sh`: builds and runs the `obscura` system service the GUI and CLI talk to.

### Building and signing packages

Build all the packages (`obscura-cli`, `obscura-gui`, `obscura`) and the signed
apt/dnf/pacman repositories:

```bash
./contrib/bin/linux-build-packages.bash
```

It derives the signing key from `linux/signing_keys/current.public.asc` (exporting its secret
from your gpg keyring) and prompts for its passphrase. Publish the three repository trees it
produces, `result-linux/dist-prod/{deb,rpm,arch}`, at `https://linux-pkgs.obscura.com/{deb,rpm,arch}`.
Pass `--test` to build instead with the committed keys in `linux/signing_keys_test/`.

### Signing key rotation

`linux/signing_keys/` holds `current.public.asc` (signs releases) and `next.public.asc`
(the next key, shipped ahead). It also holds `rotate_signing_key.bash`. The directory is
self-contained: copy it to the isolated machine that holds the secret keys, run the script
there, and copy `current.public.asc`, `next.public.asc`, and `revocation.asc` back.

Revoked keys ship in the repositories so already-installed clients stop trusting them. Each format handles revocation differently:

- **deb**: nothing to do beyond dropping the key. `obscura-repository` replaces the keyring
  file wholesale on upgrade, so the dropped key is gone from every client.
- **rpm**: the revoked key is dropped from `RPM-GPG-KEY-obscura` (so new installs never
  trust it) and its fingerprint is listed in `RPM-GPG-KEY-obscura-revoked`.
  `obscura-package-signing-key-refresh.timer` (in `obscura-repository`) removes listed
  keys from the rpm keystore within a day, and imports newly shipped keys, because rpm
  itself never re-reads the key file once a key is imported (not on upgrade, and
  scriptlets cannot import while the transaction lock is held).
- **arch**: the fingerprint is added to `obscura-revoked` and the revoked key to `obscura.gpg`;
  `pacman-key --populate obscura` (run from the keyring package's install hook on every
  upgrade) disables it.
