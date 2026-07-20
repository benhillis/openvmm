// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![expect(missing_docs)]

const BUILD_CHANNEL: &str = "OPENVMM_BUILD_CHANNEL";
const BUILD_DATE: &str = "OPENVMM_BUILD_DATE";
const BUILD_NUMBER: &str = "OPENVMM_BUILD_NUMBER";

enum BuildChannel {
    Dev,
    Release,
    Nightly(NightlyMetadata),
}

struct NightlyMetadata {
    date: String,
    build_number: u64,
}

fn parse_component(name: &str, value: &str) {
    value
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("{name} must be an unsigned 16-bit integer, got {value:?}"));
}

fn product_version() -> String {
    let version_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../VERSION");
    let version = std::fs::read_to_string(&version_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", version_path.display()));
    let version = version.trim();
    let components = version.split('.').collect::<Vec<_>>();
    let [major, minor, patch] = components.as_slice() else {
        panic!("OpenVMM VERSION must contain exactly three components, got {version:?}");
    };

    parse_component("OpenVMM VERSION major component", major);
    parse_component("OpenVMM VERSION minor component", minor);
    parse_component("OpenVMM VERSION patch component", patch);

    version.to_owned()
}

fn optional_env(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => panic!("{name} must be valid Unicode"),
    }
}

fn validate_date(date: &str) {
    if date.len() != 8 || !date.bytes().all(|byte| byte.is_ascii_digit()) {
        panic!("{BUILD_DATE} must use YYYYMMDD format, got {date:?}");
    }

    let year = date[0..4].parse::<u16>().unwrap();
    let month = date[4..6].parse::<u8>().unwrap();
    let day = date[6..8].parse::<u8>().unwrap();
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => panic!("{BUILD_DATE} contains invalid month {month}"),
    };

    if day == 0 || day > days_in_month {
        panic!("{BUILD_DATE} contains invalid day {day} for month {month}");
    }
}

fn build_channel() -> BuildChannel {
    let channel = optional_env(BUILD_CHANNEL);
    let date = optional_env(BUILD_DATE);
    let build_number = optional_env(BUILD_NUMBER);

    match (channel.as_deref(), date, build_number) {
        (None | Some("dev"), None, None) => BuildChannel::Dev,
        (Some("release"), None, None) => BuildChannel::Release,
        (Some("nightly"), Some(date), Some(build_number)) => {
            validate_date(&date);
            let parsed_build_number = build_number.parse::<u64>().unwrap_or_else(|_| {
                panic!("{BUILD_NUMBER} must be an unsigned integer, got {build_number:?}")
            });
            if parsed_build_number.to_string() != build_number {
                panic!(
                    "{BUILD_NUMBER} must use canonical unsigned integer formatting, got {build_number:?}"
                );
            }

            BuildChannel::Nightly(NightlyMetadata {
                date,
                build_number: parsed_build_number,
            })
        }
        (Some(channel), _, _) if !matches!(channel, "dev" | "release" | "nightly") => {
            panic!("{BUILD_CHANNEL} has unsupported value {channel:?}")
        }
        _ => panic!("{BUILD_DATE} and {BUILD_NUMBER} are only valid for nightly builds"),
    }
}

fn short_revision(git_info: Option<&build_rs_git_info::GitInfo>) -> Option<&str> {
    let revision = git_info?.sha();
    Some(
        revision
            .get(..9)
            .unwrap_or_else(|| panic!("OpenVMM Git revision is too short: {revision:?}")),
    )
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../VERSION");
    println!("cargo:rerun-if-env-changed={BUILD_CHANNEL}");
    println!("cargo:rerun-if-env-changed={BUILD_DATE}");
    println!("cargo:rerun-if-env-changed={BUILD_NUMBER}");

    let product_version = product_version();
    let build_channel = build_channel();
    let git_info = match build_rs_git_info::collect_git_info() {
        Ok(git_info) => {
            git_info.emit();
            Some(git_info)
        }
        Err(error) => {
            println!("cargo:warning=failed to collect OpenVMM git build information: {error:#}");
            None
        }
    };

    let (channel, version) = match build_channel {
        BuildChannel::Dev => {
            let version = match short_revision(git_info.as_ref()) {
                Some(revision) => format!("{product_version}-dev+g{revision}"),
                None => format!("{product_version}-dev"),
            };
            ("dev", version)
        }
        BuildChannel::Release => ("release", product_version.clone()),
        BuildChannel::Nightly(NightlyMetadata { date, build_number }) => {
            let revision = short_revision(git_info.as_ref())
                .unwrap_or_else(|| panic!("OpenVMM nightly builds require Git source information"));
            (
                "nightly",
                format!("{product_version}-nightly.{date}.{build_number}.g{revision}"),
            )
        }
    };

    println!("cargo:rustc-env=OPENVMM_PRODUCT_VERSION={product_version}");
    println!("cargo:rustc-env=OPENVMM_BUILD_CHANNEL={channel}");
    println!("cargo:rustc-env=OPENVMM_VERSION={version}");
}
