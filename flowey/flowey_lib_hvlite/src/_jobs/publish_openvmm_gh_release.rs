// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Attest and publish an assembled standalone OpenVMM source release.
//!
//! OpenVMM releases source first. The published assets are the source archive
//! and its checksums; prebuilt binaries are a later phase, and deliberately not
//! part of this pipeline, so releasing does not depend on binary signing.
//!
//! The assets arrive as a workflow artifact after the distribution build job
//! proves those exact bytes buildable.

use crate::assemble_openvmm_source_release::CHECKSUM_FILE;
use crate::assemble_openvmm_source_release::SourceReleaseOutput;
use flowey::node::prelude::*;

flowey_request! {
    pub struct Request {
        pub release: ReadVar<SourceReleaseOutput>,
        pub done: WriteVar<SideEffect>,
    }
}

new_simple_flow_node!(struct Node);

impl SimpleFlowNode for Node {
    type Request = Request;

    fn imports(ctx: &mut ImportCtx<'_>) {
        ctx.import::<flowey_lib_common::attest_build_provenance::Node>();
        ctx.import::<flowey_lib_common::publish_gh_release::Node>();
    }

    fn process_request(request: Self::Request, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        let Request { release, done } = request;

        let resolved = ctx.emit_rust_stepv("read OpenVMM release identity", |ctx| {
            let release = release.claim(ctx);
            move |rt| {
                let release = rt.read(release);
                let identity =
                    crate::assemble_openvmm_source_release::read_source_identity(&release.assets)?;
                Ok((release, identity))
            }
        });

        let files = resolved.clone().map(ctx, |(release, identity)| {
            // Name the assets explicitly rather than globbing the
            // directory, so nothing incidental can end up on the release.
            vec![
                (release.assets.join(identity.archive_name()), None),
                (release.assets.join(CHECKSUM_FILE), None),
            ]
        });

        let target = resolved.clone().map(ctx, |(_, identity)| identity.revision);
        let tag = resolved
            .clone()
            .map(ctx, |(_, identity)| identity.release_tag());
        let title = resolved.map(ctx, |(_, identity)| {
            format!("OpenVMM v{}", identity.version)
        });

        let (attestation_done, write_attestation_done) = ctx.new_var();
        ctx.req(flowey_lib_common::attest_build_provenance::Request {
            files: files.clone(),
            done: write_attestation_done,
        });
        ctx.req(flowey_lib_common::publish_gh_release::Request(
            flowey_lib_common::publish_gh_release::GhReleaseParams {
                repo_owner: "microsoft".into(),
                repo_name: "openvmm".into(),
                target,
                tag,
                title,
                files,
                notes: flowey_lib_common::publish_gh_release::GhReleaseNotes::Generated,
                // Publish as a draft. Releasing is new enough that a human
                // should look at the assembled release before it is public --
                // and GitHub does not create a draft release's tag until it is
                // published, so the irreversible step is genuinely last.
                draft: true,
                // A failed run is safely rerunnable: replace an existing draft
                // from this version, but never alter a published release.
                on_existing: flowey_lib_common::publish_gh_release::OnExistingRelease::ReplaceDraft,
                prerequisites: vec![attestation_done],
                done,
            },
        ));

        Ok(())
    }
}
