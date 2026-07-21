// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Validate an OpenVMM release tag and its commit topology.

use anyhow::Context;
use flowey::node::prelude::*;

const RELEASE_TAG_PREFIX: &str = "openvmm-v";

#[derive(Debug, PartialEq, Eq)]
struct ReleaseVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl ReleaseVersion {
    fn parse_tag(tag: &str) -> anyhow::Result<Self> {
        let version = tag.strip_prefix(RELEASE_TAG_PREFIX).with_context(|| {
            format!("OpenVMM release tag must start with {RELEASE_TAG_PREFIX:?}, got {tag:?}")
        })?;
        let components = version.split('.').collect::<Vec<_>>();
        let [major, minor, patch] = components.as_slice() else {
            anyhow::bail!(
                "OpenVMM release version must contain exactly three components, got {version:?}"
            );
        };
        let parse = |name: &str, component: &str| -> anyhow::Result<u16> {
            if component.len() > 1 && component.starts_with('0') {
                anyhow::bail!("OpenVMM release {name} component is not canonical: {component:?}");
            }
            component.parse().with_context(|| {
                format!(
                    "OpenVMM release {name} component must be an unsigned 16-bit integer: \
                     {component:?}"
                )
            })
        };
        Ok(Self {
            major: parse("major", major)?,
            minor: parse("minor", minor)?,
            patch: parse("patch", patch)?,
        })
    }

    fn previous_patch_tag(&self) -> Option<String> {
        self.patch
            .checked_sub(1)
            .map(|patch| format!("{RELEASE_TAG_PREFIX}{}.{}.{patch}", self.major, self.minor))
    }
}

flowey_request! {
    pub struct Request {
        /// Whether a non-tag GitHub ref is an error.
        pub require_tag: bool,
        pub done: WriteVar<SideEffect>,
    }
}

new_simple_flow_node!(struct Node);

impl SimpleFlowNode for Node {
    type Request = Request;

    fn imports(ctx: &mut ImportCtx<'_>) {
        ctx.import::<crate::git_checkout_openvmm_repo::Node>();
    }

    fn process_request(request: Self::Request, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        let Request { require_tag, done } = request;
        let openvmm_repo_path = ctx.reqv(crate::git_checkout_openvmm_repo::req::GetRepoDir);

        ctx.emit_rust_step("validate OpenVMM release tag", |ctx| {
            done.claim(ctx);
            let openvmm_repo_path = openvmm_repo_path.claim(ctx);
            move |rt| {
                let ref_type =
                    std::env::var("GITHUB_REF_TYPE").context("GITHUB_REF_TYPE is not available")?;
                if ref_type != "tag" {
                    if require_tag {
                        anyhow::bail!("OpenVMM releases must run from a Git tag");
                    }
                    return Ok(());
                }

                let tag =
                    std::env::var("GITHUB_REF_NAME").context("GITHUB_REF_NAME is not available")?;
                let version = ReleaseVersion::parse_tag(&tag)?;
                let openvmm_repo_path = rt.read(openvmm_repo_path);
                rt.sh.change_dir(openvmm_repo_path);

                let shallow =
                    flowey::shell_cmd!(rt, "git rev-parse --is-shallow-repository").read()?;
                if shallow == "true" {
                    flowey::shell_cmd!(rt, "git fetch --force --tags --unshallow origin")
                        .run()
                        .context("failed to fetch full OpenVMM release history and tags")?;
                } else {
                    flowey::shell_cmd!(rt, "git fetch --force --tags origin")
                        .run()
                        .context("failed to fetch OpenVMM release tags")?;
                }
                flowey::shell_cmd!(rt, "git fetch --force origin main:refs/remotes/origin/main")
                    .run()
                    .context("failed to fetch the current OpenVMM main branch")?;

                let head = flowey::shell_cmd!(rt, "git rev-parse HEAD").read()?;
                let tag_commit = flowey::shell_cmd!(rt, "git rev-list -n 1 {tag}")
                    .read()
                    .with_context(|| format!("failed to resolve release tag {tag:?}"))?;
                if tag_commit != head {
                    anyhow::bail!(
                        "OpenVMM release tag {tag:?} resolves to {tag_commit}, but the workflow \
                         checked out {head}"
                    );
                }

                let release_tags =
                    flowey::shell_cmd!(rt, "git tag --points-at HEAD --list {RELEASE_TAG_PREFIX}*")
                        .read()?
                        .lines()
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                if release_tags.as_slice() != [tag.as_str()] {
                    anyhow::bail!(
                        "expected exactly release tag {tag:?} at HEAD; found {release_tags:?}"
                    );
                }

                if let Some(previous_tag) = version.previous_patch_tag() {
                    let previous_commit =
                        flowey::shell_cmd!(rt, "git rev-list -n 1 {previous_tag}")
                            .read()
                            .with_context(|| {
                                format!(
                                    "patch release {tag:?} requires immediate predecessor tag \
                                     {previous_tag:?}"
                                )
                            })?;
                    flowey::shell_cmd!(rt, "git merge-base --is-ancestor {previous_commit} HEAD")
                        .run()
                        .with_context(|| {
                            format!(
                                "patch release {tag:?} must descend from immediate predecessor \
                             {previous_tag:?} ({previous_commit})"
                            )
                        })?;
                } else {
                    flowey::shell_cmd!(rt, "git merge-base --is-ancestor HEAD origin/main")
                        .run()
                        .with_context(|| {
                            format!(
                                "normal release {tag:?} must point to a commit reachable from main"
                            )
                        })?;
                }

                Ok(())
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_release_tags() {
        assert_eq!(
            ReleaseVersion::parse_tag("openvmm-v0.12.3").unwrap(),
            ReleaseVersion {
                major: 0,
                minor: 12,
                patch: 3,
            }
        );
    }

    #[test]
    fn rejects_noncanonical_release_tags() {
        for tag in [
            "v0.1.0",
            "openvmm-v0.1",
            "openvmm-v0.1.0.0",
            "openvmm-v0.01.0",
            "openvmm-v0.1.0-rc.1",
            "openvmm-v0.1.0+build",
        ] {
            assert!(ReleaseVersion::parse_tag(tag).is_err(), "{tag}");
        }
    }

    #[test]
    fn identifies_immediate_patch_predecessor() {
        assert_eq!(
            ReleaseVersion::parse_tag("openvmm-v0.12.3")
                .unwrap()
                .previous_patch_tag()
                .as_deref(),
            Some("openvmm-v0.12.2")
        );
        assert_eq!(
            ReleaseVersion::parse_tag("openvmm-v0.12.0")
                .unwrap()
                .previous_patch_tag(),
            None
        );
    }
}
