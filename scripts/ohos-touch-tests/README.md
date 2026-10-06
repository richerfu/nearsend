# OHOS framework regressions

This host harness compiles the exact production gesture, dispatcher, task queue,
frame state, render cache and capture conversion modules from `vendor/gpui-ohos`.
It uses the unmodified pinned GPUI futures/executors. Native UI wakes use a
counting stub and the host main queue uses the production queue facade. No OHOS
native libraries are linked. Simulator checks cover native input, VSync, frame
fallback, multiwindow operation and lifecycle integration separately.

33 regressions include cooperative worker reuse, blocking parents/children,
external work while base workers block, foreground/timer wake delivery,
shutdown, frame generation races, priority fairness and pixel conversion.

```sh
cargo test --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked
cargo clippy --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked --all-targets --no-deps -- -D warnings
cargo fmt --manifest-path scripts/ohos-touch-tests/Cargo.toml --check
```
