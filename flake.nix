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
        pkgs = import nixpkgs { inherit overlays system; };
        lib = pkgs.lib;
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rustlib/rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        rustDeps = [ pkgs.cmake ];
        rustDepsArgs = {
          src = ./rustlib;

          strictDeps = true;
          nativeBuildInputs = rustDeps;
        };

        rustArgs = rustDepsArgs // { cargoArtifacts = craneLib.buildDepsOnly rustDepsArgs; };

        rustlibBindgenArgs = {
          # Environment variables for cbindgen, see rustlib/build.rs
          outputs = [ "out" "dev" ]; # Assumes that crane's derivation only has "out"
          OBSCURA_CLIENT_RUSTLIB_CBINDGEN_CONFIG_PATH = ./apple/cbindgen-apple.toml;
          OBSCURA_CLIENT_RUSTLIB_CBINDGEN_OUTPUT_HEADER_PATH = "${placeholder "dev"}/include/libobscuravpn_client.h";
        };

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

        shellFiles = lib.sources.sourceFilesBySuffices ./. [ ".bash" ".sh" ".shellcheckrc" ];

        swiftFiles = lib.sources.sourceFilesBySuffices (lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [ ./.swiftformat apple/client ];
        }) [ ".swift" ".swiftformat" ];
      in rec {
        checks = {
          inherit (packages) licenses rust;

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
            ] ++ rustDeps ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.create-dmg ];

            shellHook = ''
              export OBSCURA_MAGIC_IN_NIX_SHELL=1
            '';
          };

          web = pkgs.mkShellNoCC {
            packages = [ pkgs.just pkgs.nodejs_20 pkgs.pnpm ];

            # This only changes when our dependencies or license config changes and is relatively slow.
            # So build it once and cache it.
            LICENSE_JSON = packages.licenses;
          };
        };

        packages = rec {
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

          rust = craneLib.buildPackage (rustArgs // rustlibBindgenArgs);
        };
      });
}
