// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Validate and assemble the source assets for an OpenVMM release.

use crate::assemble_openvmm_source_release::SourceReleaseOutput;
use flowey::node::prelude::*;

fn stable_version(version: &str) -> anyhow::Result<(u64, u64, u64)> {
    let components = version
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>();
    let Ok(components) = components else {
        anyhow::bail!("{version:?} is not a stable MAJOR.MINOR.PATCH version");
    };
    let [major, minor, patch] = components.as_slice() else {
        anyhow::bail!("{version:?} is not a stable MAJOR.MINOR.PATCH version");
    };
    if version != format!("{major}.{minor}.{patch}") {
        anyhow::bail!("{version:?} is not a canonical MAJOR.MINOR.PATCH version");
    }
    Ok((*major, *minor, *patch))
}

flowey_request! {
    pub struct Request {
        pub release: WriteVar<SourceReleaseOutput>,
    }
}

new_simple_flow_node!(struct Node);

impl SimpleFlowNode for Node {
    type Request = Request;

    fn imports(ctx: &mut ImportCtx<'_>) {
        ctx.import::<crate::assemble_openvmm_source_release::Node>();
        ctx.import::<crate::git_checkout_openvmm_repo::Node>();
    }

    fn process_request(request: Self::Request, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        let Request { release } = request;

        let openvmm_repo_path = ctx.reqv(crate::git_checkout_openvmm_repo::req::GetRepoDir);
        let resolved = ctx.emit_rust_stepv("resolve OpenVMM release identity", |ctx| {
            let openvmm_repo_path = openvmm_repo_path.claim(ctx);
            move |rt| {
                let assets = std::env::current_dir()?.join("openvmm-source-release");
                let path = rt.read(openvmm_repo_path);
                rt.sh.change_dir(&path);

                let identity = crate::assemble_openvmm_source_release::resolve_identity(rt)?;
                let version = stable_version(&identity.version)?;

                let shallow =
                    flowey::shell_cmd!(rt, "git rev-parse --is-shallow-repository").read()?;
                if shallow.trim() == "true" {
                    flowey::shell_cmd!(
                        rt,
                        "git fetch --no-tags --unshallow origin +refs/heads/main:refs/remotes/origin/main"
                    )
                    .run()?;
                } else {
                    flowey::shell_cmd!(
                        rt,
                        "git fetch --no-tags origin +refs/heads/main:refs/remotes/origin/main"
                    )
                    .run()?;
                }
                let reachable = std::process::Command::new("git")
                    .args(["merge-base", "--is-ancestor", "HEAD", "origin/main"])
                    .current_dir(&path)
                    .status()
                    .context("failed to check whether the release commit is on main")?;
                if !reachable.success() {
                    anyhow::bail!(
                        "{} is not reachable from origin/main; OpenVMM releases must come from \
                         mainline history",
                        identity.revision
                    );
                }

                let existing =
                    flowey::shell_cmd!(rt, "git ls-remote --tags origin refs/tags/openvmm-v*")
                        .read()?;
                let latest = existing
                    .lines()
                    .filter_map(|line| line.split_whitespace().nth(1))
                    .filter_map(|name| name.strip_prefix("refs/tags/openvmm-v"))
                    .filter_map(|name| name.strip_suffix("^{}").or(Some(name)))
                    .filter_map(|name| stable_version(name).ok())
                    .max();
                if let Some(latest) = latest
                    && version <= latest
                {
                    anyhow::bail!(
                        "OpenVMM {} is not newer than the latest released version {}.{}.{}",
                        identity.version,
                        latest.0,
                        latest.1,
                        latest.2
                    );
                }

                let tag = identity.release_tag();
                let existing_tag =
                    flowey::shell_cmd!(rt, "git ls-remote --tags origin refs/tags/{tag}").read()?;
                if !existing_tag.trim().is_empty() {
                    anyhow::bail!("{tag} already exists; releasing again would redefine it");
                }

                Ok((identity, assets))
            }
        });

        let identity = resolved.clone().map(ctx, |(identity, _)| identity);
        let assets = resolved.clone().map(ctx, |(_, assets)| assets);
        let assembled = ctx.reqv(|done| crate::assemble_openvmm_source_release::Request {
            identity,
            output_dir: assets,
            done,
        });

        ctx.emit_rust_step("publish assembled OpenVMM release artifact", |ctx| {
            assembled.claim(ctx);
            let resolved = resolved.claim(ctx);
            let release = release.claim(ctx);
            move |rt| {
                let (_, assets) = rt.read(resolved);
                rt.write(release, &SourceReleaseOutput { assets });
                Ok(())
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::stable_version;

    #[test]
    fn accepts_only_stable_three_part_versions() {
        assert_eq!(stable_version("1.2.3").unwrap(), (1, 2, 3));
        for invalid in ["1.2", "1.2.3.4", "1.2.3-dev", "v1.2.3", "1.02.3"] {
            assert!(stable_version(invalid).is_err(), "{invalid}");
        }
    }
}
