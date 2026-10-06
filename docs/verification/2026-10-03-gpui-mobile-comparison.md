# 手势修复位置与 gpui-mobile 对照

本报告记录 10 月 3 日的历史方案。用户要求不得修改 GPUI 核心后，核心补丁已撤回，手势修复已迁到 gpui-ohos；当前方案及验证见 [10 月 4 日平台层修复](2026-10-04-ohos-scroll.md)。

## 当时修复的位置（历史记录）

补丁位于 NearSend 自己的 `vendor` 目录。

| 层 | 文件 | 职责与修复 |
| --- | --- | --- |
| OHOS 平台输入 | [gpui-ohos/window.rs](../../vendor/gpui-ohos/src/ohos/window.rs) | 从 XComponent 读取触摸点、分配稳定且不复用的 TouchId，并把原生纳秒采样时间交给 GPUI |
| 共用手势识别器（历史补丁，已撤回） | `gpui/gestures.rs` | 速度估算、停住检测、方向锁定与惯性；用硬件采样间隔避免积压事件放大速度；抓停后的新触摸自行选择方向 |
| GPUI 帧调度（历史补丁，已撤回） | `gpui/window.rs` | 同一时间只排入一个惯性帧回调，避免反复抓停、重启产生多条回调链 |
| OHOS 帧与生命周期 | [gpui-ohos/platform.rs](../../vendor/gpui-ohos/src/ohos/platform.rs) 与 window.rs | 先处理可见性和 Surface 事件，再绘制待处理帧；隐藏或销毁的 Surface 不提交帧 |

拖动本身按坐标变化移动。抬手后需要估算每秒速度，时间间隔来自系统已有的触摸采样，不需要为采样新建定时任务。

## 对照的真实实现

检查的是 [longbridge/gpui-mobile](https://github.com/longbridge/gpui-mobile/tree/9075e3aa3eea812127f2c60ed66f0cd5798ff245)，它是 itsbalamurali/gpui-mobile 的 fork。读取的 revision 为 `9075e3aa3eea812127f2c60ed66f0cd5798ff245`，当前 Cargo manifest 使用 `gpui-pre = 0.3.7`。

该版本的 [Android 输入](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/android/window.rs#L1447) 和 [iOS 输入](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/ios/window.rs#L901) 都传递原始 `TouchEvent`，由 GPUI 共用识别器负责点击、长按、拖动和惯性。这与本次 OHOS 的分层相同。

两端额外使用 [FlingGuard](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/fling_guard.rs) 处理抓停惯性后继承旧轴的问题。它向 GPUI 发送一个合成触摸的 Started/Cancelled，再发送真实触摸；文件也说明这种绕法会失去“抓停后松手不能成为点击”的语义。

NearSend 可以修改共用识别器，因此直接在那里修复：抓停立即生效，下一次非零移动选择新轴，原来的抓停不误点击行为继续由原有测试验证。新增测试分别覆盖横向惯性后向上拖动、纵向惯性后横向拖动，以及选择新轴前的静止样本。修改前第一个方向输出零位移而失败，修改后两种方向都输出手指实际移动的 3 px。

## 不应混淆的旧模块

仓库还保留 [momentum.rs](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/momentum.rs)。这个可独立使用的模块用 `Instant::now()` 记录速度样本，并把单次惯性推进时间上限设为 33 ms。上面这版 Android/iOS 的平台输入路径已经转交 GPUI，没有调用该模块的 VelocityTracker 或 MomentumScroller。

所以 33 ms 是旧模块对长暂停的保护，不是当前平台路径的 30 fps 限制，也不能用于替代 OHOS 的原生采样时间修复。本次没有照搬这一常数或改换已验证的惯性曲线。

## 帧调度参考

[Android frame_source.rs](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/android/frame_source.rs) 由 GPUI 请求帧、AChoreographer 提供 VSync，事件循环在有需求且帧到期时才绘制。[iOS FrameDemand](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/frame_demand.rs) 合并同一帧的唤醒请求。

OHOS 使用自己的原生 VSync 与 Ability 主线程，采用相同的按需绘制原则。首页 Logo 已恢复原来的 `Animation::new(duration).repeat()`，没有人为限制帧率；可见性和 Surface 生命周期保护在平台层处理。
