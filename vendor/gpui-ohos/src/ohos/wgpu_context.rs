use anyhow::Context as _;
use std::sync::Arc;
#[cfg(feature = "gles")]
use wgpu::hal::Instance as _;

#[cfg(not(any(feature = "gles", feature = "vulkan")))]
compile_error!("enable at least one graphics backend: `gles` or `vulkan`");

pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    dual_source_blending: bool,
}

impl WgpuContext {
    pub fn new() -> anyhow::Result<Self> {
        let device_id_filter = match std::env::var("ZED_DEVICE_ID") {
            Ok(val) => match parse_pci_id(&val)
                .context("Failed to parse device ID from `ZED_DEVICE_ID` environment variable")
            {
                Ok(device_id) => Some(device_id),
                Err(error) => {
                    log::error!("{error:#}");
                    None
                }
            },
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => {
                log::error!(
                    "Failed to read value of `ZED_DEVICE_ID` environment variable: {error}"
                );
                None
            }
        };

        let mut default_backends = wgpu::Backends::empty();
        #[cfg(feature = "gles")]
        default_backends.insert(wgpu::Backends::GL);
        #[cfg(feature = "vulkan")]
        default_backends.insert(wgpu::Backends::VULKAN);
        let backends = match wgpu::Backends::from_env() {
            Some(configured_backends) if configured_backends.is_empty() => {
                log::warn!(
                    "WGPU_BACKEND is set but no valid backend was parsed; falling back to {:?}",
                    default_backends
                );
                default_backends
            }
            Some(configured_backends) => configured_backends,
            None => default_backends,
        };

        log::info!("OHOS WGPU backends configured: {:?}", backends);
        let mut instance_flags = wgpu::InstanceFlags::from_env_or_default();
        let validation_override_present = std::env::var_os("WGPU_VALIDATION").is_some();
        let debug_override_present = std::env::var_os("WGPU_DEBUG").is_some();
        if !validation_override_present && !debug_override_present {
            instance_flags.remove(wgpu::InstanceFlags::VALIDATION | wgpu::InstanceFlags::DEBUG);
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags: instance_flags,
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });

        let adapter = smol::block_on(Self::select_adapter(&instance, device_id_filter))?;

        log::info!(
            "Selected GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let dual_source_blending_available = false;

        let mut required_features = wgpu::Features::empty();
        if dual_source_blending_available {
            required_features |= wgpu::Features::DUAL_SOURCE_BLENDING;
        } else {
            log::warn!(
                "Dual-source blending not available on this GPU. \
                Subpixel text antialiasing will be disabled."
            );
        }

        let (device, queue) = smol::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("gpui_device"),
                required_features,
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits())
                    .using_alignment(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            }),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create wgpu device: {e}"))?;

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            dual_source_blending: dual_source_blending_available,
        })
    }

    async fn select_adapter(
        instance: &wgpu::Instance,
        device_id_filter: Option<u32>,
    ) -> anyhow::Result<wgpu::Adapter> {
        #[cfg(feature = "gles")]
        if let Some(hal_instance) = unsafe { instance.as_hal::<wgpu::hal::api::Gles>() } {
            // SAFETY: The exposed adapters are enumerated from the HAL instance owned by
            // this exact wgpu instance and are immediately imported back into it.
            let mut adapters = unsafe { hal_instance.enumerate_adapters(None) };
            anyhow::ensure!(!adapters.is_empty(), "No OpenGL ES adapters found");

            let selected_index = device_id_filter
                .and_then(|device_id| {
                    adapters
                        .iter()
                        .position(|adapter| adapter.info.device == device_id)
                })
                .unwrap_or(0);
            let mut adapter = adapters.swap_remove(selected_index);

            // OpenHarmony's GLES compatibility layer reports GL_MAX_VARYING_COMPONENTS
            // using a non-component unit. wgpu-hal divides that value by four and exposes
            // only 7 variables on the emulator, although GLES 3 guarantees at least 15.
            // Correct the imported adapter metadata in the platform adaptation layer.
            let webgl2_minimum =
                wgpu::Limits::downlevel_webgl2_defaults().max_inter_stage_shader_variables;
            if adapter.capabilities.limits.max_inter_stage_shader_variables < webgl2_minimum {
                log::warn!(
                    "Correcting OpenHarmony GLES max_inter_stage_shader_variables from {} to {}",
                    adapter.capabilities.limits.max_inter_stage_shader_variables,
                    webgl2_minimum
                );
                adapter.capabilities.limits.max_inter_stage_shader_variables = webgl2_minimum;
            }

            // SAFETY: `adapter` was created by `hal_instance`, which is the internal
            // GLES instance backing `instance`.
            return Ok(unsafe { instance.create_adapter_from_hal(adapter) });
        }

        if let Some(device_id) = device_id_filter {
            let adapters: Vec<_> = instance.enumerate_adapters(wgpu::Backends::all()).await;

            if adapters.is_empty() {
                anyhow::bail!("No GPU adapters found");
            }

            let mut non_matching_adapter_infos: Vec<wgpu::AdapterInfo> = Vec::new();

            for adapter in adapters.into_iter() {
                let info = adapter.get_info();
                if info.device == device_id {
                    log::info!(
                        "Found GPU matching ZED_DEVICE_ID={:#06x}: {}",
                        device_id,
                        info.name
                    );
                    return Ok(adapter);
                } else {
                    non_matching_adapter_infos.push(info);
                }
            }

            log::warn!(
                "No GPU found matching ZED_DEVICE_ID={:#06x}. Available devices:",
                device_id
            );

            for info in &non_matching_adapter_infos {
                log::warn!(
                    "  - {} (device_id={:#06x}, backend={})",
                    info.name,
                    info.device,
                    info.backend
                );
            }
        }

        instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to request GPU adapter: {e}"))
    }

    pub fn supports_dual_source_blending(&self) -> bool {
        self.dual_source_blending
    }
}

fn parse_pci_id(id: &str) -> anyhow::Result<u32> {
    let mut id = id.trim();

    if id.starts_with("0x") || id.starts_with("0X") {
        id = &id[2..];
    }
    let is_hex_string = id.chars().all(|c| c.is_ascii_hexdigit());
    let is_4_chars = id.len() == 4;
    anyhow::ensure!(
        is_4_chars && is_hex_string,
        "Expected a 4 digit PCI ID in hexadecimal format"
    );

    u32::from_str_radix(id, 16).context("parsing PCI ID as hex")
}

#[cfg(test)]
mod tests {
    use super::parse_pci_id;

    #[test]
    fn test_parse_device_id() {
        assert!(parse_pci_id("0xABCD").is_ok());
        assert!(parse_pci_id("ABCD").is_ok());
        assert!(parse_pci_id("abcd").is_ok());
        assert!(parse_pci_id("1234").is_ok());
        assert!(parse_pci_id("123").is_err());
        assert_eq!(
            parse_pci_id(&format!("{:x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:X}", 0x1234)).unwrap(),
        );

        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
    }
}
