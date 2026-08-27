//! Third-party dependency metadata for the open source licenses page.

#[derive(Clone)]
pub struct ThirdPartyLib {
    pub name: String,
    pub repository: String,
    pub license: String,
    pub license_text: String,
}

impl ThirdPartyLib {
    fn new(name: &str, repository: &str, license: &str, license_text: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            repository: repository.to_string(),
            license: license.to_string(),
            license_text: license_text.into(),
        }
    }
}

const GPUI_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/gpui-LICENSE-APACHE.txt");
const GPUI_COMPONENT_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/gpui-component-LICENSE-APACHE.txt");
const GPUI_ROUTER_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/gpui-router-LICENSE-MIT.txt");
const OPENHARMONY_ABILITY_LICENSE: &str =
    include_str!("../../../licenses/third_party/openharmony-ability-LICENSE.txt");
const LOCALSEND_LICENSE: &str = include_str!("../../../licenses/third_party/localsend-LICENSE.txt");

const NAPI_OHOS_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/napi-ohos-LICENSE-MIT.txt");
const NAPI_DERIVE_OHOS_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/napi-derive-ohos-LICENSE-MIT.txt");
const NAPI_DERIVE_BACKEND_OHOS_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/napi-derive-backend-ohos-LICENSE-MIT.txt");
const OHOS_HILOG_BINDING_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/ohos-hilog-binding-LICENSE-APACHE.txt");
const OHOS_HILOG_BINDING_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/ohos-hilog-binding-LICENSE-MIT.txt");
const LOG_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/log-LICENSE-APACHE.txt");
const LOG_LICENSE_MIT: &str = include_str!("../../../licenses/third_party/log-LICENSE-MIT.txt");

const TOKIO_LICENSE: &str = include_str!("../../../licenses/third_party/tokio-LICENSE.txt");
const SERDE_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/serde-LICENSE-APACHE.txt");
const SERDE_LICENSE_MIT: &str = include_str!("../../../licenses/third_party/serde-LICENSE-MIT.txt");
const SERDE_JSON_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/serde_json-LICENSE-APACHE.txt");
const SERDE_JSON_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/serde_json-LICENSE-MIT.txt");
const ANYHOW_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/anyhow-LICENSE-APACHE.txt");
const ANYHOW_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/anyhow-LICENSE-MIT.txt");
const UUID_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/uuid-LICENSE-APACHE.txt");
const UUID_LICENSE_MIT: &str = include_str!("../../../licenses/third_party/uuid-LICENSE-MIT.txt");
const RCGEN_LICENSE: &str = include_str!("../../../licenses/third_party/rcgen-LICENSE.txt");
const REQWEST_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/reqwest-LICENSE-APACHE.txt");
const REQWEST_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/reqwest-LICENSE-MIT.txt");
const FUTURES_UTIL_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/futures-util-LICENSE-APACHE.txt");
const FUTURES_UTIL_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/futures-util-LICENSE-MIT.txt");
const CHRONO_LICENSE: &str = include_str!("../../../licenses/third_party/chrono-LICENSE.txt");
const HYPER_LICENSE: &str = include_str!("../../../licenses/third_party/hyper-LICENSE.txt");
const HYPER_UTIL_LICENSE: &str =
    include_str!("../../../licenses/third_party/hyper-util-LICENSE.txt");
const HTTP_BODY_UTIL_LICENSE: &str =
    include_str!("../../../licenses/third_party/http-body-util-LICENSE.txt");
const BYTES_LICENSE: &str = include_str!("../../../licenses/third_party/bytes-LICENSE.txt");
const BASE64_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/base64-LICENSE-APACHE.txt");
const BASE64_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/base64-LICENSE-MIT.txt");
const TOKIO_RUSTLS_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/tokio-rustls-LICENSE-APACHE.txt");
const TOKIO_RUSTLS_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/tokio-rustls-LICENSE-MIT.txt");
const RUSTLS_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/rustls-LICENSE-APACHE.txt");
const RUSTLS_LICENSE_ISC: &str =
    include_str!("../../../licenses/third_party/rustls-LICENSE-ISC.txt");
const RUSTLS_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/rustls-LICENSE-MIT.txt");
const IF_ADDRS_LICENSE_BSD: &str =
    include_str!("../../../licenses/third_party/if-addrs-LICENSE-BSD.txt");
