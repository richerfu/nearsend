# OHOS touch regressions

This host harness compiles the exact `vendor/gpui-ohos/src/ohos/touch_scroll.rs`
module against the unmodified, pinned GPUI git dependency. It does not link
OHOS native libraries. Simulator checks cover XComponent input, VSync and
window lifecycle integration separately.

```sh
cargo test --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked
cargo clippy --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked --all-targets --no-deps -- -D warnings
```
