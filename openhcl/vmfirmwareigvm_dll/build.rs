// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! A pure-Rust build system for building [Resource-only DLL] files containing
//! OpenHCL IGVM files.
//!
//! This DLL is used when packaging up "production" OpenHCL builds, such as
//! those that get shipped out to Azure.
//!
//! The primary benefit of packaging IGVM files into these resource DLLs is that
//! the resulting DLL files can be digitally signed, to ensure machines in
//! production are running verified builds of OpenHCL.
//!
//! # Building
//!
//! > NOTE: it is highly unlikely that you'll need to build this crate manually.
//! > Check the Guide for the most up-to-date guidance on what pipelines / tools
//! > can be used to generate vmfirmewareigvm.dll files.
//!
//! > WARNING: this crate will _not_ automatically sign resulting DLL files!
//!
//! In order to build this crate, several environment variables must be set.
//! These environment variables control the details of what metadata gets set in
//! the resulting DLL file, as well as what IGVM file gets included.
//!
//! For a detailed breakdown of what each environment variable does - see the
//! inline comments in the code below.
//!
//! Once those environment variables are set, a standard `cargo build -p
//! vmfirmwareigvm_dll` invocation should be sufficient to build the DLL. This
//! assumes you're running on Windows (or have Windows cross-compile set up).
//!
//! The resulting DLL will be emitted in the standard Rust output directory
//! (i.e: under target/...), and will be named `vmfirmwareigvm_dll.dll`.
//!
//! > NOTE: The "double-dll" naming is _not_ a bug, and is a natural consequence
//! > of how cargo names output artifacts according the the name of the crate.
//!
//! [Resource-only DLL]:
//!     https://learn.microsoft.com/en-us/cpp/build/creating-a-resource-only-dll?view=msvc-170

fn main() {
    if cfg!(feature = "ci") {
        return;
    }

    println!("cargo:rerun-if-env-changed=UH_DLL_NAME");
    println!("cargo:rerun-if-env-changed=UH_IGVM_PATH");

    // If none of our env vars are set, do nothing instead of erroring.
    if std::env::var_os("UH_DLL_NAME").is_none()
        && std::env::var_os("UH_IGVM_PATH").is_none()
        && std::env::var_os("UH_MAJOR").is_none()
        && std::env::var_os("UH_MINOR").is_none()
        && std::env::var_os("UH_PATCH").is_none()
        && std::env::var_os("UH_REVISION").is_none()
    {
        println!(
            "cargo::warning=Attempted to build without setting UH_IGVM_PATH - resulting DLL will be empty!"
        );
        return;
    }

    // (string) corresponding the _internal_ DLL name reported by the DLL.
    //
    // (this does not correspond to the name of the DLL file emitted by cargo)
    let uh_dll_name = std::env::var("UH_DLL_NAME").expect("must set UH_DLL_NAME");

    // (path) absolute path to an IGVM file to package up
    let uh_igvm_path = std::env::var("UH_IGVM_PATH").expect("must set UH_IGVM_PATH");

    assert!(std::path::Path::new(&uh_igvm_path).exists());

    // workaround for the fact that hvlite's root-level `.cargo/config.toml`
    // currently sets a bunch of extraneous linker flags via
    //
    // [target.'cfg(all(windows, target_env = "msvc"))']
    if option_env!("RUSTFLAGS")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(true)
    {
        panic!("must compile with RUSTFLAGS=\"\"")
    }

    // Handle version information using the shared utility
    if let Some(version_config) = dll_version::DllVersionConfig::from_env(
        "UH",
        "Microsoft VM HCL IGVM Firmware Resources",
        &uh_dll_name,
        &uh_dll_name,
        "Microsoft VM HCL",
    ) {
        // Add the IGVM resource to the standard version resources
        let additional_content = format!("1 VMFW \"{}\"", uh_igvm_path);

        version_config
            .embed_version_info(Some(&additional_content))
            .expect("Failed to embed version information");
    } else if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // If no version info but we're building a Windows DLL,
        // we still need to add the IGVM resource
        println!("cargo:rustc-link-arg=/NOENTRY"); // resource DLL
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-changed=resources.rc");

        let resources_content = format!("2 24 \"manifest.xml\"\n1 VMFW \"{}\"", uh_igvm_path);
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let resources_rc_path = out_dir.join("resources.rc");
        std::fs::write(&resources_rc_path, resources_content)
            .expect("Failed to write resources.rc");

        // Create a minimal manifest if it doesn't exist
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let manifest_path = manifest_dir.join("manifest.xml");
        if !manifest_path.exists() {
            let manifest_out_path = out_dir.join("manifest.xml");
            std::fs::write(
                &manifest_out_path,
                dll_version::DllVersionConfig::generate_manifest_content(),
            )
            .expect("Failed to write manifest.xml");
        }

        embed_resource::compile(&resources_rc_path, std::iter::empty::<String>())
            .manifest_required()
            .expect("Failed to compile resources");
    }
}
