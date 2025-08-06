// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Shared utilities for adding version information to Windows DLLs
//!
//! This crate provides a consistent way to add version information to Windows DLLs
//! across different projects in the repository. It handles the creation of version
//! resource files and embedding them using the embed-resource crate.

/// Configuration for DLL version information
#[derive(Debug, Clone)]
pub struct DllVersionConfig {
    /// Company name (defaults to "Microsoft Corporation")
    pub company_name: String,
    /// File description
    pub file_description: String,
    /// Internal name of the DLL
    pub internal_name: String,
    /// Original filename of the DLL
    pub original_filename: String,
    /// Product name
    pub product_name: String,
    /// Legal copyright (defaults to "(C) Microsoft Corporation. All rights reserved.")
    pub legal_copyright: String,
    /// Major version number
    pub major: u16,
    /// Minor version number
    pub minor: u16,
    /// Patch version number
    pub patch: u16,
    /// Revision version number
    pub revision: u16,
}

impl DllVersionConfig {
    /// Create a new configuration with the specified names and version
    pub fn new(
        file_description: impl Into<String>,
        internal_name: impl Into<String>,
        original_filename: impl Into<String>,
        product_name: impl Into<String>,
        major: u16,
        minor: u16,
        patch: u16,
        revision: u16,
    ) -> Self {
        Self {
            company_name: "Microsoft Corporation".to_string(),
            file_description: file_description.into(),
            internal_name: internal_name.into(),
            original_filename: original_filename.into(),
            product_name: product_name.into(),
            legal_copyright: "(C) Microsoft Corporation. All rights reserved.".to_string(),
            major,
            minor,
            patch,
            revision,
        }
    }

    /// Create a configuration from environment variables with the specified prefix
    ///
    /// Looks for environment variables:
    /// - `{prefix}_MAJOR`
    /// - `{prefix}_MINOR`
    /// - `{prefix}_PATCH`
    /// - `{prefix}_REVISION`
    ///
    /// If any version environment variables are missing, returns None.
    /// If all are missing, uses default version 1.0.0.0.
    pub fn from_env(
        prefix: &str,
        file_description: impl Into<String>,
        internal_name: impl Into<String>,
        original_filename: impl Into<String>,
        product_name: impl Into<String>,
    ) -> Option<Self> {
        let major_key = format!("{}_MAJOR", prefix);
        let minor_key = format!("{}_MINOR", prefix);
        let patch_key = format!("{}_PATCH", prefix);
        let revision_key = format!("{}_REVISION", prefix);

        // Add rerun-if-env-changed for all version variables
        println!("cargo:rerun-if-env-changed={}", major_key);
        println!("cargo:rerun-if-env-changed={}", minor_key);
        println!("cargo:rerun-if-env-changed={}", patch_key);
        println!("cargo:rerun-if-env-changed={}", revision_key);

        // Check if any version env vars are set
        let has_major = std::env::var(&major_key).is_ok();
        let has_minor = std::env::var(&minor_key).is_ok();
        let has_patch = std::env::var(&patch_key).is_ok();
        let has_revision = std::env::var(&revision_key).is_ok();

        // If none are set, skip version handling
        if !has_major && !has_minor && !has_patch && !has_revision {
            return None;
        }

        // Parse version numbers, using defaults for missing values
        let major = std::env::var(&major_key)
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u16>()
            .unwrap_or_else(|_| panic!("{} must be a u16", major_key));

        let minor = std::env::var(&minor_key)
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u16>()
            .unwrap_or_else(|_| panic!("{} must be a u16", minor_key));

        let patch = std::env::var(&patch_key)
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u16>()
            .unwrap_or_else(|_| panic!("{} must be a u16", patch_key));

        let revision = std::env::var(&revision_key)
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u16>()
            .unwrap_or_else(|_| panic!("{} must be a u16", revision_key));

        Some(Self::new(
            file_description,
            internal_name,
            original_filename,
            product_name,
            major,
            minor,
            patch,
            revision,
        ))
    }

    /// Get the version as a comma-separated string for resource files
    pub fn version(&self) -> String {
        format!(
            "{},{},{},{}",
            self.major, self.minor, self.patch, self.revision
        )
    }

