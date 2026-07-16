// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![forbid(unsafe_code)]

//! Build-script helper that emits `BUILD_GIT_SHA` and `BUILD_GIT_BRANCH`
//! cargo environment variables by invoking the `git` CLI.

use std::process::Command;

/// Git source information collected for a build.
pub struct GitInfo {
    sha: String,
    branch: String,
}

impl GitInfo {
    /// The full Git commit hash.
    pub fn sha(&self) -> &str {
        &self.sha
    }

    /// The checked-out Git branch, or `HEAD` for a detached checkout.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Emit this information as Cargo environment variables.
    pub fn emit(&self) {
        println!("cargo:rustc-env=BUILD_GIT_SHA={}", self.sha);
        println!("cargo:rustc-env=BUILD_GIT_BRANCH={}", self.branch);
    }
}

fn git_output(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git").args(args).output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git {:?} failed with code {:?}: {}",
            args,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    Ok(output)
}

fn git_path(args: &[&str]) -> anyhow::Result<std::path::PathBuf> {
    let output = git_output(args)?;
    Ok(std::path::absolute(&output)?)
}

/// Collect Git information for the current checkout.
pub fn collect_git_info() -> anyhow::Result<GitInfo> {
    // Always rerun when HEAD changes (e.g. branch switch).
    let head_path = git_path(&["rev-parse", "--git-path", "HEAD"])?;
    println!("cargo:rerun-if-changed={}", head_path.display());

    // If HEAD is a symbolic ref (i.e. points at a branch), also watch the
    // branch ref file so we rebuild when new commits land on that branch.
    if let Ok(head_ref) = git_output(&["symbolic-ref", "HEAD"]) {
        // e.g. refs/heads/main → .git/refs/heads/main (or the worktree equivalent)
        let ref_path = git_path(&["rev-parse", "--git-path", &head_ref])?;
        println!("cargo:rerun-if-changed={}", ref_path.display());
    }

    let sha = git_output(&["rev-parse", "HEAD"])?;
    let branch = git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?;

    Ok(GitInfo { sha, branch })
}

/// Emit git information as `cargo:rustc-env` variables so they are available via
/// `env!()` / `option_env!()` in the consuming crate.
pub fn emit_git_info() -> anyhow::Result<()> {
    collect_git_info()?.emit();
    Ok(())
}
