{
  inputs = {
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { crane, flake-utils, nixpkgs, rust-overlay, self, }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit overlays system;

          config = {
            allowUnfree = true; # sadly, for Android
            android_sdk.accept_license = true;
          };
        };

        lib = pkgs.lib;

        androidBuildToolsVersion = "36.0.0";
        androidCmakeVersion = "3.31.6";
        android = pkgs.androidenv.composeAndroidPackages {
          toolsVersion = "26.1.1"; # frozen legacy version
          platformToolsVersion = "36.0.0";

          platformVersions = [ "36" ];
          buildToolsVersions = [ androidBuildToolsVersion ];

          includeEmulator = false;
          includeSources = false;

          cmakeVersions = [ androidCmakeVersion ];

          includeNDK = true;
          ndkVersion = "26.3.11579264";

          useGoogleAPIs = true;
          useGoogleTVAddOns = false;

          includeExtras = [ "extras;google;google_play_services" ];
        };
        androidBuildTools = "${android.androidsdk}/libexec/android-sdk/build-tools/${androidBuildToolsVersion}";
        androidGradleEnv = { ANDROID_HOME = "${android.androidsdk}/libexec/android-sdk"; };
        androidRustEnv = { ANDROID_NDK_ROOT = "${android.ndk-bundle}/libexec/android-sdk/ndk-bundle"; };
        gradleFlags = [ "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidBuildTools}/aapt2" ];

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rustlib/rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        rustDepsArgs = {
          src = ./rustlib;

          strictDeps = true;
          nativeBuildInputs = [ pkgs.cmake ];
        };
        rustDepsArgs-android = rustDepsArgs // androidRustEnv // {
          buildInputs = [ android.androidsdk ];
          nativeBuildInputs = rustDepsArgs.nativeBuildInputs ++ [ pkgs.cargo-ndk ];
          CARGO_BUILD_TARGET = "aarch64-linux-android";
          doCheck = false;

          # TODO: Long-term it is probably better to just configure the environment ourselves using nixpkgs's standard cross-compilation framework. Right now this is a weird state where we are "secretly" cross-compiling.
          cargoBuildCommand = "cargo ndk -t arm64-v8a build --release";
          cargoCheckCommand = "cargo ndk -t arm64-v8a check --release";
        };

        rustArgs = rustDepsArgs // { cargoArtifacts = craneLib.buildDepsOnly rustDepsArgs; };
        rustArgs-android = rustDepsArgs-android // { cargoArtifacts = craneLib.buildDepsOnly rustDepsArgs-android; };

        rustlibBindgenArgs = {
          # Environment variables for cbindgen, see rustlib/build.rs
          outputs = [ "out" "dev" ]; # Assumes that crane's derivation only has "out"
          OBSCURA_CLIENT_RUSTLIB_CBINDGEN_CONFIG_PATH = ./apple/cbindgen-apple.toml;
          OBSCURA_CLIENT_RUSTLIB_CBINDGEN_OUTPUT_HEADER_PATH = "${placeholder "dev"}/include/libobscuravpn_client.h";
        };

        rust = craneLib.buildPackage (rustArgs // rustlibBindgenArgs);
        rust-android = craneLib.buildPackage (rustArgs-android // rustlibBindgenArgs);

        nodeModules = pkgs.importNpmLock.buildNodeModules {
          npmRoot = ./obscura-ui;
          nodejs = pkgs.nodejs;
        };

        nodeDerivation = { name, nativeBuildInputs ? [ ], preBuildPhases ? [ ], ... }@args:
          pkgs.stdenv.mkDerivation (args // {
            name = "obscuravpn-client-${name}";

            nativeBuildInputs = nativeBuildInputs ++ [ pkgs.nodejs ];

            preBuildPhases = [ "preBuildNodeDerivation" ] ++ preBuildPhases;
            preBuildNodeDerivation = ''
              ln -s ${nodeModules}/node_modules .
              export PATH="${nodeModules}/node_modules/.bin/:$PATH"
            '';
          });

        licenses = pkgs.runCommand "licenses.json" {
          nativeBuildInputs = [ pkgs.nodejs ];

          LICENSES_NODE = licenses-node;
          LICENSES_RUST = licenses-rust;
        } ''
          node ${contrib/licenses.mjs} >"$out"
        '';

        licenses-node = nodeDerivation {
          name = "licenses-node.json";

          nativeBuildInputs = [ pkgs.pnpm ];

          src = (lib.fileset.toSource {
            root = ./obscura-ui;
            fileset = lib.fileset.unions [ ./obscura-ui/package.json ./obscura-ui/package-lock.json ];
          });

          buildPhase = ''
            license-checker \
              --start ${nodeModules} \
              --onlyAllow '0BSD;Apache-2.0;BSD-2-Clause;BSD-3-Clause;CC0-1.0;CC-BY-3.0;CC-BY-4.0;ISC;MIT;OFL-1.1;Python-2.0' \
              --excludePrivatePackages \
              --unknown \
              --json \
              >"$out"
          '';
        };

        licenses-rust = craneLib.mkCargoDerivation (rustArgs // {
          name = "licenses-rust.json";
          nativeBuildInputs = [ pkgs.cargo-about ];
          src = ./rustlib;
          buildPhaseCargoCommand = ''
            cargo-about generate --format=json --fail >"$out"
          '';
          installPhase = " ";
        });

        mkWeb = platform:
          nodeDerivation {
            name = "web-${platform}";

            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [ ./apple/client/Assets.xcassets ./obscura-ui ];
            };

            LICENSE_JSON = licenses;
            OBS_WEB_PLATFORM = platform;

            buildPhase = ''
              pushd obscura-ui

              npm run build

              popd
            '';

            installPhase = ''
              mv obscura-ui/build $out
            '';
          };

        web-android = mkWeb "android";
        web-ios = mkWeb "iphoneos";
        web-macos = mkWeb "macosx";

        # https://nixos.org/manual/nixpkgs/stable/#gradle
        apks = pkgs.stdenv.mkDerivation (finalAttrs:
          androidGradleEnv // {
            name = "obscura-apks";

            src = (lib.fileset.toSource {
              root = ./android;
              fileset = lib.fileset.unions [
                android/app/build.gradle.kts
                android/app/proguard-rules.pro
                android/app/src
                android/build.gradle.kts
                android/gradle.properties
                android/gradle/libs.versions.toml
                android/settings.gradle.kts
              ];
            });

            nativeBuildInputs = [ pkgs.gradle ];

            mitmCache = pkgs.gradle.fetchDeps {
              pkg = finalAttrs.finalPackage;
              data = android/deps.json;
            };

            ANDROID_USER_HOME = "/tmp/";
            gradleFlags = gradleFlags;

            patchPhase = ''
              # TODO: Find a cleaner way to pass these inputs that works during dev as well.
              ln -sfv ${rust-android}/lib/libobscuravpn_client.so app/src/main/jniLibs/arm64-v8a/
              ln -sfv ${web-android} app/src/main/assets
            '';

            installPhase = ''
              mkdir $out
              cp -v app/build/outputs/apk/debug/app-debug.apk $out/
              cp -v app/build/outputs/apk/release/app-release-unsigned.apk $out/
            '';

            doCheck = false;
          });

        shellFiles = lib.sources.sourceFilesBySuffices ./. [ ".bash" ".sh" ".shellcheckrc" ];

        swiftFiles = lib.sources.sourceFilesBySuffices (lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [ ./.swiftformat apple/client ];
        }) [ ".swift" ".swiftformat" ];
      in {
        apps = {
          gradle-deps-update = {
            type = "app";
            program = toString apks.mitmCache.updateScript;
          };
        };

        checks = {
          inherit apks licenses rust rust-android web-android web-ios web-macos;

          shellcheck = pkgs.runCommand "shellcheck" { nativeBuildInputs = [ pkgs.shellcheck ]; } ''
            shopt -s globstar
            shellcheck -P ${shellFiles} -- ${shellFiles}/**/*.{bash,sh}
            touch "$out"
          '';

          rustfmt = craneLib.cargoFmt rustArgs;

          swiftformat = pkgs.runCommand "swiftformat" { nativeBuildInputs = [ pkgs.swiftformat ]; } ''
            swiftformat --lint ${swiftFiles}
            touch "$out"
          '';

          typescript = nodeDerivation {
            name = "typescript";

            src = ./obscura-ui;

            buildPhase = ''
              tsc --noEmit
              touch "$out"
            '';
          };

          nixfmt = pkgs.runCommand "nixfmt" { nativeBuildInputs = [ pkgs.nixfmt-classic ]; } ''
            nixfmt --width=120 --check ${self}/*.nix
            touch "$out"
          '';
        } // (lib.optionalAttrs pkgs.stdenv.isDarwin {
          # TODO: Fails due to unused code on non-darwin.
          clippy =
            craneLib.cargoClippy (rustArgs // { cargoClippyExtraArgs = "--all-features --all-targets -- -Dwarnings"; });
        });

        devShells = {
          default = pkgs.mkShellNoCC {
            packages = [
              pkgs.corepack_20
              pkgs.gnused
              pkgs.just
              pkgs.nixfmt-classic
              pkgs.nodejs_20
              pkgs.shellcheck
              pkgs.swiftformat
              rustToolchain.passthru.availableComponents.rustfmt # Just rustfmt, nothing else
            ] ++ rustArgs.nativeBuildInputs ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.create-dmg ];

            shellHook = ''
              export OBSCURA_MAGIC_IN_NIX_SHELL=1
            '';
          };

          web = pkgs.mkShellNoCC {
            packages = [ pkgs.just pkgs.nodejs_20 pkgs.pnpm ];

            # This only changes when our dependencies or license config changes and is relatively slow.
            # So build it once and cache it.
            LICENSE_JSON = licenses;
          };

          android = pkgs.mkShellNoCC (androidGradleEnv // androidRustEnv // {
            buildInputs = [ pkgs.libiconv ] ++ rustArgs-android.buildInputs;
            nativeBuildInputs = [
              android.cmake
              android.emulator
              android.platform-tools
              rustToolchain
              pkgs.gradle
              pkgs.jdk21
              pkgs.just
              pkgs.ninja
              pkgs.nodejs_20
              pkgs.pkg-config
              pkgs.pnpm
            ] ++ rustArgs-android.nativeBuildInputs;

            GRADLE_OPTS = lib.concatStringsSep " " gradleFlags; # Doesn't support spaces.
            JAVA_HOME = pkgs.jdk21.home;

            shellHook = ''
              export PATH="$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:${androidBuildTools}:$PATH"
            '';
          });
        };

        packages = { inherit apks licenses licenses-node licenses-rust rust web-android web-ios web-macos; };
      });
}
