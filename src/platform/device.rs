use napi_derive_ohos::napi;
use std::sync::{OnceLock, RwLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DeviceClass {
    Phone,
    Tablet,
    TwoInOne,
    #[default]
    Other,
}

impl DeviceClass {
    pub(crate) fn supports_system_file_picker(self) -> bool {
        matches!(self, Self::Tablet | Self::TwoInOne)
    }
}

static DEVICE_CLASS: OnceLock<RwLock<DeviceClass>> = OnceLock::new();

/// Reports the HarmonyOS product type before GPUI creates its first frame.
#[napi]
pub fn set_device_type(device_type: String) {
    if let Ok(mut current) = DEVICE_CLASS
        .get_or_init(|| RwLock::new(DeviceClass::Other))
        .write()
    {
        *current = classify_device_type(&device_type);
    }
}

pub(crate) fn current_device_class() -> DeviceClass {
    DEVICE_CLASS
        .get_or_init(|| RwLock::new(DeviceClass::Other))
        .read()
        .map(|current| *current)
        .unwrap_or_default()
}

fn classify_device_type(device_type: &str) -> DeviceClass {
    match device_type.trim().to_ascii_lowercase().as_str() {
        "phone" | "default" => DeviceClass::Phone,
        "tablet" => DeviceClass::Tablet,
        "2in1" | "pc" => DeviceClass::TwoInOne,
        _ => DeviceClass::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_tablet_and_two_in_one_support_system_file_picker() {
        assert!(classify_device_type("tablet").supports_system_file_picker());
        assert!(classify_device_type("2in1").supports_system_file_picker());
        assert!(classify_device_type("PC").supports_system_file_picker());
        assert!(!classify_device_type("phone").supports_system_file_picker());
        assert!(!classify_device_type("default").supports_system_file_picker());
        assert!(!classify_device_type("tv").supports_system_file_picker());
    }
}
