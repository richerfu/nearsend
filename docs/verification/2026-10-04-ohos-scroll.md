# OHOS 手势修复迁移与验证（1.2.1）

本报告为 10 月 4 日的原始触摸方案记录。10 月 6 日已用 XComponent 系统 Pan 替代平台滚动识别与速度采样，当前实现、验证及新测试包见 [系统 Pan 接入](2026-10-06-system-pan.md)。

## GPUI 核心保持上游原样

按用户要求，撤回本地 GPUI 核心中的时间戳字段、速度估算、方向锁定、惯性调度和嵌套滚动补丁。`vendor/gpui` 已移出工程；保留的历史副本在 `/tmp/nearsend-ohos-gestures/gpui-before`。

[Cargo.toml](../../Cargo.toml) 统一使用 `ohos-rs/zed` 的 `f45c7c22d0c04fb12527e71bbbee1dc6bdbdee0a`。Cargo.lock 只解析一个 GPUI，NearSend、gpui-ohos、gpui-component 和 gpui-router 共用它。上游 checkout 的受 Git 管理文件没有改动，仅有 Cargo 自动生成的 `.cargo-ok` 标记。见 [依赖树](evidence/2026-10-04-ohos-scroll/gpui-dependency-tree.txt) 和 [源码检查](evidence/2026-10-04-ohos-scroll/gpui-source-check.json)。

## 当前修复位置

| 文件 | 职责 |
| --- | --- |
| [gpui-ohos/touch_scroll.rs](../../vendor/gpui-ohos/src/ohos/touch_scroll.rs) | 使用公开 PlatformInput 实现 OHOS 点击、长按、平移和惯性；原生时间戳只保留在平台速度采样器中 |
| [gpui-ohos/window.rs](../../vendor/gpui-ohos/src/ohos/window.rs) | XComponent 输入、稳定 TouchId、IME 点击判断、单个可取消长按任务；在原生 VSync 推进惯性，隐藏/销毁时清理手势 |
| [gpui-ohos/platform.rs](../../vendor/gpui-ohos/src/ohos/platform.rs) | 声明平台识别 tap / long_press / pan；保留按需 VSync 和 Surface 生命周期保护 |
| [NearSend 协议页](../../src/ui/pages/open_source_licenses/page.rs) | 正文处理滚动后调用公开 `cx.stop_propagation()`，防止外层列表同时移动 |

先发送原始 Touch Started，让 GPUI 的控件有机会捕获直接拖动。被控件捕获的接触继续使用完整原始事件流；其余接触立即取消 GPUI 中的候选手势，交由 gpui-ohos 识别。平台点击发送 MouseDown/MouseUp，随后用空闲状态下的 Touch Cancelled 恢复触摸输入样式；长按发送公开的 LongPressEvent；平移发送 ScrollWheelEvent。抓停惯性后立即停止旧曲线，新接触自行选择方向，松手不会误点击或唤起键盘。

硬件采样间隔用于估算抬手速度，避免积压输入按过短的派发间隔计算出过大速度。惯性从抬手事件的派发时刻开始，避免第一帧跳过已经过去的时间。保持 8 个逻辑像素的识别门槛和 ArkUI 摩擦曲线。每个窗口只有一个惯性状态，在每个 VSync 推进一次；没有 GPUI 核心惯性回调链。拖动直接跟随位置，只有抬手速度计算需要采样时间；一次性计时任务用于长按。

参考 [gpui-mobile 的 FlingGuard](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/fling_guard.rs) 和 [Android 帧调度](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/android/frame_source.rs)。该版 Android/iOS 将原始触摸交给 GPUI，FlingGuard 用合成接触绕过旧轴继承，并注明会失去抓停后不误点击的语义。OHOS 此次在平台层保存该语义，未照搬这一绕法。10 月 3 日核心修改方案已失效，旧报告顶部已标明历史状态。

## 回归与构建

直接编译平台源码的 [宿主测试 harness](../../scripts/ohos-touch-tests/lib.rs) 共 **13 项通过**，覆盖：积压采样、停住后抬手、抓停后换轴、不误点击、控件拖动、长按捕获/拒绝/过期任务、点击次数、额外手指、Surface 取消、反复抓停、帧间隔变化和采样时钟缺失/重置。[测试日志](evidence/2026-10-04-ohos-scroll/touch-tests.log)。

```sh
cargo test --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked
cargo clippy --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked --all-targets --no-deps -- -D warnings
```

Clippy、根工程/平台/harness 的 rustfmt、git diff --check 通过。未修改的上游 `block 0.1.6` 有 Rust future compatibility 提示。使用 OHOS API 26 SDK 完成 ARM64 Rust 优化 Release、ArkTS Release HAP 和 APP 构建。

## 最终测试包的模拟器验证

安装到两台模拟器的 default 签名 Release HAP：

`264d19779d443396272f5366bdfdd0f2a3590a8d497117f4f2259b256b9ab831`

