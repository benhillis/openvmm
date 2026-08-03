// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

const RELEASE_TAG_PREFIX: &str = "openvmm-v";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Release,
    Development,
    Custom,
}

impl BuildKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Development => "development",
            Self::Custom => "custom",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Development => "development (not an official release)",
            Self::Custom => "custom (built with OPENVMM_PKGVERSION)",
        }
    }
}

pub struct GitSource {
    pub revision: String,
    pub tags: Vec<String>,
}

pub struct VersionInfo {
    pub version: String,
    pub kind: BuildKind,
    pub revision: String,
}

pub fn resolve_version(
    product_version: &str,
    package_version: Option<&str>,
    git: Option<GitSource>,
) -> VersionInfo {
    if let Some(package_version) = package_version.filter(|value| !value.is_empty()) {
        return VersionInfo {
            version: package_version.to_owned(),
            kind: BuildKind::Custom,
            revision: git.map_or_else(String::new, |git| git.revision),
        };
    }

    let Some(git) = git else {
        return VersionInfo {
            version: product_version.to_owned(),
            kind: BuildKind::Release,
            revision: String::new(),
        };
    };

    let expected_tag = format!("{RELEASE_TAG_PREFIX}{product_version}");
    let release_tags = git
        .tags
        .iter()
        .filter(|tag| tag.starts_with(RELEASE_TAG_PREFIX))
        .collect::<Vec<_>>();
    if matches!(release_tags.as_slice(), [tag] if tag.as_str() == expected_tag) {
        return VersionInfo {
            version: product_version.to_owned(),
            kind: BuildKind::Release,
            revision: git.revision,
        };
    }

    let short_revision = &git.revision[..9.min(git.revision.len())];
    VersionInfo {
        version: format!("{product_version}+g{short_revision}"),
        kind: BuildKind::Development,
        revision: git.revision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_with_tracing::test;

    const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

    fn git(tags: &[&str]) -> GitSource {
        GitSource {
            revision: REVISION.into(),
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
        }
    }

    #[test]
    fn an_archive_reports_the_product_version() {
        let version = resolve_version("0.2.0", None, None);
        assert_eq!(version.version, "0.2.0");
        assert_eq!(version.kind, BuildKind::Release);
        assert!(version.revision.is_empty());
    }

    #[test]
    fn an_ordinary_checkout_includes_the_revision() {
        let version = resolve_version("0.2.0", None, Some(git(&[])));
        assert_eq!(version.version, "0.2.0+g012345678");
        assert_eq!(version.kind, BuildKind::Development);
        assert_eq!(version.revision, REVISION);
    }

    #[test]
    fn the_exact_matching_tag_reports_a_release() {
        let version = resolve_version("0.2.0", None, Some(git(&["openvmm-v0.2.0"])));
        assert_eq!(version.version, "0.2.0");
        assert_eq!(version.kind, BuildKind::Release);
    }

    #[test]
    fn mismatched_or_ambiguous_tags_fail_toward_development() {
        for tags in [
            &["openvmm-v0.1.0"][..],
            &["openvmm-v0.2.0", "openvmm-v0.1.0"][..],
        ] {
            let version = resolve_version("0.2.0", None, Some(git(tags)));
            assert_eq!(version.version, "0.2.0+g012345678");
            assert_eq!(version.kind, BuildKind::Development);
        }
    }

    #[test]
    fn a_package_override_is_custom() {
        let version = resolve_version(
            "0.2.0",
            Some("Distribution package 0.2.0-4"),
            Some(git(&[])),
        );
        assert_eq!(version.version, "Distribution package 0.2.0-4");
        assert_eq!(version.kind, BuildKind::Custom);
        assert_eq!(version.revision, REVISION);
    }

    #[test]
    fn build_kind_has_stable_output_names() {
        assert_eq!(BuildKind::Release.name(), "release");
        assert_eq!(BuildKind::Development.name(), "development");
        assert_eq!(BuildKind::Custom.name(), "custom");
        assert_eq!(BuildKind::Release.description(), "release");
        assert!(
            BuildKind::Development
                .description()
                .contains("not an official release")
        );
        assert!(
            BuildKind::Custom
                .description()
                .contains("OPENVMM_PKGVERSION")
        );
    }
}
