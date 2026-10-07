# Ability 仅保留 OHOS 实现

已推送 `feat/pr82-nearsend-integration` 提交
[`ab1d6c2`](https://github.com/richerfu/openharmony-ability/commit/ab1d6c20bdd135b4d713708b942d33f53634791a)。

- 删除 `configure_frame_callback` 的非 OHOS 错误桩，直接使用原 OHOS 实现。
- 删除帧回调和活动 owner 校验的 `target_env` 条件编译。
- 构建脚本删除 `CARGO_CFG_TARGET_ENV` 判断，保留 OHOS `desktop/mobile` 设备类型选择。
- 菜单测试只保留 `cfg(test)`；清理版本测试中旧平台门控注释。
- 按已有 IME 容器的主线程归属规则补充子窗口 IME 的局部 Clippy 注解，未改变其实现。

源码扫描：`crates` 与 `rust_example` 的 Rust/Cargo 文件中无
`target_env`、`target_os`、`target_family`、`target_arch` 或 `CARGO_CFG_TARGET` 条件。
功能 feature 与测试条件继续用于 OHOS 本身。

确认原 OHOS 帧回调函数体及活动 owner 校验逐字节未变；默认/mobile/desktop
设备类型输出相同；菜单运行时及具名 N-API 契约未改变。

验证通过：

- ARM64、ARMv7、x86_64 的 OHOS 工作区 Clippy，`--all-targets --all-features -D warnings`，
  按仓库 CI 排除 `webview_example`、`xcomponent_example`；包含单测编译，未计为设备单测执行。
- `cargo fmt --all --check`、`git diff --check`。
- NearSend 使用远端新提交的 ARM64 Rust Release 完整构建与链接。

gpui-ohos 依赖锁和示例 submodule 已同步到同一 SDK 提交，发布于
`feat/pr82-ability-adaptation` 的 `a76350c`。NearSend 依赖锁同步，
框架运行时源码未改动，其他依赖版本保持原样。

[结构化验证](evidence/2026-10-07-ability-ohos-only/source-check.json)。
原始构建日志保存在 `/tmp/openharmony-ability-ohos-only-2026-10-07/`。