const IF_ADDRS_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/if-addrs-LICENSE-MIT.txt");
const SOCKET2_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/socket2-LICENSE-APACHE.txt");
const SOCKET2_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/socket2-LICENSE-MIT.txt");
const QRCODE_LICENSE_APACHE: &str =
    include_str!("../../../licenses/third_party/qrcode-LICENSE-APACHE.txt");
const QRCODE_LICENSE_MIT: &str =
    include_str!("../../../licenses/third_party/qrcode-LICENSE-MIT.txt");
const URLENCODING_LICENSE: &str =
    include_str!("../../../licenses/third_party/urlencoding-LICENSE.txt");
const RUST_EMBED_LICENSE: &str =
    include_str!("../../../licenses/third_party/rust-embed-LICENSE.txt");

fn merge_license(parts: &[&str]) -> String {
    let mut merged = String::new();
    for (index, part) in parts.iter().enumerate() {
        merged.push_str(part.trim_end());
        if index + 1 < parts.len() {
            merged.push_str("\n\n");
        }
    }
    merged
}

pub fn get_third_party_libs() -> Vec<ThirdPartyLib> {
    vec![
        ThirdPartyLib::new(
            "gpui",
            "https://github.com/zed-industries/zed.git",
            "Apache-2.0",
            GPUI_LICENSE_APACHE,
        ),
        ThirdPartyLib::new(
            "gpui_platform",
            "https://github.com/zed-industries/zed.git",
            "Apache-2.0",
            GPUI_LICENSE_APACHE,
        ),
        ThirdPartyLib::new(
            "gpui-component",
            "https://github.com/longbridge/gpui-component.git",
            "Apache-2.0",
            GPUI_COMPONENT_LICENSE_APACHE,
        ),
        ThirdPartyLib::new(
            "gpui-router",
            "https://github.com/justjavac/gpui-router",
            "MIT",
            GPUI_ROUTER_LICENSE_MIT,
        ),
        ThirdPartyLib::new(
            "gpui-component-assets",
            "https://github.com/longbridge/gpui-component.git",
            "Apache-2.0",
            GPUI_COMPONENT_LICENSE_APACHE,
        ),
        ThirdPartyLib::new(
            "openharmony-ability",
            "https://github.com/harmony-contrib/openharmony-ability.git",
            "MIT OR Apache-2.0",
            OPENHARMONY_ABILITY_LICENSE,
        ),
        ThirdPartyLib::new(
            "openharmony-ability-derive",
            "https://github.com/harmony-contrib/openharmony-ability.git",
            "MIT OR Apache-2.0",
            OPENHARMONY_ABILITY_LICENSE,
        ),
        ThirdPartyLib::new(
            "openharmony-ability-plugin-files",
            "https://github.com/harmony-contrib/openharmony-ability.git",
            "MIT OR Apache-2.0",
            OPENHARMONY_ABILITY_LICENSE,
        ),
        ThirdPartyLib::new(
            "napi-ohos",
            "https://github.com/ohos-rs/ohos-rs",
            "MIT",
            NAPI_OHOS_LICENSE_MIT,
        ),
        ThirdPartyLib::new(
            "napi-derive-ohos",
            "https://github.com/ohos-rs/ohos-rs",
            "MIT",
            NAPI_DERIVE_OHOS_LICENSE_MIT,
        ),
        ThirdPartyLib::new(
            "napi-derive-backend-ohos",
            "https://github.com/ohos-rs/ohos-rs",
            "MIT",
            NAPI_DERIVE_BACKEND_OHOS_LICENSE_MIT,
        ),
        ThirdPartyLib::new(
            "ohos-hilog-binding",
            "https://crates.io/crates/ohos-hilog-binding",
            "MIT OR Apache-2.0",
            merge_license(&[
                OHOS_HILOG_BINDING_LICENSE_MIT,
                OHOS_HILOG_BINDING_LICENSE_APACHE,
            ]),
        ),
        ThirdPartyLib::new(
            "log",
            "https://github.com/rust-lang/log",
            "MIT OR Apache-2.0",
            merge_license(&[LOG_LICENSE_MIT, LOG_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "localsend",
            "https://github.com/localsend/localsend.git",
            "Apache-2.0",
            LOCALSEND_LICENSE,
        ),
        ThirdPartyLib::new(
            "tokio",
            "https://github.com/tokio-rs/tokio",
            "MIT",
            TOKIO_LICENSE,
        ),
        ThirdPartyLib::new(
            "serde",
            "https://github.com/serde-rs/serde",
            "MIT OR Apache-2.0",
            merge_license(&[SERDE_LICENSE_MIT, SERDE_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "serde_json",
            "https://github.com/serde-rs/json",
            "MIT OR Apache-2.0",
            merge_license(&[SERDE_JSON_LICENSE_MIT, SERDE_JSON_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "anyhow",
            "https://github.com/dtolnay/anyhow",
            "MIT OR Apache-2.0",
            merge_license(&[ANYHOW_LICENSE_MIT, ANYHOW_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "uuid",
            "https://github.com/uuid-rs/uuid",
            "Apache-2.0 OR MIT",
            merge_license(&[UUID_LICENSE_MIT, UUID_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "rcgen",
            "https://github.com/rustls/rcgen",
            "MIT OR Apache-2.0",
            RCGEN_LICENSE,
        ),
        ThirdPartyLib::new(
            "reqwest",
            "https://github.com/seanmonstar/reqwest",
            "MIT OR Apache-2.0",
            merge_license(&[REQWEST_LICENSE_MIT, REQWEST_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "futures-util",
            "https://github.com/rust-lang/futures-rs",
            "MIT OR Apache-2.0",
            merge_license(&[FUTURES_UTIL_LICENSE_MIT, FUTURES_UTIL_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "chrono",
            "https://github.com/chronotope/chrono",
            "MIT OR Apache-2.0",
            CHRONO_LICENSE,
        ),
        ThirdPartyLib::new(
            "hyper",
            "https://github.com/hyperium/hyper",
            "MIT",
            HYPER_LICENSE,
        ),
        ThirdPartyLib::new(
            "hyper-util",
            "https://github.com/hyperium/hyper-util",
            "MIT",
            HYPER_UTIL_LICENSE,
        ),
        ThirdPartyLib::new(
            "http-body-util",
            "https://github.com/hyperium/http-body",
            "MIT",
            HTTP_BODY_UTIL_LICENSE,
        ),
        ThirdPartyLib::new(
            "bytes",
            "https://github.com/tokio-rs/bytes",
            "MIT",
            BYTES_LICENSE,
        ),
        ThirdPartyLib::new(
            "base64",
            "https://github.com/marshallpierce/rust-base64",
            "MIT OR Apache-2.0",
            merge_license(&[BASE64_LICENSE_MIT, BASE64_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "tokio-rustls",
            "https://github.com/rustls/tokio-rustls",
            "MIT OR Apache-2.0",
            merge_license(&[TOKIO_RUSTLS_LICENSE_MIT, TOKIO_RUSTLS_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "rustls",
            "https://github.com/rustls/rustls",
            "Apache-2.0 OR ISC OR MIT",
            merge_license(&[
                RUSTLS_LICENSE_MIT,
                RUSTLS_LICENSE_APACHE,
                RUSTLS_LICENSE_ISC,
            ]),
        ),
        ThirdPartyLib::new(
            "if-addrs",
            "https://github.com/messense/if-addrs",
            "MIT OR BSD-3-Clause",
            merge_license(&[IF_ADDRS_LICENSE_MIT, IF_ADDRS_LICENSE_BSD]),
        ),
        ThirdPartyLib::new(
            "socket2",
            "https://github.com/rust-lang/socket2",
            "MIT OR Apache-2.0",
            merge_license(&[SOCKET2_LICENSE_MIT, SOCKET2_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "qrcode",
            "https://github.com/kennytm/qrcode-rust",
            "MIT OR Apache-2.0",
            merge_license(&[QRCODE_LICENSE_MIT, QRCODE_LICENSE_APACHE]),
        ),
        ThirdPartyLib::new(
            "urlencoding",
            "https://github.com/kornelski/rust_urlencoding",
            "MIT",
            URLENCODING_LICENSE,
        ),
        ThirdPartyLib::new(
            "rust-embed",
            "https://pyrossh.dev/repos/rust-embed",
            "MIT",
            RUST_EMBED_LICENSE,
        ),
    ]
}
