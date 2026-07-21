// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Build and publish standalone OpenVMM release artifacts.

use crate::pipelines_shared::gh_pools;
use flowey::node::prelude::FlowPlatformLinuxDistro;
use flowey::node::prelude::GhPermission;
use flowey::node::prelude::GhPermissionValue;
use flowey::pipeline::prelude::*;
use flowey_lib_common::git_checkout::RepoSource;
use flowey_lib_hvlite::_jobs::publish_openvmm_gh_release::OpenvmmReleaseTarget;
use flowey_lib_hvlite::build_openvmm::OpenvmmBuildParams;
use flowey_lib_hvlite::build_openvmm::OpenvmmFeature;
use flowey_lib_hvlite::common::CommonArch;
use flowey_lib_hvlite::common::CommonPlatform;
use flowey_lib_hvlite::common::CommonProfile;
use flowey_lib_hvlite::common::CommonTriple;
use std::collections::BTreeMap;

#[derive(clap::Args)]
pub struct ReleaseOpenvmmCli {}

impl IntoPipeline for ReleaseOpenvmmCli {
    fn into_pipeline(self, backend_hint: PipelineBackendHint) -> anyhow::Result<Pipeline> {
        openvmm_release_pipeline(backend_hint)
    }
}

fn openvmm_release_pipeline(backend_hint: PipelineBackendHint) -> anyhow::Result<Pipeline> {
    if !matches!(backend_hint, PipelineBackendHint::Github) {
        anyhow::bail!("OpenVMM release pipelines only support the GitHub backend");
    }

    let mut pipeline = Pipeline::new();
    pipeline
        .gh_set_ci_triggers(GhCiTriggers {
            tags: vec!["openvmm-v*".into()],
            ..Default::default()
        })
        .gh_set_name("OpenVMM Release")
        .gh_set_flowey_bootstrap_template(
            crate::pipelines_shared::gh_flowey_bootstrap_template::get_template(),
        );

    let cfg_common_params = crate::pipelines_shared::cfg_common_params::get_cfg_common_params(
        &mut pipeline,
        backend_hint,
        None,
    )?;
    pipeline.inject_all_jobs_with(move |job| {
        job.dep_on(&cfg_common_params)
            .dep_on(|_| flowey_lib_hvlite::_jobs::cfg_versions::Request::Init)
            .dep_on(
                |_| flowey_lib_hvlite::_jobs::cfg_hvlite_reposource::Params {
                    hvlite_repo_source: RepoSource::GithubSelf,
                    checkout_depth: None,
                },
            )
            .gh_grant_permissions::<flowey_lib_common::git_checkout::Node>([(
                GhPermission::Contents,
                GhPermissionValue::Read,
            )])
            .gh_grant_permissions::<flowey_lib_common::gh_task_azure_login::Node>([(
                GhPermission::IdToken,
                GhPermissionValue::Write,
            )])
    });

    let validate_tag = pipeline
        .new_job(
            FlowPlatform::Linux(FlowPlatformLinuxDistro::Ubuntu),
            FlowArch::X86_64,
            "validate release tag",
        )
        .gh_set_pool(gh_pools::linux_x64_gh())
        .side_effect(
            |done| flowey_lib_hvlite::_jobs::validate_openvmm_release_tag::Request {
                require_tag: true,
                done,
            },
        )
        .finish();

    let targets = [
        (
            OpenvmmReleaseTarget::WindowsX64,
            CommonTriple::Common {
                arch: CommonArch::X86_64,
                platform: CommonPlatform::WindowsMsvc,
            },
        ),
        (
            OpenvmmReleaseTarget::WindowsArm64,
            CommonTriple::Common {
                arch: CommonArch::Aarch64,
                platform: CommonPlatform::WindowsMsvc,
            },
        ),
        (
            OpenvmmReleaseTarget::LinuxX64,
            CommonTriple::Common {
                arch: CommonArch::X86_64,
                platform: CommonPlatform::LinuxMusl,
            },
        ),
        (
            OpenvmmReleaseTarget::LinuxArm64,
            CommonTriple::Common {
                arch: CommonArch::Aarch64,
                platform: CommonPlatform::LinuxMusl,
            },
        ),
    ];

    let mut release_artifacts = BTreeMap::new();
    for (release_target, target) in targets {
        let label = release_target.label();
        let (publish_openvmm, use_openvmm) =
            pipeline.new_typed_artifact(format!("openvmm-release-{label}"));
        let platform = match release_target {
            OpenvmmReleaseTarget::WindowsX64 | OpenvmmReleaseTarget::WindowsArm64 => {
                FlowPlatform::Windows
            }
            OpenvmmReleaseTarget::LinuxX64 | OpenvmmReleaseTarget::LinuxArm64 => {
                FlowPlatform::Linux(FlowPlatformLinuxDistro::Ubuntu)
            }
        };
        let features = match release_target {
            OpenvmmReleaseTarget::WindowsX64 | OpenvmmReleaseTarget::WindowsArm64 => {
                Default::default()
            }
            OpenvmmReleaseTarget::LinuxX64 | OpenvmmReleaseTarget::LinuxArm64 => {
                [OpenvmmFeature::Tpm].into()
            }
        };
        let params = OpenvmmBuildParams {
            profile: CommonProfile::Release,
            target,
            features,
        };

        let build_job = pipeline
            .new_job(
                platform,
                FlowArch::X86_64,
                format!("build OpenVMM release [{label}]"),
            )
            .gh_set_pool(match release_target {
                OpenvmmReleaseTarget::WindowsX64 | OpenvmmReleaseTarget::WindowsArm64 => {
                    gh_pools::default_windows()
                }
                OpenvmmReleaseTarget::LinuxX64 | OpenvmmReleaseTarget::LinuxArm64 => {
                    gh_pools::default_linux()
                }
            })
            .publish(publish_openvmm, move |openvmm| {
                flowey_lib_hvlite::build_openvmm::Request { params, openvmm }
            })
            .finish();
        pipeline.non_artifact_dep(&build_job, &validate_tag);
        release_artifacts.insert(release_target, use_openvmm);
    }

    pipeline
        .new_job(
            FlowPlatform::Linux(FlowPlatformLinuxDistro::Ubuntu),
            FlowArch::X86_64,
            "publish OpenVMM release",
        )
        .gh_set_pool(gh_pools::linux_x64_gh())
        .gh_grant_permissions::<flowey_lib_common::publish_gh_release::Node>([(
            GhPermission::Contents,
            GhPermissionValue::Write,
        )])
        .gh_grant_permissions::<flowey_lib_common::attest_build_provenance::Node>([
            (GhPermission::Contents, GhPermissionValue::Read),
            (GhPermission::IdToken, GhPermissionValue::Write),
            (GhPermission::Attestations, GhPermissionValue::Write),
        ])
        .dep_on(
            |ctx| flowey_lib_hvlite::_jobs::publish_openvmm_gh_release::Request {
                openvmm: release_artifacts
                    .into_iter()
                    .map(|(target, artifact)| (target, ctx.use_typed_artifact(&artifact)))
                    .collect(),
                done: ctx.new_done_handle(),
            },
        )
        .finish();

    Ok(pipeline)
}
