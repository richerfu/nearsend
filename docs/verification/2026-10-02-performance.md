# NearSend 1.2.1 性能与手势验证

验证时间：2026-10-02 至 2026-10-03（Asia/Shanghai）。

后续更新：用户要求保留首页原动画，2026-10-03 已恢复逐帧旋转并继续修复滚动手势。最新实现、验证和产物请看[原动画恢复验证](2026-10-03-smooth-logo.md)。本文保留为上一轮静态 Logo 候选的历史记录。

## 改动

- 移除接收首页持续旋转的 Logo，静止页面不再因这个动画逐帧重绘。
- Release 编译移除 Debug 日志，并将运行日志级别设为 Info。
- 发送选择状态增加 revision；内容没有变化时，渲染不再复制文件列表和重复计算大小。
- 在 OHOS 基线 GPUI 上应用上游嵌套滚动修复：一次位移只消费一次，边界处沿真实父容器链传递。
- gpui-ohos 由 GPUI 的按需 VSync 调度绘制，避免 XComponent 回调重复推进滚动与提交帧；VSync 不可用时保留原生回调作为备用。

补丁来源与维护说明：[GPUI](../../vendor/gpui/README.nearsend.md)、[gpui-ohos](../../vendor/gpui-ohos/README.nearsend.md)。

## 环境与测量

| 模拟器 | 形态 | 分辨率 | 系统 |
| --- | --- | --- | --- |
| Pura 90 | phone | 1320 × 2856 | HarmonyOS 7.0.0.106 / API 26 |
| MateBook Pro | 2in1，浮动窗口 | 3120 × 2080 | HarmonyOS 7.0.0.106 / API 26 |

两台模拟器均为 ARM64、4 核、4 GB。测试的是 Rust release 与 ArkTS release 构建，使用用户提供的 default 签名；包内 debug=false。

基线为修改前的 `dist/NearSend-1.2.1-publishrelease.hap`。场景均为接收服务运行、接收首页静止。读取进程 `/proc/<pid>/stat` 的 CPU 时间，连续采样 3 × 10 秒；百分比按单核 100% 计算，包含整个应用进程。

| 场景 | 修改前 CPU | 最终候选包 CPU | 相对降低 |
| --- | ---: | ---: | ---: |
| phone 静止首页 | 19.337% | 1.129% | 94.2% |
| 2in1 静止首页 | 20.666% | 1.096% | 94.7% |
| phone 后台，10 秒 | — | 0.100% | — |
| 2in1 最小化，10 秒 | — | 0.100% | — |

原始数据：[证据目录](evidence/2026-10-02-performance/)。脚本：[measure_ohos_cpu.py](../../scripts/measure_ohos_cpu.py)。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| phone 设置页慢滑、快速滑动、反向滑动、到底部、触摸停止后继续滑动 | 通过；额外连续 24 次不同速度往返滚动，进程不变、截图成功、无新增故障日志 |
| 2in1 设置页同类操作 | 通过；额外连续 16 次往返滚动，进程不变、截图成功、无新增故障日志 |
| phone 与 2in1 开源协议长列表、展开协议后滚动 | 通过；内容移动，标题区保持固定，点击可打开条目 |
| 后台 / 最小化后恢复，再次滚动和打开页面 | 通过 |
| GPUI 嵌套滚动回归 | 4 项通过：按轴消费、横向嵌套、位移只消费一次、仅向真实祖先传递 |
| GPUI 手势回归 | 37 项通过，涵盖触摸识别、惯性与摩擦曲线等 |
| 发送选择状态 revision 回归 | 1 项通过；相同大小的文本修改仍能刷新，无效删除不增加 revision |
| Rust 格式、diff 空白检查、ARM64 release 编译 | 通过 |

连续滚动脚本和每次操作检查结果、测试输出均保存在证据目录。手势验证是模拟器功能验证，没有测量端到端输入延迟或掉帧率。

### 测试中出现的异常

在单一 VSync 补丁加入前的第一轮 phone 测试中，出现过一次 THREAD_BLOCK_6S 与系统 render_service SERVICE_BLOCK。应用堆栈阻塞在 EGL present → NativeWindowRequestBuffer；模拟器宿主日志同时出现空图形缓冲区错误。根因尚未独立确认，不能仅凭堆栈归因于模拟器。

加入调度补丁后重新安装最终候选包，完成上述连续滚动、长列表和前后台操作，未出现新的冻结 / 崩溃日志。这证明本轮操作通过，不能证明该偶发异常已永久消除。

## 产物与核验

- `dist/NearSend-1.2.1-performance-default-release.hap`：供真机验证，使用现有 default 签名。
- `dist/NearSend-1.2.1-performance-publishrelease.app`：使用 publishrelease 配置签名，Profile 类型 release。
- 同目录另有 publishrelease HAP 与 SHA-256 清单。

default HAP、publishrelease HAP 和 APP 均通过 hap-sign-tool verify-app。正式产物中的原生库全部 22 个已分配 ELF section、ArkTS ABC 均与模拟器测试包一致；版本为 1.2.1 / 1002001，debug=false。构建后产品签名选择已恢复为 default。

详细产物大小与哈希：[package-verification.json](evidence/2026-10-02-performance/package-verification.json)。

## 验证边界

模拟器结果确认了空闲 CPU 大幅下降，以及本轮手势功能通过。没有测量真机温度、功耗、大文件传输性能、120 Hz 屏幕体验或鼠标滚轮；真机发热与实际手感仍需用提供的 HAP 复核。
