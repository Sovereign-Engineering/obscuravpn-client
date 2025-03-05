extern crate cbindgen;

use std::env;

const OUTPUT_HEADER_PATH_ENVVAR: &str = "OBSCURA_CLIENT_RUSTLIB_CBINDGEN_OUTPUT_HEADER_PATH";
const CBINDGEN_CONFIG_PATH_ENVVAR: &str = "OBSCURA_CLIENT_RUSTLIB_CBINDGEN_CONFIG_PATH";

fn main() {
    // Tell cargo to re-run this script if the config file changes
    println!("cargo:rerun-if-env-changed={}", CBINDGEN_CONFIG_PATH_ENVVAR);
    println!("cargo:rerun-if-env-changed={}", OUTPUT_HEADER_PATH_ENVVAR);

    // Use var_os instead of var to isolate env var presence from Unicode parsing
    let Some(cbindgen_config_path) = env::var_os(CBINDGEN_CONFIG_PATH_ENVVAR) else {
        println!(
            "cargo::warning=NOT generating bindings! Environment variable '{}' not set",
            CBINDGEN_CONFIG_PATH_ENVVAR
        );
        return;
    };
    println!("cargo:rerun-if-changed={}", cbindgen_config_path.clone().into_string().unwrap());

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
