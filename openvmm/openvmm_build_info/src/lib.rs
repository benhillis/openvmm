// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! OpenVMM product version and build identity.

#![expect(missing_docs)]

#[cfg(test)]
#[path = "../version.rs"]
mod version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Release,
    Development,
    Custom,
}

impl BuildKind {
    const fn resolve() -> Self {
        match env!("OPENVMM_BUILD_KIND").as_bytes() {
            b"release" => Self::Release,
            b"development" => Self::Development,
            _ => Self::Custom,
        }
    }

    pub const fn is_release(self) -> bool {
        matches!(self, Self::Release)
    }
}

#[derive(Debug)]
pub struct BuildInfo {
    version: &'static str,
    long_version: &'static str,
    product_version: &'static str,
    revision: &'static str,
    target: &'static str,
    kind: BuildKind,
}

impl BuildInfo {
    pub const fn new() -> Self {
        Self {
            version: env!("OPENVMM_VERSION"),
            long_version: include_str!(concat!(env!("OUT_DIR"), "/long_version.txt")),
            product_version: env!("OPENVMM_PRODUCT_VERSION"),
            revision: env!("OPENVMM_REVISION"),
            target: env!("OPENVMM_TARGET"),
            kind: BuildKind::resolve(),
        }
    }

    pub const fn version(&self) -> &'static str {
        self.version
    }

    pub const fn long_version(&self) -> &'static str {
        self.long_version
    }

    pub const fn product_version(&self) -> &'static str {
        self.product_version
    }

    pub const fn scm_revision(&self) -> &'static str {
        self.revision
    }

    pub const fn target(&self) -> &'static str {
        self.target
    }

    pub const fn kind(&self) -> BuildKind {
        self.kind
    }
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self::new()
    }
}

// Keep the build information discoverable without running the binary.
// UNSAFETY: link_section and export_name are unsafe.
#[expect(unsafe_code)]
// SAFETY: These are custom metadata sections with no safety requirements.
#[cfg_attr(target_os = "windows", unsafe(link_section = ".build_i"))]
#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,__build_info"))]
#[cfg_attr(
    not(any(target_os = "windows", target_vendor = "apple")),
    unsafe(link_section = ".build_info")
)]
// SAFETY: This symbol is uniquely named for OpenVMM and has no runtime ABI.
#[unsafe(export_name = "OPENVMM_BUILD_INFO")]
static OPENVMM_BUILD_INFO: BuildInfo = BuildInfo::new();

pub fn get() -> &'static BuildInfo {
    std::hint::black_box(&OPENVMM_BUILD_INFO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_with_tracing::test;

    #[test]
    fn product_version_is_not_the_cargo_default() {
        assert_ne!(get().product_version(), "0.0.0");
        assert!(!get().version().is_empty());
    }

    #[test]
    fn revision_is_a_full_object_id_or_absent() {
        let revision = get().scm_revision();
        assert!(
            revision.is_empty()
                || (matches!(revision.len(), 40 | 64)
                    && revision.bytes().all(|byte| byte.is_ascii_hexdigit())),
            "unexpected revision {revision:?}"
        );
    }

    #[test]
    fn long_version_carries_the_identity() {
        let info = get();
        let long = info.long_version();
        assert!(long.starts_with(info.version()), "{long:?}");
        assert!(long.contains(info.product_version()), "{long:?}");
        assert!(long.contains(info.target()), "{long:?}");
        if !info.scm_revision().is_empty() {
            assert!(long.contains(info.scm_revision()), "{long:?}");
        }
    }
}
