{
  inputs = {
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    napalm.inputs.nixpkgs.follows = "nixpkgs";
    napalm.url = "github:nix-community/napalm";
    nixpkgs.url = "nixpkgs/nixos-24.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    swiftformat.url = "github:Sovereign-Engineering/SwiftFormat-nix";
  };

  outputs = { crane, flake-utils, napalm, nixpkgs, rust-overlay, self, swiftformat }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import ./nix/overlays) (import rust-overlay) napalm.overlays.default ];
        pkgs = import nixpkgs { inherit overlays system; };
        lib = pkgs.lib;
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        swiftfmt = swiftformat.packages.${system}.default;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        rustDepsArgs = {
          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [ ./.cargo ./Cargo.lock ./Cargo.toml ./rustfmt.toml ./src ];
          };

          strictDeps = true;

          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.darwin.apple_sdk.frameworks.Security ];
        };

        rustArgs = rustDepsArgs // { cargoArtifacts = craneLib.buildDepsOnly rustDepsArgs; };

        rustlibBindgenArgs = {
          # Environment variables for cbindgen, see rustlib/build.rs
          outputs = [ "out" "dev" ]; # Assumes that crane's derivation only has "out"
          OBSCURA_CLIENT_RUSTLIB_CBINDGEN_CONFIG_PATH = ./apple/cbindgen-apple.toml;
          OBSCURA_CLIENT_RUSTLIB_CBINDGEN_OUTPUT_HEADER_PATH = "${placeholder "dev"}/include/libobscuravpn_client.h";
        };

        nodeModules = pkgs.napalm.buildPackage (lib.fileset.toSource {
          root = ./obscura-ui;
          fileset = lib.fileset.unions [ ./obscura-ui/package.json ./obscura-ui/package-lock.json ];
        }) { };

        shellFiles = lib.sources.sourceFilesBySuffices ./. [ ".bash" ".sh" ".shellcheckrc" ];

        swiftFiles = lib.sources.sourceFilesBySuffices (lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [ ./.swiftformat apple/client ];
        }) [ ".swift" ".swiftformat" ];
      in rec {
        checks = {
          inherit (packages) licenses rust;

          clippy =
            craneLib.cargoClippy (rustArgs // { cargoClippyExtraArgs = "--all-features --all-targets -- -Dwarnings"; });

          shellcheck = pkgs.runCommand "shellcheck" { nativeBuildInputs = [ pkgs.shellcheck ]; } ''
            shopt -s globstar
            shellcheck -P ${shellFiles} -- ${shellFiles}/**/*.{bash,sh}
            touch "$out"
          '';

          swiftformat = pkgs.runCommand "swiftfmt" { nativeBuildInputs = [ swiftfmt ]; } ''
            swiftformat --lint ${swiftFiles}
            touch "$out"
          '';

          rustfmt = craneLib.cargoFmt rustArgs;

          nixfmt = pkgs.runCommand "nixfmt" { nativeBuildInputs = [ pkgs.nixfmt-classic ]; } ''
            nixfmt --width=120 --check ${self}/*.nix
            touch "$out"
          '';
        };

        devShells = {
          default = pkgs.mkShellNoCC {
            packages = [
              pkgs.corepack_20
              pkgs.gnused
              pkgs.just
              pkgs.nixfmt-classic
              pkgs.nodejs_20
              pkgs.shellcheck
              rustToolchain.passthru.availableComponents.rustfmt # Just rustfmt, nothing else
              swiftfmt
            ] ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.create-dmg ];

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

          licenses-node = pkgs.stdenv.mkDerivation {
            name = "licenses-node.json";

            nativeBuildInputs = [ pkgs.nodejs pkgs.pnpm ];

            src = (lib.fileset.toSource {
              root = ./obscura-ui;
              fileset = lib.fileset.unions [ ./obscura-ui/package.json ./obscura-ui/package-lock.json ];
            });

            buildPhase = ''
              ${nodeModules}/_napalm-install/node_modules/.bin/license-checker \
                --start ${nodeModules}/_napalm-install \
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
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./about.toml
                ./Cargo.lock
                ./Cargo.toml
                src/lib.rs # Required for cargo-metadata not to fail.
              ];
            };
            buildPhaseCargoCommand = ''
              cargo-about generate --format=json --fail >"$out"
            '';
            installPhase = " ";
          });

          rust = craneLib.buildPackage (rustArgs // rustlibBindgenArgs);
        };
      });
}

