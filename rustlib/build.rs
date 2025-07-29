extern crate cbindgen;

use std::env;

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

    // Get the crate directory where our source code lives
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let config = cbindgen::Config::from_file(cbindgen_config_path).expect("Unable to load cbindgen config file");

    // Generate the bindings
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .unwrap_or_else(|e| panic!("cbingen failed to generate bindings: {:?}", e))
        .write_to_file(output_header_path);
}
