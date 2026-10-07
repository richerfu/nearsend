# 适配层审计修复与验证（2026-10-07）

对应 [修复前审计](https://github.com/ohos-rs/gpui-ohos/blob/7553b7431da38471094d6677510f680a47132f9e/docs/audit-2026-10-07.md)。修改限于 gpui-ohos、openharmony-ability 与 NearSend 的依赖接入；GPUI 核心及 NearSend 业务源码未改。

## 实现

1. **Surface 与帧注册分离**：有效 Surface 先发布创建通知，再协调帧源。Ability 按窗口保存请求的模式及成功应用的模式；失败不记为成功，销毁使应用状态失效。移除 GPUI 的重复模式缓存。注册失败不会阻断首次启动，后续协调可重试。
2. **工作线程复用**：保留优先级队列、线程局部存储、同步阻塞及退出语义。队列暂未出队仅触发观察；从工作线程自身打开的 `/proc/thread-self/stat` 读取系统状态，R（运行中或等待 CPU）计入现有执行能力，S/D（系统等待）允许补充线程。没有积压时监控器睡眠；额外线程空闲后退出。创建补充线程失败会记录并重试，不再终止监控器。
3. **系统 Pan 分段**：每次 Start 以零累计偏移为基线，但立即交付系统 Start 中已有的位移；Update/End 必须属于已开始的手势。Cancel 即使缺少原始指针或 Pan 数据，也用上一条有效指针收尾并清空速度。Surface 创建、销毁及过期 owner 均清理手势状态。
4. **视口发布**：独立记录真正交付给 GPUI 的有效尺寸和 scale，在 Surface 创建、尺寸/键盘/配置变化时发布并去重。不再用渲染器已更新的 bounds 判断 GPUI 是否已收到通知。修正无 content rect 时逻辑尺寸到设备像素的换算，以及 scale 改变时逻辑尺寸的换算。

## 其他平台与 FFRT 取舍

| 平台 | 实际实现 |
| --- | --- |
| GPUI macOS | 系统 GCD，全局队列和任务优先级 |
| gpui-mobile iOS | 系统 GCD，全局队列和任务优先级 |
| GPUI Windows | 系统线程池 `TrySubmitThreadpoolCallback` |
| GPUI Linux | 按 CPU 数创建固定线程池，GPUI 优先级队列 |
| gpui-mobile Android | 固定后台线程池，后台任务暂不区分优先级；UI 通过 Looper 唤醒 |

核对 GPUI revision `f45c7c22d0`、gpui-mobile revision `9075e3aa3eea812127f2c60ed66f0cd5798ff245`。

FFRT 原型使用 API 20 起提供的 queue thread mode、default QoS、队列最大并发数 1024，避免将任意 Rust runnable 放入可迁移协程。结果：

| 环境 | 64 个 CPU 任务 | 16 个同步等待子任务的父任务 |
| --- | --- | --- |
| phone 原生进程 | 64 完成，峰值 4 | 仅 4 启动，12 秒内 0 完成 |
| 2in1 原生进程 | 64 完成，峰值 4 | 仅 4 启动，12 秒内 0 完成 |
| phone 应用进程 | 64 完成，峰值 4 | 仅 4 启动，12 秒内 0 完成 |
| 2in1 应用进程 | 64 完成，峰值 4 | 仅 4 启动，12 秒内 0 完成 |

应用测试超时后解除测试阻塞并清理队列，不挂死宿主 UI。结果证明这两个测试环境中的该接入方式不能直接替代当前执行器；不推断所有设备、所有 FFRT 配置都有此表现。FFRT 默认协程的 TLS 语义、队列销毁等待以及最低 API 支持也不能忽略。

因此采用 Linux 的线程复用/优先级队列结构，补充系统等待状态检查以保留已有同步阻塞能力。本次没有引入 FFRT 运行时依赖，也没有提高最低 API。若读取内核状态失败，保守保留补充线程的进度保障；该异常路径不承诺 CPU 并发优化。phone/2in1 应用实测该状态读取正常。

参考：[macOS](https://github.com/ohos-rs/zed/blob/f45c7c22d0/crates/gpui_macos/src/dispatcher.rs)、[Windows](https://github.com/ohos-rs/zed/blob/f45c7c22d0/crates/gpui_windows/src/dispatcher.rs)、[Linux](https://github.com/ohos-rs/zed/blob/f45c7c22d0/crates/gpui_linux/src/linux/dispatcher.rs)、[gpui-mobile Android](https://github.com/longbridge/gpui-mobile/blob/9075e3aa3eea812127f2c60ed66f0cd5798ff245/src/android/dispatcher.rs)、[FFRT queue API](https://github.com/openharmony/resourceschedule_ffrt/blob/master/interfaces/kits/c/queue.h)。

## 已完成验证

- 生产适配模块宿主回归：**37/37**；包括 CPU 长 poll、同步父子等待、外部任务推进、定时器、优先级、视口发布、帧状态和触摸状态。
- Ability 帧模式/Pan 状态回归：**4/4**。直接包含生产状态模块；宿主输入枚举是测试替身。完整 Ability 测试模块另通过 OHOS 三架构编译检查。独立 OHOS 测试可执行文件因应用运行库加载环境限制未能直接从 shell 运行，不计作设备单测通过。
- Ability workspace Clippy：aarch64、armv7、x86_64 OHOS，all-targets/all-features、`-D warnings` 通过。gpui-ohos 新增模块宿主严格 Clippy 通过。
- gpui-ohos 整库 OHOS 严格 Clippy 仍报告 **22 条既有 lint**（类型复杂度、冗余转换、可折叠 if 等）；逐条诊断源码行均存在于基线。本次引入的两条 lint 已修正，未用全局 allow 隐藏存量问题。完整 Release 编译通过。
- 同一主机 CPU 扩容探针：修复前基础 8、峰值/线程数 **30**；修复后均为 **8**。每个任务按墙钟时间计算 300 ms；该对比证明扩容纠正，不是等工作量吞吐对比。
- phone/2in1 **应用进程**：64 个 CPU 任务均保持基础/峰值/线程数 **4/4/4**；16 个同步阻塞父任务及 16 个子任务完成；全部基础线程阻塞时外部新任务继续推进。
- phone/2in1 原生错误注入：前两次帧模式配置强制返回错误，均能到达 GPUI 启动回调并重试成功，页面正常显示。
- phone/2in1：另强制首次 NativeVSync RequestFrame 失败，叠加前两次帧注册失败，均成功切换到 XComponent Continuous，原生回调至少推进 60 帧，页面正常显示。
- 2in1：主窗口与 3 个子窗口同时存在；每个子窗口输入、关闭、重开通过；主窗口最大化后有效 viewport 更新为 `1642.1052 × 992.1053 @ 1.9`。
- phone/2in1：各 3 次前后台恢复，PID 不变，无新增应用 fault。
- 隔离 GPUI 示例在任务完成后，9 秒采样 CPU 分别约单核 **0.66% / 0.11%**。不含 NearSend 业务；不作真机热量结论。

原生日志、测试夹具、截图位于 `/tmp/gpui-ohos-fixes-2026-10-07/`。框架故障包与最终正常 Release 包分开，故障注入和观测日志不进入生产源码。

## NearSend 接入回归

正常 default 签名 Release 包（无注入）通过两端首次短拖：注入 -12 设备像素，在按住期间页面已移动 -12 像素，图像匹配误差为 0，松手没有补跳。phone 的 350 ms 停留、抓停、双向慢滑/快滑通过；2in1 的 2 秒停留、抓停、双向慢滑/快滑通过，PID 不变且无新增应用 fault。

2in1 的 350 ms 停留检查在旧包与新包均观察到松手后的位移，200 ms 抓停也相同。该短停留项没有计作通过，不能用较长停留通过来替代。原始失败记录保留，未通过修改 GPUI 核心或新增超时判断改变其语义。诊断包在 350 ms 停留结束时记录到系统 Pan End 的 delta 为 0、速度约 -0.000266 设备像素/秒，低于惯性启动门槛；这一观测不支持“松手后启动惯性”的解释。可观察到系统批量交付 Update，因此自动截图尚不能区分迟到事件/画面呈现与额外滚动，需要结合事件消费和呈现时刻判断，不能把该像素差直接当作本轮回归。

## 验证边界

- viewport 特定“同焦点/原点，Surface 重建先改 bounds”的回归由生产发布模块测试与调用链检查覆盖；设备上覆盖了真实窗口尺寸改变、子窗口重开与前后台恢复。
- 缺失终止回调、无指针 Cancel 使用状态回归覆盖；不宣称已在真实系统中观测到所有缺失终止序列。
- 未进行新的真机温度测量；模拟器 CPU 与线程数不能替代真机功耗。

接入：gpui-ohos `feat/pr82-ability-adaptation` / `7553b7431da38471094d6677510f680a47132f9e`；Ability `feat/pr82-nearsend-integration` / `2a5d247c`。

验证包：`dist/NearSend-1.2.1-adapter-audit-fixed-default-release.hap`，使用用户提供的 default 签名；不含故障注入与手势观测代码。