| 场景 | phone（Pura90，1320×2856） | 2in1（MateBookPro，3120×2080） |
| --- | --- | --- |
| 设置页慢拖上下、快甩上下 | 39 张连续截图，人工检查通过 | 32 张连续截图，人工检查通过 |
| 停住 350 ms 后抬手，上/下各一次 | 700 ms 后内容像素差 0 | 700 ms 后内容像素差 0 |
| 抓停惯性后松手 | 700 ms 后内容像素差 0 | 700 ms 后内容像素差 0 |
| 反复上下滑动 | 24 次；进程保持，无新增 fault 文件 | 16 次；进程保持，无新增 fault 文件 |
| 协议正文拖动 | 正文变化，标题及下方条目像素差 0 | 正文变化，标题及下方条目像素差 0 |
| 正文回到顶部后继续向下拖 | 两次边界拖动像素差 0，外层不动 | 两次边界拖动像素差 0，外层不动 |
| 在正文区域外拖动 | 外层列表正常移动 | 外层列表正常移动 |
| 首页、息屏 5 秒后恢复、Home 后恢复 | 同一进程，未新增应用/渲染服务 fault | 同一进程，未新增应用/渲染服务 fault |

两台模拟器为 HarmonyOS 7.0.0.106 / API 26 ARM64。连续截图中观察到顺序移动及正常边界停止，本次样本未观察到异常跳到顶部/底部。[phone 自动检查](evidence/2026-10-04-ohos-scroll/phone-scroll-checks.json)、[2in1 自动检查](evidence/2026-10-04-ohos-scroll/2in1-scroll-checks.json)、[phone 嵌套检查](evidence/2026-10-04-ohos-scroll/phone-nested-scroll.json)、[2in1 嵌套检查](evidence/2026-10-04-ohos-scroll/2in1-nested-scroll.json)。连续截图汇总见同目录的 `*-contact-sheet-*.jpg`。

撤回核心嵌套滚动补丁后的初始候选包曾复现正文和外层一起移动，见 `*-initial-candidate-nested-failure.png`。最终包的页面隔离处理已修复该场景。边界验证检查的是正文顶部，未宣称完整 Apache 正文底部也被遍历验证。

[首页 Logo](../../src/ui/components/logo.rs) 保持原来的 15 秒一圈、逐帧 `Animation::new(duration).repeat()`，没有 15/30 fps 上限；首页滚动仍保留。最终包首页动画运行约 20/21 秒并完成恢复检查，见 [phone 生命周期](evidence/2026-10-04-ohos-scroll/phone-lifecycle.json) 和 [2in1 生命周期](evidence/2026-10-04-ohos-scroll/2in1-lifecycle.json)。同一平台方案的初始候选包此前完成约 305/306 秒持续动画检查，但发生在页面隔离和点击输入样式恢复的最后调整之前，记录为 `*-initial-candidate-lifecycle.json`，不能算作最终包的五分钟验证。

## 签名产物与真机状态

| 产物 | 用途 / 校验 |
| --- | --- |
| [default Release HAP](../../dist/NearSend-1.2.1-ohos-scroll-default-release.hap) | 按用户指定的 default 签名安装测试；Rust/ArkTS Release、debug=false，签名 profile 类型为 debug |
| [publishrelease HAP](../../dist/NearSend-1.2.1-ohos-scroll-publishrelease.hap) | publishrelease 正式签名，profile 类型为 release |
| [publishrelease APP](../../dist/NearSend-1.2.1-ohos-scroll-publishrelease.app) | 正式签名 APP 产物，profile 类型为 release |

三份产物版本均为 **1.2.1 / 1002001**，通过 hap-sign-tool 签名校验。对照模拟器实际安装的 default HAP，正式产物中 ArkTS ABC 完全一致，原生 ELF 的 22 个有效分配节内容完全一致。见 [包校验](evidence/2026-10-04-ohos-scroll/package-verification.json) 和 [SHA-256](evidence/2026-10-04-ohos-scroll/packages.sha256)。正式构建后已恢复原始 build-profile 内容，默认产品仍选用 default 签名。

10 月 4 日验证结束时，HDC 只列出 `127.0.0.1:10001` 与 `127.0.0.1:10002`，最终包尚未安装真机，见 [当时安装状态](evidence/2026-10-04-ohos-scroll/installation.json)。模拟器验证不代表真机发热已经解决；此次没有重新采集 CPU/温度数据，也没有沿用旧的限制帧率性能数值。

10 月 6 日更新：用户卸载旧版后，同一 default 签名 Release HAP 已成功安装并启动于 ALN-AL00 真机；设备端核对版本为 1.2.1 / 1002001、debug=false。包 SHA-256 与上面的模拟器测试包一致，见 [真机安装记录](evidence/2026-10-06-physical-install/installation.json)。滚动与发热由用户继续验证。
