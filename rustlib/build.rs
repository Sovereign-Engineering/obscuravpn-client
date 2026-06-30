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

    // Log OBSCURA_VERSION so we can inspect in the build log whether it is set.
    match env::var("OBSCURA_VERSION") {
        Ok(v) => println!("cargo::warning=OBSCURA_VERSION is set to '{v}'"),
        Err(_) => println!("cargo::warning=OBSCURA_VERSION is NOT set"),
    }

    // Get the crate directory where our source code lives
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    #[cfg(target_os = "windows")]
    {
        let dll_src = get_wintun_dll_src(&crate_dir);
        copy_to_bin_dir(&dll_src, "wintun.dll");
        emit_wintun_dll_hash(&dll_src);
        emit_package_family_name();
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
fn emit_package_family_name() {
    // <Identity Name> from SparsePackage.appxmanifest.
    const PACKAGE_NAME: &str = "SovereignEngineering.ObscuraVPN";
    // Signing certificate subject (X.500 distinguished name).
    const PUBLISHER: &str = "CN=Sovereign Engineering Inc., O=Sovereign Engineering Inc., L=New York, S=New York, C=US, SERIALNUMBER=7746810, OID.2.5.4.15=Private Organization, OID.1.3.6.1.4.1.311.60.2.1.2=Delaware, OID.1.3.6.1.4.1.311.60.2.1.3=US";

    let publisher_id = crockford_publisher_id(PUBLISHER);
    println!("cargo:rustc-env=OBSCURA_PACKAGE_FAMILY_NAME={PACKAGE_NAME}_{publisher_id}");
}

/// MSIX publisherId: the first 8 bytes of SHA-256(Publisher encoded as UTF-16LE), read as 65 bits
/// (the 64 hash bits plus a trailing zero bit) and emitted as 13 Crockford base32 characters.
#[cfg(target_os = "windows")]
fn crockford_publisher_id(publisher: &str) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let utf16le: Vec<u8> = publisher.encode_utf16().flat_map(|unit| unit.to_le_bytes()).collect();
    let digest = ring::digest::digest(&ring::digest::SHA256, &utf16le);
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest.as_ref()[..8]);
    let bits = u128::from(u64::from_be_bytes(head)) << 1; // 65 bits, trailing zero
    (0..13)
        .map(|i| char::from(ALPHABET[usize::try_from((bits >> (60 - 5 * i)) & 0x1f).expect("masked to 5 bits")]))
        .collect()
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
