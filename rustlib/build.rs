extern crate cbindgen;

use std::env;
#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};

const OUTPUT_HEADER_PATH_ENVVAR: &str = "OBSCURA_CLIENT_RUSTLIB_CBINDGEN_OUTPUT_HEADER_PATH";
const CBINDGEN_CONFIG_PATH_ENVVAR: &str = "OBSCURA_CLIENT_RUSTLIB_CBINDGEN_CONFIG_PATH";

fn main() {
    // NOTE: DO NOT emit any `cargo:rerun-if-*` instructions.
    //
    //       When there are `cargo:rerun-if-*` instructions, `cargo` relies on these instructions
    //       to be fully accurate for change detection and WILL NOT rerun build scripts if files
    //       not listed in the instructions change.
    //
    //       If there are no `cargo:rerun-if-*` instructions, `cargo` will "always re-running the
    //       build script if any file within the package is changed (or the list of files
    //       controlled by the exclude and include fields)". Which is what we want for `cbindgen`.
    //
    //       Also note that `cbindgen` itself does not emit any `cargo:rerun-if-*` instructions.
    //
    //       Source: https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection

    // Get the crate directory where our source code lives
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    #[cfg(target_os = "windows")]
    {
        let dll_src = get_wintun_dll_src(&crate_dir);
        copy_to_bin_dir(&dll_src, "wintun.dll");
        emit_wintun_dll_hash(&dll_src);
    }

    // Use var_os instead of var to isolate env var presence from Unicode parsing
    let Some(cbindgen_config_path) = env::var_os(CBINDGEN_CONFIG_PATH_ENVVAR) else {
        println!(
            "cargo::warning=NOT generating bindings! Environment variable '{}' not set",
            CBINDGEN_CONFIG_PATH_ENVVAR
        );
        return;
    };

    let Some(output_header_path) = env::var_os(OUTPUT_HEADER_PATH_ENVVAR) else {
        println!(
            "cargo::warning=NOT generating bindings! Environment variable '{}' not set",
            OUTPUT_HEADER_PATH_ENVVAR
        );
        return;
    };

    let config = cbindgen::Config::from_file(cbindgen_config_path).expect("Unable to load cbindgen config file");

    // Generate the bindings
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .unwrap_or_else(|e| panic!("cbingen failed to generate bindings: {e:?}"))
        .write_to_file(output_header_path);
}

#[cfg(target_os = "windows")]
fn copy_to_bin_dir(src: &Path, file_name: &str) {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let binary_dir = std::path::Path::new(&out_dir)
        .ancestors()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(profile.as_str()))
        .expect("could not find target binary dir (debug/release) in OUT_DIR ancestors");
    let dst = binary_dir.join(file_name);

    std::fs::copy(src, &dst).unwrap_or_else(|e| {
        panic!("Failed to copy {src:?} to {dst:?}: {e}");
    });
}

/// SECURITY: Calculate the SHA-256 hash of the wintun.dll at build time and expose it as a
/// compile-time environment variable `WINTUN_DLL_SHA256`. This allows the runtime code to verify
/// the DLL's integrity before loading it, protecting against DLL replacement attacks.
#[cfg(target_os = "windows")]
fn emit_wintun_dll_hash(dll_path: &Path) {
    let dll_bytes = std::fs::read(dll_path).unwrap_or_else(|e| panic!("Failed to read {dll_path:?} for hashing: {e}"));
    let hash = ring::digest::digest(&ring::digest::SHA256, &dll_bytes);
    let hash_hex = hash.as_ref().iter().map(|b| format!("{b:02x}")).collect::<String>();

    println!("cargo:rustc-env=WINTUN_DLL_SHA256={hash_hex}");
}

#[cfg(target_os = "windows")]
const WINTUN_VERSION: &str = "0.14.1";

#[cfg(target_os = "windows")]
fn get_wintun_dll_src(manifest_dir: &String) -> PathBuf {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
        .or_else(|_| env::var("TARGET"))
        .unwrap_or_else(|_| std::env::consts::ARCH.to_string());
    let arch = match target_arch.as_str() {
        "x86" => "x86",
        "x86_64" => "amd64",
        "arm" => "arm",
        "aarch64" => "arm64",
        arch => panic!("Unsupported architecture: {arch}"),
    };
    let dll_path = format!("windows/wintun-{WINTUN_VERSION}/bin/{arch}/wintun.dll");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("Manifest directory has no parent")
        .join(dll_path)
}