    /// Get the version as a quoted dot-separated string for resource files
    pub fn version_string(&self) -> String {
        format!(
            "\"{}.{}.{}.{}\"",
            self.major, self.minor, self.patch, self.revision
        )
    }

    /// Generate the resource content for a standard Windows DLL
    pub fn generate_resource_content(&self) -> String {
        format!(
            r#"1 VERSIONINFO
FILEVERSION    {}
PRODUCTVERSION {}
FILEOS 0x10004
FILETYPE 0x2
{{
BLOCK "StringFileInfo"
{{
	BLOCK "040904B0"
	{{
		VALUE "CompanyName", "{}"
		VALUE "FileDescription", "{}"
		VALUE "FileVersion", {}
		VALUE "InternalName", "{}"
		VALUE "LegalCopyright", "{}"
		VALUE "OriginalFilename", "{}"
		VALUE "ProductName", "{}"
		VALUE "ProductVersion", {}
	}}
}}

BLOCK "VarFileInfo"
{{
	VALUE "Translation", 0x0409 0x04B0
}}
}}

2 24 "manifest.xml"
"#,
            self.version(),         // FILEVERSION
            self.version(),         // PRODUCTVERSION
            self.company_name,      // CompanyName
            self.file_description,  // FileDescription
            self.version_string(),  // FileVersion
            self.internal_name,     // InternalName
            self.legal_copyright,   // LegalCopyright
            self.original_filename, // OriginalFilename
            self.product_name,      // ProductName
            self.version_string(),  // ProductVersion
        )
    }

    /// Generate a standard manifest.xml content
    pub fn generate_manifest_content() -> &'static str {
        r#"<?xml version='1.0' encoding='UTF-8' standalone='yes'?>
<assembly xmlns='urn:schemas-microsoft-com:asm.v1' manifestVersion='1.0'>
</assembly>
"#
    }

    /// Embed version information into a Windows DLL using embed-resource
    ///
    /// This function will:
    /// 1. Generate the resource content
    /// 2. Write it to a temporary resources.rc file
    /// 3. Generate a manifest.xml file if it doesn't exist
    /// 4. Use embed-resource to compile and embed the resources
    ///
    /// Additional resource content can be appended to the generated content
    /// by providing it in the `additional_content` parameter.
    pub fn embed_version_info(
        &self,
        additional_content: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Only process on Windows targets
        if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
            return Ok(());
        }

        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

        // Generate the resource content
        let mut content = self.generate_resource_content();
        if let Some(additional) = additional_content {
            content.push('\n');
            content.push_str(additional);
        }

        // Write resources.rc to OUT_DIR
        let resources_rc_path = out_dir.join("resources.rc");
        std::fs::write(&resources_rc_path, content)?;

        // Check if manifest.xml exists in the manifest dir, if not create it in OUT_DIR
        let manifest_source_path = manifest_dir.join("manifest.xml");
        if !manifest_source_path.exists() {
            let manifest_out_path = out_dir.join("manifest.xml");
            std::fs::write(&manifest_out_path, Self::generate_manifest_content())?;
        }

        // Add rerun triggers
        println!("cargo:rerun-if-changed=build.rs");
        if manifest_source_path.exists() {
            println!("cargo:rerun-if-changed={}", manifest_source_path.display());
        }

        // Use embed-resource to compile the resources
        embed_resource::compile(&resources_rc_path, std::iter::empty::<String>());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_formatting() {
        let config = DllVersionConfig::new(
            "Test DLL",
            "test.dll",
            "test.dll",
            "Test Product",
            1,
            2,
            3,
            4,
        );

        assert_eq!(config.version(), "1,2,3,4");
        assert_eq!(config.version_string(), "\"1.2.3.4\"");
    }

    #[test]
    fn test_resource_generation() {
        let config = DllVersionConfig::new(
            "Test DLL",
            "test.dll",
            "test.dll",
            "Test Product",
            1,
            2,
            3,
            4,
        );

        let content = config.generate_resource_content();
        assert!(content.contains("FILEVERSION    1,2,3,4"));
        assert!(content.contains("VALUE \"FileDescription\", \"Test DLL\""));
        assert!(content.contains("VALUE \"InternalName\", \"test.dll\""));
    }
}
