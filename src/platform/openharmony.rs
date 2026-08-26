use std::future::Future;
use std::pin::Pin;
use std::sync::{LazyLock, RwLock};

use napi_derive_ohos::napi;
use napi_ohos::{Error, Result};
use openharmony_ability::{
    impl_bridge_napi_type, AsyncBridge, BridgeCallOptions, BridgeContextRequirement, BridgePlugin,
    OpenHarmonyApp,
};

static OPENHARMONY_APP: LazyLock<RwLock<Option<OpenHarmonyApp>>> =
    LazyLock::new(|| RwLock::new(None));

pub struct NearSendPlatformBridgePlugin;

impl BridgePlugin for NearSendPlatformBridgePlugin {
    type Mode = AsyncBridge;

    const ID: &'static str = "nearsend.platform";
    const REQUIRED_CONTEXTS: &'static [BridgeContextRequirement] =
        &[BridgeContextRequirement::Ability];
}

#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct EmptyRequest {}

impl_bridge_napi_type!(EmptyRequest, "nearsend.platform.EmptyRequest");

#[napi(object)]
#[derive(Clone, Debug)]
pub struct ClipboardWriteRequest {
    pub text: String,
}

impl_bridge_napi_type!(
    ClipboardWriteRequest,
    "nearsend.platform.ClipboardWriteRequest"
);

#[napi(object)]
#[derive(Clone, Debug)]
pub struct ClipboardReadResponse {
    pub text: String,
}

impl_bridge_napi_type!(
    ClipboardReadResponse,
    "nearsend.platform.ClipboardReadResponse"
);

#[napi(object)]
#[derive(Clone, Debug)]
pub struct AcceptedResponse {
    pub accepted: bool,
}

impl_bridge_napi_type!(AcceptedResponse, "nearsend.platform.AcceptedResponse");

#[napi(object)]
#[derive(Clone, Debug)]
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub struct SaveFileRequest {
    pub file_name: String,
}

impl_bridge_napi_type!(SaveFileRequest, "nearsend.platform.SaveFileRequest");

#[napi(object)]
#[derive(Clone, Debug)]
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub struct UriResponse {
    pub uri: String,
}

impl_bridge_napi_type!(UriResponse, "nearsend.platform.UriResponse");

#[napi(object)]
#[derive(Clone, Debug)]
#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub struct OpenFileRequest {
    pub uri: String,
}

impl_bridge_napi_type!(OpenFileRequest, "nearsend.platform.OpenFileRequest");

pub fn set_app(app: OpenHarmonyApp) -> Result<()> {
    OPENHARMONY_APP
        .write()
        .map_err(|_| Error::from_reason("failed to lock OpenHarmony app state"))?
        .replace(app);
    Ok(())
}

pub fn app() -> Result<OpenHarmonyApp> {
    OPENHARMONY_APP
        .read()
        .map_err(|_| Error::from_reason("failed to read OpenHarmony app state"))?
        .as_ref()
        .cloned()
        .ok_or_else(|| Error::from_reason("OpenHarmony app is not initialized"))
}

#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
fn validate_non_empty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(Error::from_reason(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub trait NearSendPlatformExt {
    fn read_clipboard_text(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;

    fn write_clipboard_text(
        &self,
        text: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send>>;

    fn pick_save_file(
        &self,
        file_name: String,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;

    fn open_file(&self, uri: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

impl NearSendPlatformExt for OpenHarmonyApp {
    fn read_clipboard_text(&self) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let bridge = self.bridge();
        Box::pin(async move {
            let response = bridge?
                .call_async::<NearSendPlatformBridgePlugin, EmptyRequest, ClipboardReadResponse>(
                    "read-clipboard",
                    EmptyRequest::default(),
                    BridgeCallOptions::default(),
                )
                .await?;
            Ok(response.text)
        })
    }

    fn write_clipboard_text(
        &self,
        text: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send>> {
        let bridge = self.bridge();
        Box::pin(async move {
            let response = bridge?
                .call_async::<
                    NearSendPlatformBridgePlugin,
                    ClipboardWriteRequest,
                    AcceptedResponse,
                >(
                    "write-clipboard",
                    ClipboardWriteRequest { text },
                    BridgeCallOptions::default(),
                )
                .await?;
            Ok(response.accepted)
        })
    }

    fn pick_save_file(
        &self,
        file_name: String,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let validation = validate_non_empty(&file_name, "file_name");
        let bridge = self.bridge();
        Box::pin(async move {
            validation?;
            let response = bridge?
                .call_async::<NearSendPlatformBridgePlugin, SaveFileRequest, UriResponse>(
                    "pick-save-file",
                    SaveFileRequest { file_name },
                    BridgeCallOptions::default().with_timeout_ms(60_000),
                )
                .await?;
            Ok(response.uri)
        })
    }

    fn open_file(&self, uri: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        let validation = validate_non_empty(&uri, "uri");
        let bridge = self.bridge();
        Box::pin(async move {
            validation?;
            let response = bridge?
                .call_async::<NearSendPlatformBridgePlugin, OpenFileRequest, AcceptedResponse>(
                    "open-file",
                    OpenFileRequest { uri },
                    BridgeCallOptions::default(),
                )
                .await?;
            if response.accepted {
                Ok(())
            } else {
                Err(Error::from_reason(
                    "OpenHarmony rejected the open-file request",
                ))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_non_empty, AcceptedResponse, ClipboardReadResponse, ClipboardWriteRequest,
        EmptyRequest, OpenFileRequest, SaveFileRequest, UriResponse,
    };
    use openharmony_ability::BridgeNapiType;

    #[test]
    fn platform_plugin_uses_stable_named_napi_contracts() {
        assert_eq!(
            <EmptyRequest as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.EmptyRequest"
        );
        assert_eq!(
            <ClipboardWriteRequest as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.ClipboardWriteRequest"
        );
        assert_eq!(
            <ClipboardReadResponse as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.ClipboardReadResponse"
        );
        assert_eq!(
            <AcceptedResponse as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.AcceptedResponse"
        );
        assert_eq!(
            <SaveFileRequest as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.SaveFileRequest"
        );
        assert_eq!(
            <UriResponse as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.UriResponse"
        );
        assert_eq!(
            <OpenFileRequest as BridgeNapiType>::TYPE_NAME,
            "nearsend.platform.OpenFileRequest"
        );
    }

    #[test]
    fn platform_plugin_rejects_empty_path_inputs() {
        assert!(validate_non_empty("receive.png", "file_name").is_ok());
        assert!(validate_non_empty("file://bundle/path", "uri").is_ok());
        assert!(validate_non_empty("  ", "uri").is_err());
    }
}
