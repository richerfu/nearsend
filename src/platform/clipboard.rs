use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, LazyLock, RwLock};

use napi_derive_ohos::napi;
use napi_ohos::{
    bindgen_prelude::{Function, PromiseRaw, Unknown},
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    Error, Result, Status,
};
#[cfg(target_env = "ohos")]
use ohos_ability_access_control_binding::check_self_permission;
use openharmony_ability::OpenHarmonyApp;
use openharmony_ability_plugin_permission::PermissionExt as _;
use tokio::sync::oneshot;

type ReadClipboardTsfn = ThreadsafeFunction<(), Unknown<'static>, (), Status, false>;
type WriteClipboardTsfn = ThreadsafeFunction<String, Unknown<'static>, String, Status, false>;

static READ_CLIPBOARD_TSFN: LazyLock<RwLock<Option<Arc<ReadClipboardTsfn>>>> =
    LazyLock::new(|| RwLock::new(None));
static WRITE_CLIPBOARD_TSFN: LazyLock<RwLock<Option<Arc<WriteClipboardTsfn>>>> =
    LazyLock::new(|| RwLock::new(None));
static OHOS_APP: LazyLock<RwLock<Option<OpenHarmonyApp>>> = LazyLock::new(|| RwLock::new(None));

const READ_PASTEBOARD_PERMISSION: &str = "ohos.permission.READ_PASTEBOARD";
#[allow(dead_code)]
const WRITE_PASTEBOARD_PERMISSION: &str = "ohos.permission.WRITE_PASTEBOARD";

pub fn set_ohos_app(app: OpenHarmonyApp) {
    if let Ok(mut guard) = OHOS_APP.write() {
        guard.replace(app);
    }
}

#[napi]
pub fn register_clipboard_callbacks(
    read_clipboard: Function<'static, (), Unknown<'static>>,
    write_clipboard: Function<'static, String, Unknown<'static>>,
) -> Result<()> {
    let read_tsfn = read_clipboard
        .build_threadsafe_function()
        .callee_handled::<false>()
        .build()?;
    let write_tsfn = write_clipboard
        .build_threadsafe_function()
        .callee_handled::<false>()
        .build()?;

    {
        let mut guard = READ_CLIPBOARD_TSFN
            .write()
            .map_err(|_| Error::from_reason("failed to lock READ_CLIPBOARD_TSFN"))?;
        guard.replace(Arc::new(read_tsfn));
    }

    {
        let mut guard = WRITE_CLIPBOARD_TSFN
            .write()
            .map_err(|_| Error::from_reason("failed to lock WRITE_CLIPBOARD_TSFN"))?;
        guard.replace(Arc::new(write_tsfn));
    }

    Ok(())
}

fn get_read_clipboard_tsfn() -> Result<Arc<ReadClipboardTsfn>> {
    READ_CLIPBOARD_TSFN
        .read()
        .map_err(|_| Error::from_reason("failed to read READ_CLIPBOARD_TSFN"))?
        .as_ref()
        .map(Arc::clone)
        .ok_or_else(|| Error::from_reason("clipboard read callback is not registered"))
}

fn get_write_clipboard_tsfn() -> Result<Arc<WriteClipboardTsfn>> {
    WRITE_CLIPBOARD_TSFN
        .read()
        .map_err(|_| Error::from_reason("failed to read WRITE_CLIPBOARD_TSFN"))?
        .as_ref()
        .map(Arc::clone)
        .ok_or_else(|| Error::from_reason("clipboard write callback is not registered"))
}

fn get_ohos_app() -> Result<OpenHarmonyApp> {
    OHOS_APP
        .read()
        .map_err(|_| Error::from_reason("failed to read OHOS_APP"))?
        .as_ref()
        .cloned()
        .ok_or_else(|| Error::from_reason("OpenHarmony app is not initialized"))
}

fn has_permission(permission: &str) -> bool {
    #[cfg(target_env = "ohos")]
    {
        check_self_permission(permission)
    }

    #[cfg(not(target_env = "ohos"))]
    {
        let _ = permission;
        true
    }
}

