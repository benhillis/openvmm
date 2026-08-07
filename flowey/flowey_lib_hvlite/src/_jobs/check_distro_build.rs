// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Ensure `openvmm` still builds the way a Linux distribution package builds
//! it.
//!
//! OpenVMM publishes a source release that distributions build and package
//! themselves, so this configuration is a shipping interface. It differs from
//! every other build in CI in one important way: it does not use the
//! repository's `.packages/` provisioning, because a distribution build cannot
//! consume prebuilt native libraries. Every native dependency comes from a
//! distribution package instead, and the two environment overrides a packager
//! must set are set here as well.
//!
//! Without this job, a change that only resolves through `.packages/` breaks
//! downstream packagers silently, and we would not find out until someone
//! tried to build a release.
//!
//! The build runs against the release assets themselves and unpacks them the
//! way a packager would. In the release workflow these are the exact
//! transferred bytes the publish job later uploads. Building the checkout
//! instead would let this pass on a tree a packager cannot reproduce.

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
        ctx.import::<flowey_lib_common::install_rust::Node>();
        ctx.import::<flowey_lib_common::install_dist_pkg::Node>();
    }

    fn process_request(request: Self::Request, ctx: &mut NodeCtx<'_>) -> anyhow::Result<()> {
        let Request { release, done } = request;

        let target = target_lexicon::triple!("x86_64-unknown-linux-gnu");

        // This job deliberately does not depend on
        // `install_openvmm_rust_build_essential`. That node provisions `protoc`
        // out of `.packages/`, which is the dependency this job exists to prove
        // we do not need. It also skips the `-Dwarnings` cargo config, which a
        // packager does not build with either; the clippy jobs cover warnings.
        let mut deps = vec![ctx.reqv(flowey_lib_common::install_rust::Request::EnsureInstalled)];

        if matches!(
            ctx.platform(),
            FlowPlatform::Linux(FlowPlatformLinuxDistro::Ubuntu)
        ) {
            deps.push(ctx.reqv(|v| {
                flowey_lib_common::install_dist_pkg::Request::Install {
                    package_names: vec![
                        // a C toolchain and a working linker
                        "build-essential".into(),
                        // Linux UAPI headers, for the SQLite bundled by
                        // `libsqlite3-sys`. `build-essential` pulls this in
                        // transitively; it is named here so the reason it is
                        // needed is written down.
                        "linux-libc-dev".into(),
                        // for `openssl-sys`
                        "libssl-dev".into(),
                        "pkg-config".into(),
                        // for `prost` / `pbjson`
                        "protobuf-compiler".into(),
                    ],
                    done: v,
                }
            }));
        }

        ctx.req(flowey_lib_common::install_rust::Request::InstallTargetTriple(target.clone()));

        ctx.emit_rust_step("build openvmm in a distribution configuration", |ctx| {
            done.claim(ctx);
            deps.claim(ctx);
            let release = release.claim(ctx);
            move |rt| {
                let release = rt.read(release);
                let identity =
                    crate::assemble_openvmm_source_release::read_source_identity(&release.assets)?;
                let output_dir = release.assets;

                // Unpack the archive exactly as a packager would, into a
                // directory outside the repository so nothing can reach back
                // into the checkout.
                let build_root = std::env::current_dir()?.join("distro-build");
                if build_root.exists() {
                    fs_err::remove_dir_all(&build_root)?;
                }
                fs_err::create_dir_all(&build_root)?;
                let archive = output_dir.join(identity.archive_name());
                flowey::shell_cmd!(rt, "tar -xf {archive} -C {build_root}").run()?;

                let source_dir = build_root.join(identity.source_root());
                rt.sh.change_dir(&source_dir);

                // `.cargo/config.toml` points `PROTOC` into `.packages/`. It
                // does not set `force`, so an inherited `PROTOC` takes
                // precedence, which is what lets a packager redirect it at the
                // system compiler.
                let protoc = flowey::shell_cmd!(rt, "which protoc").read()?;
                let protoc = protoc.trim();

                let target = target.to_string();
                // Build the way a packager does. A spec file builds the release
                // profile, so building anything else here would leave
                // release-only code -- `#[cfg(not(debug_assertions))]` blocks
                // in particular -- never compiled by this gate.
                flowey::shell_cmd!(
                    rt,
                    "cargo build --release --locked -p openvmm --target {target}"
                )
                .env("PROTOC", protoc)
                // Link the system OpenSSL rather than building a vendored
                // copy. Nothing in `openvmm`'s tree enables `openssl-sys`'s
                // `vendored` feature today, so this is currently inert --
                // it is set because a packager sets it, and because it
                // becomes load-bearing the moment something turns that
                // feature on.
                .env("OPENSSL_NO_VENDOR", "1")
                // The workspace's release profile carries debug info, which
                // is the binding constraint on runner disk. Nothing debugs
                // this artifact.
                .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
                .env("CARGO_INCREMENTAL", "0")
                .run()?;

                Ok(())
            }
        });

        Ok(())
    }
}
