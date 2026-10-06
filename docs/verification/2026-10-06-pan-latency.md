# 首次滑动延迟修复

## 原因与修复

系统已经接受 Pan 时，gpui-ohos 将 Start 包里的位移替换为 0。紧接着系统还可能发送一个位移为 0 的 Update；页面必须等后续非零 Update 才能移动。短距离滑动甚至可能一直不移动。这是接受之后额外丢失一笔位移，不是必须等待某个时间才能开始滚动。

修复在 [touch_scroll.rs](../../vendor/gpui-ohos/src/ohos/touch_scroll.rs)：Start 立即应用系统提供的 delta，并按本次手势的主轴过滤；随后 Update 继续使用系统增量，不重复累计首包。识别、8 个物理像素的系统门槛、系统速度及惯性实现均未改动。没有加入时间等待、原始触摸位移计算或速度采样。

GPUI 核心仍为未修改的 `f45c7c22d0c04fb12527e71bbbee1dc6bdbdee0a`。window.rs 与本次修复前逐字节一致，底部避让修复保留；首页动画及滚动未改。诊断日志已从生产代码移除。[源码核对](evidence/2026-10-06-pan-latency/source-check.json)。

## 明确验证

使用相同的 12 个物理像素短滑，移动后保持按住再抬手，通过截图平移拟合测量页面位移：

| 模拟器 | 修复前页面移动 | 修复后诊断包 | 最终 Release 包 |
| --- | --- | --- | --- |
| phone，1320×2856 | 4 px | 12 px | 12 px |
| 2in1，3120×2080 | 0 px | 12 px | 12 px |

所有平移拟合误差为 0。保持按住到抬手后的页面截图差异为 0，没有抬手补跳。旧版 phone 样本后续包仍产生了部分位移；首包丢失导致的短滑缺失在两端都复现。[对照测量](evidence/2026-10-06-pan-latency/first-accept-comparison.json)、[phone Release 检查](evidence/2026-10-06-pan-latency/phone-scroll-checks.json)、[2in1 Release 检查](evidence/2026-10-06-pan-latency/2in1-scroll-checks.json)。

诊断包日志确认两端系统 Start 均携带 -12 px，紧接着的首个 Update 为 0。2in1 旧版短滑没有触发绘制，新版开始绘制距 Start 约 18 ms；这是单次模拟器样本，不作为真机延迟指标。日志保存在同目录的 `baseline/fixed-*-pan-events.log`。

最终无诊断日志的 default Release 包在两端均完成：

- 首次短滑、上下慢拖和快甩。phone 捕获 38 帧、2in1 捕获 32 帧，5 张连续画面拼图已逐张检查，本次样本未见异常跳到顶部或底部。
- 移动后按住 350 ms 再松手、按下打断惯性：700 ms 后页面像素差异均为 0。
- 检查期间 PID 不变，没有新增故障记录。

Host 回归先复现 2 项失败，再通过全部 17 项测试，覆盖首包立即移动、没有 Update 的短 Pan、不重复首包及原有点击/惯性状态流转。Clippy、三个 workspace 的 fmt check、diff check、ARM64 优化 Release 和 ArkTS Release 构建均通过。对应日志及连续画面在 [证据目录](evidence/2026-10-06-pan-latency)。

## 产物与真机

- [default Release HAP](../../dist/NearSend-1.2.1-ohos-pan-latency-default-release.hap)
- [publishrelease HAP](../../dist/NearSend-1.2.1-ohos-pan-latency-publishrelease.hap)
- [publishrelease APP](../../dist/NearSend-1.2.1-ohos-pan-latency-publishrelease.app)

版本为 **1.2.1 / 1002001**，三份包的 debug 标记均为 false。default 为用户提供的调试签名配置，Rust 和 ArkTS 均为 Release 编译。publishrelease 产物使用 release profile；三份签名校验通过，22 个原生有效分配节和 ArkTS ABC 均与模拟器测试包一致。[包校验](evidence/2026-10-06-pan-latency/package-verification.json)。

default 包 SHA-256：`048eb67dd92b81a294edc396bfa2125cec615f700e82f84f31d4a27ef9b4c530`。

已覆盖安装到 ALN-AL00 真机并成功启动，版本、debug 标记及稳定 PID 已核对。未卸载或清除应用数据。真机主观跟手感待用户复核。[安装记录](evidence/2026-10-06-pan-latency/physical-installation.json)。
