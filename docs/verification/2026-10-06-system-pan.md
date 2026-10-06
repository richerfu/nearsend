# XComponent 系统 Pan 接入与验证

> 后续首次滑动延迟修复已应用系统 Pan Start 位移，见 [首次滑动验证](2026-10-06-pan-latency.md)。本文保留上一批产物及测量记录。

## 修复

用户反馈起步停顿或跳动，并要求使用系统事件。此次改动全部位于 gpui-ohos；GPUI 仍为未修改的上游 `f45c7c22d0c04fb12527e71bbbee1dc6bdbdee0a`。[源码检查](evidence/2026-10-06-system-pan/source-check.json)。

先前 `TouchInputDelivery::RawXComponent` 仅接收原始触摸，`window.rs` 忽略 ArkUI Gesture，平台用 8 个逻辑像素判断 Pan 并补发累计位移。现在启用 `TouchInputDelivery::Both`，滚动接入 XComponent 上已注册的系统 Pan：

- [platform.rs](../../vendor/gpui-ohos/src/ohos/platform.rs)：启用系统手势及原始控件输入。
- [window.rs](../../vendor/gpui-ohos/src/ohos/window.rs)：接入系统 Pan 的阶段、delta 和 velocity，原生 px / px/s 转换为 GPUI 逻辑单位一次。
- [touch_scroll.rs](../../vendor/gpui-ohos/src/ohos/touch_scroll.rs)：原始触摸不再识别滚动或估算速度；删除 VelocityTracker、采样历史、二次拟合和时间戳换算。Pan Start 建立基准，后续 Update 跟随系统 delta，起步不补发识别前的累计位移。

原始触摸仍用于控件直接拖动、点击/长按兼容和按下抓停。Raw Up 与 Pan End 可按任意顺序到达，不能误点击或重复启动惯性。Pan End 使用系统给出的速度，Swipe 不再启动另一条惯性。Pan 本身不发送抬手后的位移，因此保留一条由 VSync 推进的惯性曲线；惯性从派发时刻起算。这部分不是系统 Scroll 容器的自动滚动。

API 26 本地 `native_gesture.h` 明确注明：原生 Pan offset 单位是 px，velocity 单位是 px/s。现有 XComponent 系统识别器的 distance 为 8 个物理像素。先前平台门槛是 8 个逻辑像素，在 phone 的 3.5 倍密度下相当于 28 个物理像素。Native Pan 的门槛保留系统事件接入中已有的值，未采用此次试验过的自定义 3 像素方案。

## gpui-mobile 对照

核对了两个仓库的 main revision：

| 仓库 | 实际输入路径 |
| --- | --- |
| [itsbalamurali/gpui-mobile，c7cab3a](https://github.com/itsbalamurali/gpui-mobile/blob/c7cab3a43970bd5f1e05d907695404ed73fcdc95/src/ios/window.rs#L491) | 系统触摸输入，平台自己的 TouchState、VelocityTracker 和 MomentumScroller |
| [longbridge/gpui-mobile，9075e3a](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/ios/window.rs#L910) | 原始 TouchEvent 交给 GPUI 核心，并用 FlingGuard 处理抓停；Android 同样传递原始触摸 |

两者使用系统输入，不代表两者都由系统识别 Pan、估速和自动推进惯性。OHOS 此次直接使用 ArkUI 已识别的 Pan 及其速度。

## 明确验证

测试包：`NearSend-1.2.1-ohos-system-pan-default-release.hap`，SHA-256：

`e26a3169c9b21c1cfecef1a2f1c2244064b5376bba779937ece532c7e0310e0b`

在相同坐标、相同缓慢小幅拖动下对照旧包与新包，截图平移拟合误差均为 0：

| 模拟器 / 输入 | 旧包内容移动 | 系统 Pan 包内容移动 |
| --- | --- | --- |
| phone：20 个物理像素 | 0 | 12 个物理像素 |
| 2in1：14 个物理像素 | 0 | 6 个物理像素 |

见 [小幅拖动对照](evidence/2026-10-06-system-pan/micro-drag-comparison.json) 及同目录的 `*-micro-before/after.png`。结果符合系统 8 个物理像素识别门槛；剩余位移直接跟随输入，没有一次补回门槛距离。

两台模拟器均完成：

- 设置页慢拖上下、快甩上下：phone 39 帧、2in1 32 帧连续截图，人工检查顺序移动及正常边界停止，本次样本未见异常跳到顶部/底部。
- 停住 350 ms 后抬手、抓停后松手：700 ms 后内容像素差为 0。
- 反复滑动：phone 24 次、2in1 16 次，同一进程，无新增 fault 文件。
- 协议正文：正文变化，条目标题和后续条目像素差为 0；回到顶部后的继续拖动像素差为 0；在正文外拖动可移动父列表。见 `phone-nested.json`、`2in1-nested.json`。
- 原始首页动画、息屏 3 秒恢复、后台 2 秒再恢复：同一进程，无新增 fault 文件；首页仍为原来的逐帧动画，没有帧率上限。见 `*-lifecycle.json` 和 `*-home-final.png`。

以上 JSON 和连续截图汇总位于 [证据目录](evidence/2026-10-06-system-pan)。本次未测 CPU/温度，跟手改善的量化证据是起步位移响应，未宣称真机发热已解决。

宿主 harness 直接编译生产平台模块，16 项测试通过，涵盖系统接受后不补发初始位移、原始移动不滚动、系统速度、积压派发、停住后抬手、两种结束顺序、抓停换轴、控件捕获、长按、点击次数、额外手指、取消、帧间隔变化与无效速度。测试、Clippy、rustfmt、ARM64 优化 Release 构建均通过。见 [测试日志](evidence/2026-10-06-system-pan/tests.log)、[Clippy 日志](evidence/2026-10-06-system-pan/clippy.log)。

## 产物

- [default Release HAP](../../dist/NearSend-1.2.1-ohos-system-pan-default-release.hap)：真机测试，default 签名，应用 debug=false，签名 profile 类型为 debug。
- [publishrelease HAP](../../dist/NearSend-1.2.1-ohos-system-pan-publishrelease.hap)、[publishrelease APP](../../dist/NearSend-1.2.1-ohos-system-pan-publishrelease.app)：正式签名，profile 类型为 release。

三份产物均为 1.2.1 / 1002001，签名校验通过；正式产物的 22 个原生有效分配节与 ArkTS ABC 均和模拟器测试 HAP 一致。正式构建后恢复了原来的 default 签名选择。[包校验](evidence/2026-10-06-system-pan/package-verification.json)。

真机更新使用 default 包覆盖安装，安装与启动状态见 [真机记录](evidence/2026-10-06-system-pan/physical-installation.json)。滚动手感和发热由用户继续检查。
