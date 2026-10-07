# 系统帧调度续修与短停留复验（2026-10-07）

## 本轮修复

`OhosPlatform::handle_ohos_event` 原来先执行前台任务，再交付输入，随后在输入回调内消费 pending VSync 并绘制。XComponent 原始接触和 ArkUI Pan 可能由系统在同一批次派发，这条路径会将同步绘制插入输入批次中。诊断日志实际记录到 raw/recognized 输入之间的绘制；慢 present 又使后续位移积压。

现已将主窗口和子窗口输入直接交付后返回。NativeVSync 仍由系统触发，原有 N-API 主线程唤醒发出 `UserEvent`，仅该回调消费 NativeVSync 待绘制帧。XComponent Continuous 回退仍由系统帧回调驱动。GPUI 的失效通知继续请求系统 VSync。

此改动保留主线程所有权、全部输入数据、系统 Pan 位移/速度、惯性、动画、多窗口和帧失败回退；未增加计时门槛、限帧或额外线程，也未修改 GPUI 核心及 NearSend 业务代码。系统本身也会批量处理 Move：可参考 [ArkUI PipelineContext 源码](https://gitee.com/openharmony/arkui_ace_engine/blob/d894d724f800e970f54073c13b20d0bd299854ee/frameworks/core/pipeline_ng/pipeline_context.cpp) 的 `FlushTouchEvents`。

同时修复整库 22 条既有 Clippy 错误：窗口构造参数由 `OhosWindowContext` 持有、回调类型命名、菜单快捷键反向查找，以及冗余转换/克隆和等价分支整理。字体名称规范化减少一次字符串分配。未添加 lint allow。

## 2in1 短停留问题的对照结论

上轮未通过的 350 ms 停留及 200 ms 抓停，不能归因为新增惯性：系统 Pan End 的最终 delta 为 0、速度接近 0。本轮进一步在同一诊断包中测量 `get_current_texture`、编码、提交和 `queue.present`，长耗时集中在 GLES present 阶段。

| 环境/运行 | present 中位数 | P95 | 短停留/抓停 |
| --- | ---: | ---: | --- |
| 2in1 重启前，同一诊断包 | 125.626 ms | 152.338 ms | 仍有失败 |
| 2in1 重启后，同一诊断包 | 6.935 ms | 13.236 ms | 全部通过 |
| phone，同一诊断方案 | 6.136 ms | 13.098 ms | 全部通过 |

随后在重启后的 2in1 安装**修复前的诊断包**，4 次 350 ms 停留及 200 ms 抓停也全部通过。显示尺寸、停留时间、截图差异阈值均未为通过检查而降低。因此，原先截图观察到的延后显示与模拟器显示链路状态相关；不能把重启带来的耗时下降算作本轮代码优化收益。输入批次中插入绘制是独立确认并修正的适配层问题。

尚未定位模拟器驱动内部具体阻塞函数，本次也没有修改驱动或声称修复系统驱动。所有首次失败记录保留。首次重启后启动等待不足和子窗口 INFO 日志断言不可用的测试脚本失败也保留；后者通过逐张审阅截图确认 A/B/C 均显示 `Clicks: 1` 和正确窗口状态，再继续完成恢复检查。

## 最终验证

- 生产模块宿主回归 **37/37**，宿主严格 Clippy 通过。
- gpui-ohos 整库 OHOS arm64、all-features、`--locked -- -D warnings` 通过；本轮 22 条存量错误已清零。修改文件 rustfmt 与 diff whitespace 检查通过。
- 正常 arm64 Release 构建通过。
- 正常包在 phone / 2in1：首次注入 -12 设备像素，按住时页面已移动 -12，图像拟合误差 0，释放不补跳。
- 两端各 **8 次 350 ms 停留**（双向各 4 次），释放后与 700 ms 后截图的内容区域平均像素差均为 **0**；**200 ms 抓停**差异也为 **0**。双向 500/3500 速度滑动采集完成；这些采样不等于逐帧帧率测试。
- 两端帧故障夹具：首次 NativeVSync 请求失败、前两次原生帧注册失败，启动和重试成功，Continuous 回调至少 60 帧。
- 2in1 主窗口 + 3 子窗口同时存在，每个子窗口实际点击计数 1、读取窗口状态、关闭、A 重开均通过。子窗口输入结果采用截图目视验证。
- 两端各 3 次后台/前台恢复，PID 保持，无新增应用 fault。
- 两端应用进程：64 个 CPU 任务保持基础/峰值/线程数 **4/4/4**；16 对同步等待的父子任务完成，基础线程全阻塞时外部任务继续推进。
- 无真机安装或温度测量；不将模拟器数据作为真机功耗结论。

原始日志、截图、诊断代码和执行脚本：`/tmp/gpui-ohos-followup-2026-10-07/`。正常包不含 `SCROLL_TRACE`、`AUDIT_QA`、`FFRT_APP_QA` 或 `GESTURE_AUDIT` 标记。

验证产物：`dist/NearSend-1.2.1-system-frame-default-release.hap`（用户提供的 default 签名）。SHA-256：`72d58f0adcfbc984e52fe44fb45d0c56e16441badf07bbaa08498f4f2727acea`。

结构化结果：[checks.json](evidence/2026-10-07-system-frame-followup/checks.json)。

接入：gpui-ohos `feat/pr82-ability-adaptation` / `d9b2f1a6c1c963bc6edc931da53acb425f22f604`；Ability 保持已锁定的 `2a5d247c`。
