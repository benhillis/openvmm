// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Validate that an OpenVMM release tag matches the product version.

use anyhow::Context;
use flowey::node::prelude::*;

flowey_request! {
    pub struct Request {
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
        let Request { done } = request;
        let openvmm_repo_path = ctx.reqv(crate::git_checkout_openvmm_repo::req::GetRepoDir);

        ctx.emit_rust_step("validate OpenVMM release tag", |ctx| {
            done.claim(ctx);
            let openvmm_repo_path = openvmm_repo_path.claim(ctx);
            |rt| {
                if std::env::var("GITHUB_REF_TYPE")
                    .context("GITHUB_REF_TYPE is not available")?
                    != "tag"
                {
                    return Ok(());
                }

                let tag = std::env::var("GITHUB_REF_NAME")
                    .context("GITHUB_REF_NAME is not available")?;
                let version_path = rt.read(openvmm_repo_path).join("openvmm/VERSION");
                let version = fs_err::read_to_string(&version_path)
                    .with_context(|| format!("failed to read {}", version_path.display()))?;
                let expected_tag = format!("openvmm-v{}", version.trim());

                if tag != expected_tag {
                    anyhow::bail!(
                        "release tag {tag:?} does not match OpenVMM product version; expected {expected_tag:?}"
                    );
                }

                Ok(())
            }
        });

        Ok(())
    }
}
