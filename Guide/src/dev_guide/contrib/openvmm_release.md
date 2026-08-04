# OpenVMM Release Model

This page describes OpenVMM versions, source releases, build identity, and the
maintainer release runbook.

OpenVMM and OpenHCL share a repository, but their releases are independent.
OpenVMM publishes standalone source releases from `main`.
[OpenHCL Release Management](./openhcl_release.md) describes the separate
OpenHCL release-branch and servicing process.

## Release scope

The first release phase publishes source only:

- `openvmm-<VERSION>-source.tar.gz`;
- `SHA256SUMS`.

Every asset receives a GitHub build provenance attestation. Prebuilt binaries
are out of scope for this phase.

The archive contains the tracked standalone source tree at one commit. It does
not contain `.git`, prebuilt native dependencies, or vendored Rust crates.

## Version scheme

OpenVMM versions use stable Semantic Versioning:

```text
MAJOR.MINOR.PATCH
```

The canonical version is `[workspace.package] version` in the root
`Cargo.toml`. All versioned OpenVMM crates inherit it. The value remains at the
most recently released version until a reviewed pull request selects the next
release.

The initial workflow accepts stable three-component versions only. It rejects
prerelease suffixes, build metadata, leading zeroes, missing components, and
additional components.

Release tags use:

```text
openvmm-vMAJOR.MINOR.PATCH
```

For example, version `0.2.0` produces tag `openvmm-v0.2.0` and archive
`openvmm-0.2.0-source.tar.gz`.

## Build identity

The committed product version and the build identity answer different
questions. The product version names the release selected by the tree. Git
metadata identifies a development build made from a checkout.

| Source shape | Example `openvmm -V` | Build kind |
| --- | --- | --- |
| Ordinary checkout | `openvmm 0.2.0+g012345678` | development |
| Checkout of `openvmm-v0.2.0` | `openvmm 0.2.0+g012345678` | development |
| Extracted published archive | `openvmm 0.2.0` | release |

Every Git checkout reports development identity, including an exact checkout
of a release tag. Release tags are publication markers and are not inspected
when deriving build identity.

An extracted archive has no Git history, so Cargo reads the version already
committed in `Cargo.toml`. Nothing stamps or rewrites the archive during
release assembly.

`openvmm --version` prints the detailed form with the product version, build
kind, full revision when available, and build target.

## Distribution-build gate

CI assembles the same standalone source archive used by the release workflow
and builds it outside the repository as a Linux distribution would.

The gate:

1. consumes the exact archive intended for publication;
2. extracts it outside the checkout;
3. builds `openvmm` with `--release --locked` for
   `x86_64-unknown-linux-gnu`, using system `protoc` and OpenSSL rather than
   `.packages/`.

This standalone build does not use `openvmm-deps`. OpenHCL sysroots, firmware,
and test assets from that repository are not required to build the standalone
OpenVMM binary.

Additional assertions, such as checksum verification, archive-shape checks,
binary-version validation, or direct linkage inspection, may be added later
when each check enforces an agreed release requirement.

The release workflow assembles the archive once and transfers it as an internal
workflow artifact. The distribution gate builds that artifact, and the publish
job uploads those same bytes after the gate succeeds. Archive assembly remains
reproducible for a given commit, but the release does not rely on recreating it.

## Normal release runbook

### 1. Select the version

Merge a normal reviewed pull request changing `[workspace.package] version`
from the previous release to the next stable version.

The version PR records the release decision but does not publish anything.
There is no `-dev` transition, follow-up version commit, release branch, or
freeze on `main`.

### 2. Select the commit

Open **Actions > OpenVMM Release > Run workflow** and select `main`. The
workflow releases the current head when the run starts.

GitHub pins that commit for the entire run. Later commits may continue landing
on `main` without changing the release in progress.

The workflow requires the selected commit to be reachable from `main`. It also
requires the workspace version to be newer than every existing stable
`openvmm-v*` release tag.

### 3. Run the workflow

The workflow:

1. resolves the version and full revision from the pinned tree;
2. validates the release version and mainline ancestry;
3. rejects an existing release tag;
4. assembles the source archive and `SHA256SUMS` once;
5. transfers and builds those exact assets in the distribution configuration;
6. transfers the same assets to the publish job;
7. generates provenance attestations;
8. creates a draft GitHub release with empty notes, targeting the pinned
   commit.

A failed run may be rerun from the same workflow run, which retains the same
commit. A fresh dispatch pins the then-current `main` commit.

If a draft or published release already exists for the version, the workflow
fails without modifying it. Delete an unwanted draft manually before rerunning.
An existing Git tag is also rejected.

### 4. Review the draft

Before publishing, confirm:

- the release targets the intended commit;
- the title and tag are `OpenVMM v<VERSION>` and `openvmm-v<VERSION>`;
- the source archive and `SHA256SUMS` are present;
- the checksum validates;
- both assets have provenance attestations;
- release notes describe the intended changes and mention any Rust requirement
  change.

Verify downloaded assets with:

```bash
sha256sum -c SHA256SUMS
gh attestation verify openvmm-<VERSION>-source.tar.gz \
    --repo microsoft/openvmm
```

### 5. Publish the draft

Publishing the draft is the irreversible step. GitHub creates
`openvmm-v<VERSION>` at the draft's pinned target only when a human publishes
the release.

After publication, do not move the tag or replace assets. Correct a release
problem with a reviewed pull request selecting a new patch version, followed
by a new release.

## Minimum Rust version

The packaging guide documents the Rust version required by the source release.
When that requirement changes, update the guide and call it out in the release
notes so downstream packagers can evaluate the change.

## Security fixes

Do not report vulnerabilities through public issues or pull requests. Follow
the private process in the repository
[`SECURITY.md`](https://github.com/microsoft/openvmm/blob/main/SECURITY.md).

Before stable servicing branches exist, fixes land on `main` and ship in the
next OpenVMM release. The project may define a separate patch-release and
long-term support policy later.
