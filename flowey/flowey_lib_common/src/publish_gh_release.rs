// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Publish a github release

use flowey::node::prelude::*;

flowey_request! {
    pub struct Request(pub GhReleaseParams);
}

#[derive(Serialize, Deserialize)]
pub enum GhReleaseNotes {
    Generated,
    Text(String),
}

#[derive(Serialize, Deserialize)]
pub struct GhReleaseParams<C = VarNotClaimed> {
    /// First component of a github repo path
    ///
    /// e.g: the "foo" in "github.com/foo/bar"
    pub repo_owner: String,
    /// Second component of a github repo path
    ///
    /// e.g: the "bar" in "github.com/foo/bar"
    pub repo_name: String,
    /// Commit hash to target
    pub target: ReadVar<String, C>,
    /// Tag associated with the release artifact.
    pub tag: ReadVar<String, C>,
    /// Title associated with the release artifact.
    pub title: ReadVar<String, C>,
    /// Files to upload.
    pub files: ReadVar<Vec<(PathBuf, Option<String>)>, C>,
    /// Release notes to attach to the release.
    pub notes: GhReleaseNotes,
    /// Whether the release should be created as a draft
    pub draft: bool,
    /// Whether the release should be marked as a prerelease.
    pub prerelease: bool,
    /// Whether retries may replace assets on an existing published release.
    pub allow_published_asset_replacement: bool,
    /// Side effects that must complete before the release is published.
    pub prerequisites: Vec<ReadVar<SideEffect, C>>,

    pub done: WriteVar<SideEffect, C>,
}

impl GhReleaseParams {
    pub fn claim(self, ctx: &mut StepCtx<'_>) -> GhReleaseParams<VarClaimed> {
        let GhReleaseParams {
            repo_owner,
            repo_name,
            target,
            tag,
            title,
            files,
            notes,
            draft,
            prerelease,
            allow_published_asset_replacement,
            prerequisites,
            done,
        } = self;

        GhReleaseParams {
            repo_owner,
            repo_name,
            target: target.claim(ctx),
            tag: tag.claim(ctx),
            title: title.claim(ctx),
            files: files.claim(ctx),
            notes,
            draft,
            prerelease,
            allow_published_asset_replacement,
            prerequisites: prerequisites.claim(ctx),
            done: done.claim(ctx),
        }
    }
}

new_flow_node!(struct Node);

impl FlowNode for Node {
    type Request = Request;

    fn imports(ctx: &mut ImportCtx<'_>) {
        ctx.import::<crate::cache::Node>();
        ctx.import::<crate::use_gh_cli::Node>();
    }

    fn emit(requests: Vec<Self::Request>, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        if requests.is_empty() {
            return Ok(());
        }

        let gh_cli = ctx.reqv(crate::use_gh_cli::Request::Get);

        ctx.emit_rust_step("publish github releases", |ctx| {
            let requests = requests
                .into_iter()
                .map(|r| r.0.claim(ctx))
                .collect::<Vec<_>>();
            let gh_cli = gh_cli.claim(ctx);

            move |rt| {
                let gh_cli = rt.read(gh_cli);

                for req in requests {
                    let GhReleaseParams {
                        repo_owner,
                        repo_name,
                        target,
                        tag,
                        title,
                        files,
                        notes,
                        draft,
                        prerelease,
                        allow_published_asset_replacement,
                        prerequisites,
                        done: _,
                    } = req;

                    for prerequisite in prerequisites {
                        rt.read(prerequisite);
                    }

                    let repo = format!("{repo_owner}/{repo_name}");
                    let target = rt.read(target);
                    let tag = rt.read(tag);

                    // check if the release already exists
                    //
                    // xshell doesn't give us the exit code, so we have to
                    // use the raw process API instead.
                    let title = rt.read(title);
                    let files = rt.read(files)
                        .into_iter()
                        .map(|(path, label)| {
                            let path = path.to_string_lossy().to_string();
                            if let Some(label) = label {
                                format!("{path}#{label}")
                            } else {
                                path
                            }
                        })
                        .collect::<Vec<_>>();
                    let draft = draft.then_some("--draft");

                    let existing_release = std::process::Command::new(&gh_cli)
                        .args([
                            "release",
                            "view",
                            &tag,
                            "--repo",
                            &repo,
                            "--json",
                            "isDraft",
                            "--jq",
                            ".isDraft",
                        ])
                        .output()
                        .context("failed to run gh release view")?;

                    if existing_release.status.success() {
                        let is_draft = String::from_utf8(existing_release.stdout)?;
                        if is_draft.trim() != "true" && !allow_published_asset_replacement {
                            anyhow::bail!(
                                "GitHub release with tag {tag} already exists and is not a draft"
                            );
                        }

                        log::info!(
                            "GitHub release with tag {tag} already exists in repo {repo}; replacing \
                             its assets"
                        );
                        flowey::shell_cmd!(
                            rt,
                            "{gh_cli} release upload {tag} {files...} --repo {repo} --clobber"
                        )
                        .run()?;
                        continue;
                    }

                    let notes = match notes {
                        GhReleaseNotes::Generated => vec!["--generate-notes".to_owned()],
                        GhReleaseNotes::Text(notes) => vec!["--notes".to_owned(), notes],
                    };
                    let prerelease = prerelease.then_some("--prerelease");
                    flowey::shell_cmd!(rt, "{gh_cli} release create {tag} {files...} --repo {repo} --target {target} --title {title} {notes...} {draft...} {prerelease...}").run()?;
                }

                Ok(())
            }
        });

        Ok(())
    }
}
