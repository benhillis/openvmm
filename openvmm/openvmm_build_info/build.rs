// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![expect(missing_docs)]

fn parse_component(name: &str, value: &str) {
    value
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be an unsigned 16-bit integer, got {value:?}"));
}

fn product_version() -> String {
    let version_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../VERSION");
    let version = std::fs::read_to_string(&version_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", version_path.display()));
    let version = version.trim();
    let components = version.split('.').collect::<Vec<_>>();
    let [major, minor, patch] = components.as_slice() else {
        panic!("OpenVMM VERSION must contain exactly three components, got {version:?}");
    };

    parse_component("OpenVMM VERSION major component", major);
    parse_component("OpenVMM VERSION minor component", minor);
    parse_component("OpenVMM VERSION patch component", patch);

    version.to_owned()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../VERSION");

    println!(
        "cargo:rustc-env=OPENVMM_PRODUCT_VERSION={}",
        product_version()
    );

    if let Err(error) = build_rs_git_info::emit_git_info() {
        println!("cargo:warning=failed to collect OpenVMM git build information: {error:#}");
    }
}
