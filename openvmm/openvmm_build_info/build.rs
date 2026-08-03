// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![expect(missing_docs)]

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

mod version;

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|output| output.trim().to_owned())
}

fn git_path(repo: &Path, name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(git(repo, &["rev-parse", "--git-path", name])?);
    Some(if path.is_absolute() {
        path
    } else {
        repo.join(path)
    })
}

fn collect_git_source(repo: &Path) -> Option<version::GitSource> {
    // Git searches parent directories. Reject one so an extracted archive
    // nested in an unrelated checkout cannot inherit that repository's HEAD.
    let actual_root = PathBuf::from(git(repo, &["rev-parse", "--show-toplevel"])?);
    // Canonicalization is intentional here: Git and Cargo may reach the same
    // checkout through paths with different symlink components.
    #[expect(clippy::disallowed_methods)]
    if std::fs::canonicalize(actual_root).ok()? != std::fs::canonicalize(repo).ok()? {
        return None;
    }

    let revision = git(repo, &["rev-parse", "HEAD"])?;
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        panic!("git returned an invalid OpenVMM revision: {revision:?}");
    }

    let tags = git(
        repo,
        &["tag", "--points-at", "HEAD", "--list", "openvmm-v*"],
    )?
    .lines()
    .map(str::to_owned)
    .collect();

    Some(version::GitSource { revision, tags })
}

fn watch_git_identity(repo: &Path) {
    for name in ["HEAD", "packed-refs", "refs/tags"] {
        if let Some(path) = git_path(repo, name) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    if let Some(head_ref) = git(repo, &["symbolic-ref", "HEAD"])
        && let Some(path) = git_path(repo, &head_ref)
    {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=version.rs");
    println!("cargo:rerun-if-env-changed=OPENVMM_PKGVERSION");

    let product_version = env!("CARGO_PKG_VERSION");
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let git = collect_git_source(&repo_root);
    if git.is_some() {
        watch_git_identity(&repo_root);
    }

    let package_version = std::env::var("OPENVMM_PKGVERSION").ok();
    let version = version::resolve_version(product_version, package_version.as_deref(), git);
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".into());
    let revision = if version.revision.is_empty() {
        "(not built from a checkout)"
    } else {
        &version.revision
    };
    let long_version = format!(
        "{}\n\
         build:   {}\n\
         version: {product_version}\n\
         commit:  {revision}\n\
         host:    {target}",
        version.version,
        version.kind.description(),
    );

    println!("cargo:rustc-env=OPENVMM_VERSION={}", version.version);
    println!("cargo:rustc-env=OPENVMM_PRODUCT_VERSION={product_version}");
    println!("cargo:rustc-env=OPENVMM_BUILD_KIND={}", version.kind.name());
    println!("cargo:rustc-env=OPENVMM_TARGET={target}");
    println!("cargo:rustc-env=OPENVMM_REVISION={}", version.revision);

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    std::fs::write(out_dir.join("long_version.txt"), long_version)
        .expect("failed to write long version");
}
