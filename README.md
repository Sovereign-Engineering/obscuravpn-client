# Obscura VPN Client

Obscura VPN library, CLI client, and App

## Support

No support is provided for this code directly. However, if you are experiencing issues with your Obscura VPN service please contact <support@obscura.net>.

## Contributions

At this time we are unable to accept external contributions. This is something that we plan to resolve soon. However until we finish the paperwork we are unable to look at any patches and will close all PRs without looking at them.

# macOS App

On macOS the app installs and manages a [network extension](https://developer.apple.com/documentation/networkextension) (system extension).
The network extension manages the virtual device and maintains the tunnel using the Rust code as library.

## Setup

1. [Install `rustup`](https://rustup.rs/).
1. [Setup Nix](#nix-setup)
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
nix develop --print-build-logs --command just format-check
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

### Overriding the API server URL

1. Start the app at least once and login. A config file will be automatically created at

    `/Library/Application Support/obscura-vpn/system-network-extension/config.json`

2. Quit the app (via the top-right status menu bar so that the app quits fully)
3. Wait for the network extension to stop, or kill it using `sudo kill -9 $(pgrep net.obscura.vpn-client-app.system-network-extension)`
4. Set the `"api_url"` key in the config file
    - prod: `"https://v1.api.prod.obscura.net/api"`
    - localhost: `"http://localhost:12345/api"`
    - staging: `"https://v1.api.staging.obscura.net/api"` (internal only)

    To quickly replace the `api_url`, you can use `./bin/update-config-url.sh http://localhost:12345/api`

5. Start the app again

### Confirming "Developer ID" Setup

To confirm that the Developer ID provisioning profile and codesigning are set up correctly (required for the `Prod Client` build scheme):

1. Pick the `Prod Client` build scheme in Xcode
1. Create an Archive
    Choose from Xcode's top-left app menu: "Product → Archive"
1. Ensure that the "Archive" action succeeds in the "Report navigator"
    Choose from Xcode's top-left app menu: "View → Navigators → Reports"

## Linux

> [!WARNING]
> As of 2024-07-04, the Linux client is not maintained.

```bash
cargo build --release && sudo RUST_LOG=info ./target/release/obscuravpn-client
```
