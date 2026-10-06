# gpui-ohos

OpenHarmony platform backend for GPUI.

This crate is intentionally OHOS-only, so its implementation does not contain
`target_env = "ohos"` branches. Applications inject the platform explicitly,
following the same external-platform pattern as `gpui-mobile`:

```rust
let application = gpui::Application::with_platform(
    gpui_ohos::current_platform(openharmony_app, false),
);
```

The GPUI dependency is pinned to the `ohos-rs/zed` commit that exposes the
external-platform entry point and excludes OHOS from the Linux backend. All
OHOS windowing, input, rendering, IME, and gesture behavior lives here.

## Example

[`example`](./example) contains a minimal OpenHarmony native module based on
the NearSend integration: it keeps the embedded GPUI application alive, opens
one full-size window, and renders a centered greeting.

```bash
cd example
ohrs build --arch aarch
```

## License

[Apache-2.0](./LICENSE-APACHE) or [MIT](./LICENSE-MIT)
