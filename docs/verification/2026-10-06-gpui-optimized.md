# gpui-ohos 优化交付（1.2.1）

## 范围与能力对齐

本轮优化 GPUI 的 OHOS 适配层，GPUI 核心与 NearSend 界面/业务逻辑保持原样。
系统 Pan、原始触摸、首次接受位移、系统速度和原有惯性实现未改动；
没有降低动画帧率、抗锯齿质量或限制窗口数量、文件选择、截图格式。
截图大小校验与优化前相同。前台任务预算仅拆分事件循环轮次，剩余任务
全部重新唤醒执行；后台线程可在队列停滞时扩展，没有固定并发上限。

已推送：

- gpui-ohos `feat/pr82-ability-adaptation`：[`b0cd09f`](https://github.com/ohos-rs/gpui-ohos/commit/b0cd09f4c8741cec1a25860a9fb7c6692a9411ad)。
- Ability `feat/pr82-nearsend-integration`：[`4493821`](https://github.com/richerfu/openharmony-ability/commit/4493821eb1234af66afa4b6571e9fb6558fea4cc)。
- NearSend 的运行时副本与发布分支逐字节一致，Cargo.lock 使用同一 Ability 分支提交。

## 已完成优化

1. 复用后台线程，按 GPUI 60/30/10 优先级服务；队列停滞时弹性扩展，
   多余线程空闲后退出。后台任务与计时器均显式唤醒 UI，停止空闲轮询。
2. 合并多窗口 UI 唤醒；帧请求采用原子状态及生命周期代次，阻止旧回调
   清除恢复后的新请求。健康 VSync 注销冗余的持续 XComponent 帧回调，
   失败时恢复该回调，保留兜底能力。
3. 复用主队列接收器，通过窗口索引直接路由借用的输入事件，减少克隆、
   全窗口扫描、堆分配及相同几何的重复处理。
4. 按设备缓存着色器、布局和兼容渲染管线；各窗口的缓冲、uniform 与
   surface 纹理保持原有所有权。截图逐行复制、转换，避免完整帧清零和多次扫描。
5. 多窗口测试复现了原有节点销毁顺序导致的 native 崩溃。Ability 改为
   先释放手势、回调和渲染消费者，再销毁 ArkUI 节点，关闭及重开已通过。
6. EntryAbility 初始避让区通知补充必需的 `windowId: 0`，与 SDK 类型对齐。

## 明确验证

- 两份生产源码主机测试均 **33/33**；Clippy `-D warnings`、fmt 通过。
  覆盖后台阻塞父子任务、外部任务推进、定时器、唤醒、清理、帧代次、
  手势所有权、管线缓存变体和 RGBA/RGBX/BGRA/BGRX 转换。
- ARM64 Rust Release、ArkTS Release 构建通过。Ability 核心通过
  `cargo check --tests`；macOS 无法链接其 OHOS NDK 单测，未将它计为执行通过。
- phone 与 2in1 的纯 GPUI 示例通过：空闲无持续帧事件、前后台恢复；
  2in1 主窗口加 3 个子窗口同时存在，独立输入、关闭、重开通过。
- 强制 native VSync 创建失败后，phone 与 2in1 的 XComponent 兜底、
  前后台恢复通过；2in1 的上述多窗口操作也通过。故障注入仅在临时测试副本。
- 最终无探针 HAP 在 phone 与 2in1 验证首次位移 **-12 物理像素**、
  按住/释放、快慢双向滚动、打断惯性、3 次前后台、息屏恢复；
  进程未变化，无新增应用 crash/appfreeze。
- NearSend 原有旋转动画保持：探针版本 phone 约 60 帧/秒，2in1 约 56.5
  帧/秒（模拟器负载影响）。静态设置页与后台不持续绘帧。

纯框架示例的对比：

| 指标 | 修复前 | 优化后 |
| --- | ---: | ---: |
| phone 静态 native redraw 事件/秒 | 64 | 0 |
| 2in1 静态 native redraw 事件/秒 | 63.291 | 0 |
| phone 静态单核 CPU | 1.098% | 未观测到 CPU tick |
| 2in1 静态单核 CPU | 1.649% | 未观测到 CPU tick |
| 主窗口 + 3 个兼容子窗口的管线组 | 4 | 1 |
| 2,064 次协作调度的线程 ID 数（主机） | 2,064 | 8 |
| 同负载调度耗时中位数（主机） | 31.893 ms | 2.616 ms |

CPU 使用 9 秒 procfs 窗口、100 tick/秒精度；未观测到 tick 不代表零功耗。
主机调度耗时不代表 OHOS 或真机整机提速。真机发热与主观手势体验仍需实际使用确认。

## 签名产物与安装

版本 **1.2.1 / 1002001**，Rust 与 ArkTS 均 Release，应用 `debug=false`。
三个包签名验证通过，22 个 Native 分配段及 ArkTS ABC 与模拟器测试 HAP 一致。

- `dist/NearSend-1.2.1-gpui-optimized-default-release.hap`：用户提供的 default 签名；
  已覆盖安装并启动于 ALN-AL00 真机。
- `dist/NearSend-1.2.1-gpui-optimized-publishrelease.hap`：正式 release profile。
- `dist/NearSend-1.2.1-gpui-optimized-publishrelease.app`：正式 APP。
- `dist/NearSend-1.2.1-gpui-optimized.sha256`：校验值。

性能探针、故障注入、临时本地 SDK patch 均未进入交付源码或包；签名配置与密钥未提交。

磁盘曾不足，已清理 15.86 GiB 可再生编译缓存。旧 phone 模拟器权限数据库
损坏并拒绝升级安装，因此保留原实例，在同型号、同分辨率/API 的独立
`NearSend Phone Perf QA` 实例验证，HDC 端口 10003；2in1 使用 10002。
真机保留同签名升级安装的数据。

结构化记录位于 [evidence/2026-10-06-gpui-optimized](evidence/2026-10-06-gpui-optimized/)。
截图、日志、临时框架示例和故障注入副本保存在
`/tmp/gpui-ohos-optimized-2026-10-06/`，不提交。
