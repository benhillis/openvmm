// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Assemble, attest, and publish a standalone OpenVMM GitHub release.

use crate::build_openvmm::OpenvmmOutput;
use flowey::node::prelude::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

const RELEASE_TAG_PREFIX: &str = "openvmm-v";
const RELEASE_METADATA_SCHEMA: u32 = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OpenvmmReleaseTarget {
    WindowsX64,
    WindowsArm64,
    LinuxX64,
    LinuxArm64,
}

impl OpenvmmReleaseTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::WindowsX64 => "windows-x64",
            Self::WindowsArm64 => "windows-arm64",
            Self::LinuxX64 => "linux-x64-musl",
            Self::LinuxArm64 => "linux-arm64-musl",
        }
    }

    fn is_windows(self) -> bool {
        matches!(self, Self::WindowsX64 | Self::WindowsArm64)
    }
}

#[derive(Serialize, Deserialize)]
struct ReleaseSource {
    revision: String,
    tag: String,
    version: String,
}

#[derive(Serialize)]
struct ReleaseMetadata<'a> {
    schema_version: u32,
    version: &'a str,
    tag: &'a str,
    revision: &'a str,
}

flowey_request! {
    pub struct Request {
        pub openvmm: BTreeMap<OpenvmmReleaseTarget, ReadVar<OpenvmmOutput>>,
        pub done: WriteVar<SideEffect>,
    }
}

new_simple_flow_node!(struct Node);

impl SimpleFlowNode for Node {
    type Request = Request;

    fn imports(ctx: &mut ImportCtx<'_>) {
        ctx.import::<flowey_lib_common::attest_build_provenance::Node>();
        ctx.import::<flowey_lib_common::install_dist_pkg::Node>();
        ctx.import::<flowey_lib_common::publish_gh_release::Node>();
        ctx.import::<crate::git_checkout_openvmm_repo::Node>();
    }

