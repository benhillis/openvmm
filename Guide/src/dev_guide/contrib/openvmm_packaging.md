# Packaging OpenVMM for Linux

This page describes building an OpenVMM Linux distribution package from the
published source release.

```admonish note title="See also"
[OpenVMM Release Model](./openvmm_release.md) describes release identity,
assets, and the maintainer release runbook.
[Crypto Backends](./crypto_backends.md) describes the native crypto selection
used by the build.
```

## Obtain and verify the source

Each release publishes `openvmm-<VERSION>-source.tar.gz` and `SHA256SUMS`.
Download both from the GitHub release and verify the checksum:

```bash
sha256sum -c SHA256SUMS
```

Verify the source archive's GitHub build provenance:

```bash
gh attestation verify openvmm-<VERSION>-source.tar.gz \
    --repo microsoft/openvmm
```

```admonish warning title="Use the published OpenVMM archive"
GitHub also generates generic "Source code" links. Those files are not named
in `SHA256SUMS` and do not carry the OpenVMM release asset's provenance
attestation. Package `openvmm-<VERSION>-source.tar.gz`.
```

The extracted archive reports the plain release version because its root
`Cargo.toml` contains the canonical workspace version and the archive contains
no Git metadata.

## Rust toolchain

The source release requires Rust 1.95 or newer. Declare this requirement in
the distribution package metadata.

```spec
BuildRequires: rust >= 1.95
BuildRequires: cargo >= 1.95
```

Update the package requirement when a later OpenVMM release documents a newer
minimum.

```admonish warning title="Distribution Rust may be older"
Cargo rejects the workspace when the active compiler is older than the required
Rust version. Use a distribution release whose packaged toolchain meets the
requirement, or provide a suitable toolchain in the package build environment.
```

## Build configuration

Build the host `x86_64-unknown-linux-gnu` target dynamically linked against
the distribution's glibc and OpenSSL. Do not use
`cargo xflowey restore-packages`; that command restores prebuilt native
dependencies intended for repository development.

Install:

- a C compiler and linker;
- glibc development headers;
- Linux UAPI headers;
- OpenSSL development headers;
- `pkg-config`;
- a Protocol Buffers compiler providing `protoc`.

Point the build at the system `protoc` and prevent vendored OpenSSL:

```bash
export PROTOC="$(command -v protoc)"
export OPENSSL_NO_VENDOR=1
cargo build --release --locked -p openvmm \
    --target x86_64-unknown-linux-gnu
```

OpenVMM CI runs this distribution configuration on every change. It builds the
assembled source archive outside the repository using locked Cargo
dependencies and the distribution's system dependencies.

## Offline builds

The OpenVMM release contains project source, not a vendored Cargo dependency
tree. A distribution that requires an offline build should vendor dependencies
separately and cover that vendor archive with its own integrity metadata.

Create the vendor tree:

```bash
cargo vendor vendor/ > vendor-config.toml
```

Append the generated source replacement configuration to `.cargo/config.toml`, then build offline:

```bash
cargo build --release --locked --offline -p openvmm \
    --target x86_64-unknown-linux-gnu
```

`cargo vendor` operates on the workspace, so the vendor tree includes
dependencies not compiled by the OpenVMM Linux binary.

## Package identity

The OpenVMM binary reports the upstream product version committed in the
source tree. Record any distribution-specific package revision in the
distribution's package metadata rather than replacing the binary's version.

## Runtime dependencies

Confirm the exact dependencies for the packaged executable with `readelf` or
the distribution's automatic dependency generator. The expected shared
libraries include:

- glibc;
- OpenSSL (`libssl` and `libcrypto`);
- `libgcc_s`.

The SQLite dependency is compiled into the binary and does not add a shared
runtime dependency.

## Example package fragment

```spec
BuildRequires: rust >= 1.95
BuildRequires: cargo >= 1.95
BuildRequires: gcc glibc-devel binutils kernel-headers
BuildRequires: openssl-devel pkg-config
BuildRequires: protobuf-compiler

Requires: glibc
Requires: openssl-libs
```

Build and install:

```spec
%build
export PROTOC="$(command -v protoc)"
export OPENSSL_NO_VENDOR=1
cargo build --release --locked --offline -p openvmm \
    --target x86_64-unknown-linux-gnu

%install
install -D -m0755 \
    target/x86_64-unknown-linux-gnu/release/openvmm \
    %{buildroot}%{_bindir}/openvmm
```
