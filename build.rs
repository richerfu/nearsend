use std::{fs, path::Path};

fn main() {
    verify_ohos_version_metadata();
    napi_build_ohos::setup();

    println!("cargo:rustc-link-lib=dylib=c++_shared");
}

fn verify_ohos_version_metadata() {
    let cargo_version = std::env::var("CARGO_PKG_VERSION")
        .expect("Cargo must provide CARGO_PKG_VERSION to build.rs");
    let expected_version_code = version_code(&cargo_version);

    verify_string_field(
        "platform/ohos/AppScope/app.json5",
        "versionName",
        &cargo_version,
    );
    verify_number_field(
        "platform/ohos/AppScope/app.json5",
        "versionCode",
        expected_version_code,
    );
    verify_string_field(
        "platform/ohos/entry/oh-package.json5",
        "version",
        &cargo_version,
    );
    verify_string_field(
        "platform/ohos/entry/src/main/ets/types/libnear_send/oh-package.json5",
        "version",
        &cargo_version,
    );
}

fn verify_string_field(path: &str, field: &str, expected: &str) {
    let contents = read_metadata(path);
    let actual = field_value(&contents, field)
        .and_then(|value| value.strip_prefix('"')?.strip_suffix('"'))
        .unwrap_or_else(|| panic!("missing string field `{field}` in {path}"));
    assert_eq!(
        actual, expected,
        "version mismatch in {path}: `{field}` must match Cargo package version"
    );
}

fn verify_number_field(path: &str, field: &str, expected: u32) {
    let contents = read_metadata(path);
    let actual = field_value(&contents, field)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| panic!("missing numeric field `{field}` in {path}"));
    assert_eq!(
        actual, expected,
        "version mismatch in {path}: `{field}` must match Cargo package version"
    );
}

fn read_metadata(path: &str) -> String {
    println!("cargo:rerun-if-changed={path}");
    fs::read_to_string(Path::new(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn field_value<'a>(contents: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("\"{field}\"");
    contents.lines().find_map(|line| {
        let line = line.trim();
        let remainder = line.strip_prefix(&prefix)?.trim_start();
        let value = remainder.strip_prefix(':')?.trim_start();
        Some(value.trim_end_matches(',').trim())
    })
}

fn version_code(version: &str) -> u32 {
    let mut components = version.split('.');
    let major = parse_version_component(components.next(), "major", version);
    let minor = parse_version_component(components.next(), "minor", version);
    let patch = parse_version_component(components.next(), "patch", version);
    assert!(
        components.next().is_none(),
        "application version `{version}` must use major.minor.patch"
    );
    major
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(minor.checked_mul(1_000)?))
        .and_then(|value| value.checked_add(patch))
        .unwrap_or_else(|| panic!("application version `{version}` is too large for versionCode"))
}

fn parse_version_component(component: Option<&str>, name: &str, version: &str) -> u32 {
    component
        .and_then(|component| component.parse::<u32>().ok())
        .unwrap_or_else(|| panic!("invalid {name} component in application version `{version}`"))
}
