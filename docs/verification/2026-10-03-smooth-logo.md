# NearSend 1.2.1 原首页动画恢复与手势补充修复

后续用户要求不得修改 GPUI 核心，本报告的核心补丁已撤回。当前使用上游 GPUI 原码并将手势修复迁到 gpui-ohos，见 [10 月 4 日平台层修复](2026-10-04-ohos-scroll.md)。本报告测试包属于历史方案。

日期：2026-10-03。模拟器测试包：`NearSend-1.2.1-scrollfix-smooth-default-release.hap`。

## 本次调整

按用户要求移除首页 Logo 的帧率限制，恢复 `Animation::new(duration).repeat()`。Logo 组件源码与修改前 HEAD 完全一致：15 秒转一圈，按 GPUI 正常帧调度更新。没有将首页改为静态，也没有人为设置 15 fps 或 30 fps 上限。

对照最新 `longbridge/gpui-mobile` 后，发现抓停惯性时新触摸继承旧方向的问题。本地回归测试复现：横向惯性后向上移动 3 px，修改前输出 0 px；修改后正确输出 3 px。反方向也覆盖。修复在共用 GPUI 识别器中完成，仍保留立即抓停、从第一个像素开始拖动、松手不误触点击的行为。[修复位置和源码对照](2026-10-03-gpui-mobile-comparison.md)。

恢复逐帧动画的早期候选在模拟器留下应用与 render_service 卡死记录；应用堆栈停在 GLES present 的原生缓冲申请。检查发现平台在处理可见性与 Surface 事件之前绘制待处理帧。现在先处理生命周期，隐藏或销毁的 Surface 不提交帧，恢复可见后请求新的 VSync。本轮最终候选通过下面的有限运行检查；这不等于捕获了每种模拟器或真机卡死的全部原因。

## 验证结果

环境：Pura 90 phone 与 MateBook Pro 2in1，HarmonyOS 7.0.0.106 / API 26，ARM64，Rust 与 ArkTS 均为 Release。

| 项目 | 结果 |
| --- | --- |
| GPUI 库测试 | 349 项通过，包含原有 348 项及新增抓停后选择新轴的回归测试 |
| 原首页 Logo 动画 | 两端首页截图确认旋转角度变化，源码没有帧率上限 |
| 持续旋转首页 | phone 305 秒、2in1 306 秒，进程保持不变，无新增应用或渲染服务故障日志 |
| 熄屏 5 秒、唤醒恢复 | 两端原进程保留；截图确认恢复首页 |
| 前后台切换 | 两端原进程保留；截图确认恢复首页 |
| 设置页慢滑 / 快滑 / 反向滑动 | phone 39 张、2in1 32 张连续截图采样检查，本轮未观察到异常跳回 |
| 滑动后停住 350 ms 再抬手、抓停惯性后抬手 | 两端后续 700 ms 的内容区域像素差均为 0 |
| 额外连续滑动 | phone 24 次、2in1 16 次，进程不变，无新增故障日志 |
| 格式与构建 | Rust 格式、diff 空白检查、ARM64 Release 和 ArkTS Release 构建通过 |

持续动画检查临时将模拟器息屏超时改为 10 分钟，检查结束后恢复原设置；随后显式熄屏、唤醒与前后台切换。源码中的 Surface/可见性保护在上述切换期间保持应用正常。

连续截图是采样检查，不是逐帧录屏或真机 120 Hz 帧时间测量。实际手感与真机温度由用户继续检查。

## 当前 CPU 数据

恢复原动画后，静止首页连续测量 3 × 10 秒，整个应用进程 CPU 按单核 100% 计：

| 模拟器 | 原动画恢复后的首页 CPU |
| --- | ---: |
| phone | 23.357% |
| 2in1 | 23.580% |

此前报告中限帧版的 4.553% / 5.185%，以及更早静态 Logo 的约 1.1%，均不适用于这个版本。本次优先按用户要求恢复原动画，不声称已经降低原动画持续运行时的 CPU 或证明真机发热消失。

## 签名产物

- `dist/NearSend-1.2.1-scrollfix-smooth-default-release.hap`：用户提供的 default 签名，用于真机覆盖安装。Profile 类型 debug，应用本身 Rust/ArkTS 为 Release、`debug=false`。
- `dist/NearSend-1.2.1-scrollfix-smooth-publishrelease.app`：publishrelease 签名，Profile 类型 release。
- 同目录保留 publishrelease HAP 与 `NearSend-1.2.1-scrollfix-smooth.sha256`。

三份产物都通过签名校验；APP 校验外层签名。版本为 1.2.1 / 1002001。三份产物的 22 个已分配且有文件内容的原生 ELF section 与模拟器测试 HAP 一致，ArkTS ABC 一致。构建后产品签名选择已恢复 default。

证据：[目录](evidence/2026-10-03-smooth-logo/)；[产物核验](evidence/2026-10-03-smooth-logo/package-verification.json)。

## 真机安装

前一份 `scrollfix-default-release.hap` 已在用户手机覆盖安装并启动成功。本次 `scrollfix-smooth` 新包等待真机 USB 调试重新上线，尚未安装。