pub fn has_read_clipboard_permission() -> bool {
    has_permission(READ_PASTEBOARD_PERMISSION)
}

async fn request_permissions(permissions: Vec<String>) -> Result<bool> {
    let app = get_ohos_app()?;
    let result = app.request_permission(permissions).await?;
    for item in result {
        if item.code != 0 {
            log::warn!("permission denied: {} code={}", item.permission, item.code);
            return Ok(false);
        }
    }

    Ok(true)
}

pub async fn ensure_read_clipboard_permission() -> Result<bool> {
    if has_read_clipboard_permission() {
        return Ok(true);
    }
    request_permissions(vec![READ_PASTEBOARD_PERMISSION.to_string()]).await
}

#[allow(dead_code)]
pub async fn ensure_write_clipboard_permission() -> Result<bool> {
    if has_permission(WRITE_PASTEBOARD_PERMISSION) {
        return Ok(true);
    }
    request_permissions(vec![WRITE_PASTEBOARD_PERMISSION.to_string()]).await
}

pub async fn read_clipboard_text() -> Result<String> {
    let read_tsfn = get_read_clipboard_tsfn()?;
    let (tx, rx) = oneshot::channel::<Result<String>>();

    let status = read_tsfn.call_with_return_value(
        (),
        ThreadsafeFunctionCallMode::NonBlocking,
        move |result, _| {
            match result {
                Ok(value) => {
                    let tx_cell = Rc::new(Cell::new(Some(tx)));
                    let tx_in_catch = tx_cell.clone();
                    let promise = unsafe { value.cast::<PromiseRaw<'static, String>>() }?;
                    promise
                        .then(move |ctx| {
                            if let Some(sender) = tx_cell.replace(None) {
                                let _ = sender.send(Ok(ctx.value));
                            }
                            Ok(())
                        })?
                        .catch(
                            move |ctx: napi_ohos::bindgen_prelude::CallbackContext<Unknown>| {
                                if let Some(sender) = tx_in_catch.replace(None) {
                                    let _ = sender.send(Err(ctx.value.into()));
                                }
                                Ok(())
                            },
                        )?;
                }
                Err(err) => {
                    let _ = tx.send(Err(err));
                }
            }
            Ok(())
        },
    );

    if status != Status::Ok {
        return Err(Error::from_reason(format!(
            "call read_clipboard callback failed with status: {:?}",
            status
        )));
    }

    rx.await
        .map_err(|_| Error::from_reason("read_clipboard callback receiver dropped"))?
}

pub async fn write_clipboard_text(text: String) -> Result<bool> {
    let write_tsfn = get_write_clipboard_tsfn()?;
    let (tx, rx) = oneshot::channel::<Result<bool>>();

    let status = write_tsfn.call_with_return_value(
        text,
        ThreadsafeFunctionCallMode::NonBlocking,
        move |result, _| {
            match result {
                Ok(value) => {
                    let tx_cell = Rc::new(Cell::new(Some(tx)));
                    let tx_in_catch = tx_cell.clone();
                    let promise = unsafe { value.cast::<PromiseRaw<'static, bool>>() }?;
                    promise
                        .then(move |ctx| {
                            if let Some(sender) = tx_cell.replace(None) {
                                let _ = sender.send(Ok(ctx.value));
                            }
                            Ok(())
                        })?
                        .catch(
                            move |ctx: napi_ohos::bindgen_prelude::CallbackContext<Unknown>| {
                                if let Some(sender) = tx_in_catch.replace(None) {
                                    let _ = sender.send(Err(ctx.value.into()));
                                }
                                Ok(())
                            },
                        )?;
                }
                Err(err) => {
                    let _ = tx.send(Err(err));
                }
            }
            Ok(())
        },
    );

    if status != Status::Ok {
        return Err(Error::from_reason(format!(
            "call write_clipboard callback failed with status: {:?}",
            status
        )));
    }

    rx.await
        .map_err(|_| Error::from_reason("write_clipboard callback receiver dropped"))?
}