    fn process_request(request: Self::Request, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        let Request { openvmm, done } = request;

        let expected_targets = BTreeSet::from([
            OpenvmmReleaseTarget::WindowsX64,
            OpenvmmReleaseTarget::WindowsArm64,
            OpenvmmReleaseTarget::LinuxX64,
            OpenvmmReleaseTarget::LinuxArm64,
        ]);
        let actual_targets = openvmm.keys().copied().collect::<BTreeSet<_>>();
        if actual_targets != expected_targets {
            anyhow::bail!(
                "OpenVMM releases require exactly the Windows x64/ARM64 and Linux musl x64/ARM64 targets; got {actual_targets:?}"
            );
        }

        let zip_installed =
            ctx.reqv(
                |done| flowey_lib_common::install_dist_pkg::Request::Install {
                    package_names: vec!["zip".into()],
                    done,
                },
            );
        let openvmm_repo_path = ctx.reqv(crate::git_checkout_openvmm_repo::req::GetRepoDir);

        let source = ctx.emit_rust_stepv("resolve OpenVMM release source", |ctx| {
            let openvmm_repo_path = openvmm_repo_path.clone().claim(ctx);
            move |rt| {
                let path = rt.read(openvmm_repo_path);
                rt.sh.change_dir(path);

                let ref_type =
                    std::env::var("GITHUB_REF_TYPE").context("GITHUB_REF_TYPE is not available")?;
                if ref_type != "tag" {
                    anyhow::bail!("OpenVMM releases must run from a Git tag");
                }
                let tag =
                    std::env::var("GITHUB_REF_NAME").context("GITHUB_REF_NAME is not available")?;
                let version = parse_release_tag(&tag)?;
                let revision = flowey::shell_cmd!(rt, "git rev-parse HEAD").read()?;
                let tag_revision = flowey::shell_cmd!(rt, "git rev-list -n 1 {tag}").read()?;
                if revision != tag_revision {
                    anyhow::bail!(
                        "OpenVMM release tag {tag:?} resolves to {tag_revision}, but the publisher \
                         checked out {revision}"
                    );
                }

                Ok(ReleaseSource {
                    revision,
                    tag,
                    version,
                })
            }
        });

        let assembled = ctx.emit_rust_stepv("assemble OpenVMM release archives", |ctx| {
            let zip_installed = zip_installed.claim(ctx);
            let openvmm_repo_path = openvmm_repo_path.clone().claim(ctx);
            let source = source.clone().claim(ctx);
            let openvmm = openvmm
                .into_iter()
                .map(|(target, output)| (target, output.claim(ctx)))
                .collect::<BTreeMap<_, _>>();

            move |rt| {
                rt.read(zip_installed);

                let openvmm_repo_path = rt.read(openvmm_repo_path);
                let source = rt.read(source);
                let output_dir = std::env::current_dir()?;
                let openvmm = openvmm
                    .into_iter()
                    .map(|(target, output)| (target, rt.read(output)))
                    .collect::<BTreeMap<_, _>>();
                let linux_x64 = match openvmm.get(&OpenvmmReleaseTarget::LinuxX64) {
                    Some(OpenvmmOutput::LinuxBin { bin, .. }) => bin,
                    _ => anyhow::bail!("Linux x64 OpenVMM output is not a Linux binary"),
                };
                flowey::shell_cmd!(rt, "chmod +x {linux_x64}").run()?;
                let version_output = flowey::shell_cmd!(rt, "{linux_x64} --version").read()?;
                let built_version = version_output
                    .strip_prefix("openvmm ")
                    .context("OpenVMM --version output has an unexpected format")?;
                if built_version != source.version {
                    anyhow::bail!(
                        "OpenVMM binary version does not match release tag {}: expected {}, got \
                         {built_version}",
                        source.tag,
                        source.version
                    );
                }

                let license = openvmm_repo_path.join("LICENSE");
                let mut archives = Vec::new();

                for (target, output) in openvmm {
                    let target_label = target.label();
                    let archive_stem = format!("openvmm-{}-{target_label}", source.version);
                    let runtime_dir = output_dir.join(format!("{archive_stem}-runtime"));
                    let symbols_dir = output_dir.join(format!("{archive_stem}-symbols"));

                    if runtime_dir.exists() {
                        fs_err::remove_dir_all(&runtime_dir)?;
                    }
                    if symbols_dir.exists() {
                        fs_err::remove_dir_all(&symbols_dir)?;
                    }
                    fs_err::create_dir_all(&runtime_dir)?;
                    fs_err::create_dir_all(&symbols_dir)?;
                    fs_err::copy(&license, runtime_dir.join("LICENSE"))?;
                    fs_err::copy(&license, symbols_dir.join("LICENSE"))?;

                    match (target.is_windows(), output) {
                        (true, OpenvmmOutput::WindowsBin { exe, pdb }) => {
                            fs_err::copy(exe, runtime_dir.join("openvmm.exe"))?;
                            let pdb = pdb.context(
                                "Windows OpenVMM release builds must include debug symbols",
                            )?;
                            fs_err::copy(pdb, symbols_dir.join("openvmm.pdb"))?;

                            let runtime_archive = output_dir.join(format!("{archive_stem}.zip"));
                            let symbols_archive =
                                output_dir.join(format!("{archive_stem}-symbols.zip"));
                            flowey::shell_cmd!(
                                rt,
                                "zip -j {runtime_archive} {runtime_dir}/openvmm.exe {runtime_dir}/LICENSE"
                            )
                            .run()?;
                            flowey::shell_cmd!(
                                rt,
                                "zip -j {symbols_archive} {symbols_dir}/openvmm.pdb {symbols_dir}/LICENSE"
                            )
                            .run()?;
                            archives.extend([runtime_archive, symbols_archive]);
                        }
                        (false, OpenvmmOutput::LinuxBin { bin, dbg }) => {
                            fs_err::copy(bin, runtime_dir.join("openvmm"))?;
                            flowey::shell_cmd!(rt, "chmod +x {runtime_dir}/openvmm").run()?;
                            fs_err::copy(dbg, symbols_dir.join("openvmm.dbg"))?;

                            let runtime_archive = output_dir.join(format!("{archive_stem}.tar.gz"));
                            let symbols_archive =
                                output_dir.join(format!("{archive_stem}-symbols.tar.gz"));
                            flowey::shell_cmd!(
                                rt,
                                "tar -czf {runtime_archive} -C {runtime_dir} ."
                            )
                            .run()?;
                            flowey::shell_cmd!(
                                rt,
                                "tar -czf {symbols_archive} -C {symbols_dir} ."
                            )
                            .run()?;
                            archives.extend([runtime_archive, symbols_archive]);
                        }
                        (true, OpenvmmOutput::LinuxBin { .. })
                        | (false, OpenvmmOutput::WindowsBin { .. }) => {
                            anyhow::bail!(
                                "OpenVMM output type does not match release target {target:?}"
                            );
                        }
                    }
                    fs_err::remove_dir_all(runtime_dir)?;
                    fs_err::remove_dir_all(symbols_dir)?;
                }

                rt.sh.change_dir(&openvmm_repo_path);
                let source_root = format!("openvmm-{}", source.version);
                let source_staging = output_dir.join(&source_root);
                if source_staging.exists() {
                    fs_err::remove_dir_all(&source_staging)?;
                }
                fs_err::create_dir(&source_staging)?;
                let source_tar = output_dir.join("openvmm-source.tar");
                if source_tar.exists() {
                    fs_err::remove_file(&source_tar)?;
                }
                flowey::shell_cmd!(rt, "git archive --format=tar --output {source_tar} HEAD")
                    .run()?;
                flowey::shell_cmd!(rt, "tar -xf {source_tar} -C {source_staging}").run()?;
                fs_err::remove_file(source_tar)?;

                let metadata = ReleaseMetadata {
                    schema_version: RELEASE_METADATA_SCHEMA,
                    version: &source.version,
                    tag: &source.tag,
                    revision: &source.revision,
                };
                let mut metadata_json = serde_json::to_vec_pretty(&metadata)?;
                metadata_json.push(b'\n');
                fs_err::write(
                    source_staging.join(".openvmm-release.json"),
                    metadata_json,
                )?;

                let source_archive = output_dir.join(format!("{source_root}-source.tar.gz"));
                flowey::shell_cmd!(
                    rt,
                    "tar -czf {source_archive} -C {output_dir} {source_root}"
                )
                .run()?;
                fs_err::remove_dir_all(source_staging)?;
                archives.push(source_archive);

                rt.sh.change_dir(&output_dir);
                let checksum_archives = archives
                    .iter()
                    .map(|path| {
                        path.file_name()
                            .context("release archive path does not have a file name")
                            .map(PathBuf::from)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let checksum_output =
                    flowey::shell_cmd!(rt, "sha256sum {checksum_archives...}").output()?;
                let checksums = output_dir.join("SHA256SUMS");
                fs_err::write(&checksums, checksum_output.stdout)?;
                archives.push(checksums);

                Ok(archives
                    .into_iter()
                    .map(|path| Ok((path.absolute()?, None)))
                    .collect::<anyhow::Result<Vec<_>>>()?)
            }
        });

        let target = source.clone().map(ctx, |source| source.revision);
        let tag = source.clone().map(ctx, |source| source.tag);
        let title = source.map(ctx, |source| format!("OpenVMM v{}", source.version));
        let (attestation_done, write_attestation_done) = ctx.new_var();
        ctx.req(flowey_lib_common::attest_build_provenance::Request {
            files: assembled.clone(),
            done: write_attestation_done,
        });
        ctx.req(flowey_lib_common::publish_gh_release::Request(
            flowey_lib_common::publish_gh_release::GhReleaseParams {
                repo_owner: "microsoft".into(),
                repo_name: "openvmm".into(),
                target,
                tag,
                title,
                files: assembled,
                notes: flowey_lib_common::publish_gh_release::GhReleaseNotes::Generated,
                draft: false,
                prerelease: false,
                allow_published_asset_replacement: false,
                prerequisites: vec![attestation_done],
                done,
            },
        ));

        Ok(())
    }
}

fn parse_release_tag(tag: &str) -> anyhow::Result<String> {
    let version = tag.strip_prefix(RELEASE_TAG_PREFIX).with_context(|| {
        format!("OpenVMM release tag must start with {RELEASE_TAG_PREFIX:?}, got {tag:?}")
    })?;
    let components = version.split('.').collect::<Vec<_>>();
    let [major, minor, patch] = components.as_slice() else {
        anyhow::bail!(
            "OpenVMM release version must contain exactly three components, got {version:?}"
        );
    };
    for (name, component) in [("major", major), ("minor", minor), ("patch", patch)] {
        if component.len() > 1 && component.starts_with('0') {
            anyhow::bail!("OpenVMM release {name} component is not canonical: {component:?}");
        }
        component.parse::<u16>().with_context(|| {
            format!(
                "OpenVMM release {name} component must be an unsigned 16-bit integer: {component:?}"
            )
        })?;
    }
    Ok(version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_tag_for_asset_version() {
        assert_eq!(parse_release_tag("openvmm-v0.12.3").unwrap(), "0.12.3");
        assert!(parse_release_tag("openvmm-v0.12.3-rc.1").is_err());
        assert!(parse_release_tag("openvmm-v0.012.3").is_err());
    }

    #[test]
    fn release_metadata_matches_source_bundle_schema() {
        let metadata = ReleaseMetadata {
            schema_version: 1,
            version: "0.12.3",
            tag: "openvmm-v0.12.3",
            revision: "0123456789abcdef0123456789abcdef01234567",
        };
        assert_eq!(
            serde_json::to_value(metadata).unwrap(),
            serde_json::json!({
                "schema_version": 1,
                "version": "0.12.3",
                "tag": "openvmm-v0.12.3",
                "revision": "0123456789abcdef0123456789abcdef01234567",
            })
        );
    }
}
