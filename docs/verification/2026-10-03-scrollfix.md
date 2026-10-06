# NearSend 1.2.1 滚动跳动修复与首页动画复测

后续用户要求恢复原首页动画，已移除本报告测试包中的 15 fps 限制并补充抓停后方向修复。最新记录见 [原动画恢复验证](2026-10-03-smooth-logo.md)。本报告 CPU 数据仅代表此前的限帧候选。

验证日期：2026-10-03。测试包：`NearSend-1.2.1-scrollfix-default-release.hap`。

## 定位与修复

GPUI 之前使用主线程处理事件的 `Instant::now()` 估算抬手速度。触摸事件排队后集中处理，会把真实采样间隔压缩，导致慢滑被计算为极快的惯性滚动。

新增回归测试中，硬件每 16 ms 移动 4 px（250 px/s），主线程每 100 µs 处理一条积压事件。修改前计算出的速度达到 8000 px/s；修改后保持 250 px/s，惯性首帧移动小于 5 px。向上、向下均覆盖。另一个测试确认：手指停住 200 ms 再抬起，积压的抬手事件不会误触发惯性。

现在 gpui-ohos 将 XComponent 原有的单调纳秒时间戳传给 GPUI；速度估算和停住检测使用真实采样间隔。惯性曲线仍从实际处理抬手事件的时刻启动，避免积压事件使第一帧跳过一段轨迹。缺少原生时间戳的平台沿用处理时间。

还修复了重复惯性回调：下一帧前连续抓停并重新启动三个 fling，修改前排入三个回调，现在只排入一个。后续帧仍正常推进惯性。

这证明了两条代码路径的缺陷；未声称已经捕获用户真机上每一次偶发闪烁的完整事件记录。

## 为什么需要采样时间

手指拖动时按坐标差移动内容；松手后的惯性速度需要位移与采样时间间隔。使用系统已有的触摸时间，不增加周期性计时任务。

| 平台 | 常见实现 |
| --- | --- |
| Android | `VelocityTracker` 接收 `MotionEvent`，根据采样计算速度。[官方接口](https://developer.android.com/reference/android/view/VelocityTracker) |
| iOS | `UIPanGestureRecognizer.velocity(in:)` 提供每秒点数的速度。[官方接口](https://developer.apple.com/documentation/uikit/uipangesturerecognizer/velocity(in:)) |
| macOS 触控板 | 系统提供逐渐衰减的惯性滚动事件和 `momentumPhase`。[官方说明](https://developer.apple.com/documentation/appkit/nsevent/momentumphase) |

## 首页动画与性能

恢复接收首页 Logo 原有的 15 秒一圈旋转，保留动画开关与接收服务运行状态的控制。仅将 Logo 重绘上限设为 15 fps，每次转动约 1.6°；手势滚动没有这个帧率限制。

两台模拟器的首页截图对比确认 Logo 角度变化。最终包静止在旋转首页，连续测量 3 × 10 秒；百分比按一个 CPU 核心 100% 计算，包含整个应用进程。

| 场景 | 优化前旋转首页 | 本次旋转首页 |
| --- | ---: | ---: |
| phone | 19.337% | 4.553% |
| 2in1 | 20.666% | 5.185% |
| phone 后台，10 秒 | — | 0.100% |
| 2in1 最小化，10 秒 | — | 0.100% |

30 fps 中间候选仍有较高开销，最终包采用 15 fps。证据目录中的 `phone-home-cpu.json`、`2in1-home-cpu.json` 是该中间候选；`final-*-home-cpu.json` 才是最终包。前一轮静态 Logo 的约 1.1% 数据不代表本次恢复动画后的性能。

## 验证结果

环境与前一轮一致：Pura 90 phone（1320 × 2856）与 MateBook Pro 2in1（3120 × 2080），ARM64，HarmonyOS 7.0.0.106 / API 26。Rust 与 ArkTS 均为 Release，`debug=false`。

| 检查 | 结果 |
| --- | --- |
| GPUI 库测试 | 348 项全部通过，包含新增三项回归、嵌套滚动与原有手势/动画测试 |
| 设置页慢滑、快滑、反向滑动 | 两端连续截图人工检查，本轮未观察到异常跳回；保留接触表与操作结果 |
| 滑动后停住 350 ms 再抬手，向上与向下 | 两端抬手后与 700 ms 后的内容区域像素差均为 0 |
| 触摸抓停惯性后抬手 | 两端内容区域像素差均为 0 |
| 额外连续滑动 | phone 24 次、2in1 16 次，进程保持不变，无新增故障日志 |
| 开源协议长列表与展开 GPUI 协议 | 两端展开成功；慢滑正常，停住后抬手的内容区域像素差均为 0 |
| 前后台 / 最小化恢复 | 两端保持原进程，恢复首页显示 |
| 格式与构建 | Rust 格式、diff 空白检查、ARM64 Release 构建通过 |

本轮手势检查使用真实模拟器输入与截图，不仅检查进程是否存活。连续截图属于采样检查，未测量真机 120 Hz 的每一帧或排除所有偶发情况。真机温度与手感仍由用户复核。

脚本：[verify_ohos_scroll.py](../../scripts/verify_ohos_scroll.py)。测试输出、CPU 数据、像素对比结果与截图：[证据目录](evidence/2026-10-03-scrollfix/)。

## 产物

- `dist/NearSend-1.2.1-scrollfix-default-release.hap`：用户提供的 default 签名，供真机安装。
- `dist/NearSend-1.2.1-scrollfix-publishrelease.app`：publishrelease 签名，Profile 类型 release。
- 同目录保留 publishrelease HAP 与 SHA-256 清单。

三份产物均通过 `hap-sign-tool verify-app`；APP 包的签名在外层验证。版本为 1.2.1 / 1002001，`debug=false`。原生库 22 个已分配且有文件内容的 ELF section 与模拟器测试包一致，ArkTS ABC 完全一致。产品签名配置已经恢复 default。

详细记录：[package-verification.json](evidence/2026-10-03-scrollfix/package-verification.json)。
