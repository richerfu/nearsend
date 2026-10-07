# OHOS 优化交付核对（2026-10-07）

用户在最新真机包上反馈“看起来效果好多了”，随后要求整理提交并确认优化项。此次仅更新交付记录；运行时代码与已安装版本一致。

## 当前版本

| 仓库 | 分支 | 运行时提交 |
| --- | --- | --- |
| gpui-ohos | `feat/pr82-ability-adaptation` | `d9b2f1a6c1c963bc6edc931da53acb425f22f604` |
| openharmony-ability | `feat/pr82-nearsend-integration` | `2a5d247ccca859bf020bc34472ca22c948e9fa8c` |
| NearSend | `feat/ohos-1.2.1` | `3fc7a78` |

NearSend 的 22 个适配层源文件与 gpui-ohos 上述版本逐字节一致。Cargo.lock 中 Ability 的 git 来源统一锁定上述 richerfu 仓库分支和提交。GPUI 核心仍为原始 `f45c7c22d0c04fb12527e71bbbee1dc6bdbdee0a`。

## 实现状态

已完成：

- 后台线程复用、优先级队列、基于系统阻塞状态决定补充线程；CPU 计算不再触发盲目扩容。
- UI 唤醒合并、接收器复用、前台任务分批、计时器显式唤醒。
- 按需 NativeVSync、XComponent 帧源失败回退、生命周期代次保护；绘制经系统主队列交付，输入回调立即返回。
- 窗口索引与借用输入、设备级 GPU 资源缓存、逐行截图转换。
- native 消费者先于 RootNode 释放；Surface 通知与帧注册结果分离、失败可重试。
- Pan Start 基线重置及首次位移即时交付、Cancel/Surface 状态清理；按已发布值更新 viewport 和 scale。
- Ability 移除非 OHOS 平台实现；清理整库 22 条存量 Clippy 错误。

**以上已确认的优化及审计修复均已实现，但并非审计中的每条建议都已实施：**

- FFRT + 系统 QoS 替换未交付：所测配置未通过同步阻塞父子任务验证，当前保留可推进这些任务的线程复用方案。
- ArkUI Tap/LongPress 全面替换、系统 Scroll/Animator 全面接管惯性仍为待验证方案；当前系统 Pan 与原有兼容语义保留。
- **UI 唤醒入队结果与 session 所有权加固尚未完成**：Ability 的 `wake()` 未返回 TSFN 入队状态，仍读取全局当前 WAKER。唤醒合并和关闭处理不等同于该项完成。

逐项实现、取舍与验证范围见 gpui-ohos 的[完整核对清单](https://github.com/ohos-rs/gpui-ohos/blob/7cffba52fc4304a767b7626a69182708eaa5b895/docs/optimization-status-2026-10-07.md)。

## 验证与产物

- 生产模块宿主回归 **37/37**、严格 Clippy、正常 arm64 Release 构建通过。phone / 2in1 首次短拖、350 ms 停留、200 ms 抓停、前后台，以及框架故障夹具、多窗口检查通过；详细范围见[续验报告](2026-10-07-system-frame-followup.md)。
- 2026-10-07 11:44（UTC+8）已在 ALN-AL00 真机覆盖安装并启动，保留数据；**1.2.1 / 1002001**、default 签名、`debug=false`。用户体验反馈属于定性结果，未测量真机温度/功耗。
- 最新包为 `dist/NearSend-1.2.1-system-frame-default-release.hap`，SHA-256 为 `72d58f0adcfbc984e52fe44fb45d0c56e16441badf07bbaa08498f4f2727acea`。已再次核对本地文件摘要与安装记录一致。
- 最新修复未重新生成 publishrelease APP；2026-10-06 的正式 APP 是历史产物。

[真机安装记录](evidence/2026-10-07-system-frame-followup/physical-installation.json)。原始日志、截图与签名包留在本地；私有签名配置不进入提交。
