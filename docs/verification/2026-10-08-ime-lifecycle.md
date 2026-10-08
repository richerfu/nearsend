# PC 输入法崩溃确认与回归

## 结论

原日志符合 `ohos-ime-binding 0.3.0` 提前释放原生编辑器回调表造成的 use-after-free。
使用未修复原生库，在 PC 模拟器上通过实际中文输入、弹窗切换和最小化复现了相同故障机制。
这是 NearSend 进程内的绑定库生命周期错误；栈中出现系统库不代表系统进程崩溃。

旧代码在 Show/Hide 返回 `12800016`（NOT_EDITABLE）或 `12800009`（DETACHED）时，
调用 `discard_local_session()` 并销毁 `TextEditor`。这些错误并不保证系统已停止使用回调表。
稍后的 `OnInputStop` 仍可能调用该表中的 `sendKeyboardStatusFunc`。

修复分支：`/Users/ranger/Desktop/project/ohos-rs/ohos-native-bindings` 的
`fix/ime-editor-lifetime`，提交 `024e99d`。NearSend 的 Cargo patch 已直接依赖该本地源码。
修复保留过期会话的编辑器，重连复用回调表；失败清理时撤销 Rust 回调并延迟回收原生表，
同时串行管理进程内输入代理。

## 原事故与模拟器复现

| 项目 | 原 PC 日志 | 模拟器旧版复现 |
| --- | --- | --- |
| 系统 | MateBook Pro 6.1.0.117 | PC emulator 7.0.0.106 |
| 应用 | 1.2.2 / 1002002 | 1.2.2 / 1002002 |
| 时间 | 10-07 14:31:55.731 | 10-08 14:49:25.945 |
| 线程 | OS_IPC_0_46819 | OS_IPC_5_7801 |
| 接口 | IInputClient / OnInputStop | IInputClient / OnInputStop |
| 回调槽 | 编辑器 `x0 + 0x20` | 编辑器 `x0 + 0x20` |
| 槽值 | `0x5980df6550`，libutils 只读映射 | `0x65726f666562206c`，字符串字节 `l before` |
| 故障 | 跳到不可执行内存，SEGV_ACCERR | 跳到未映射地址，SEGV_MAPERR |

模拟器 `libohinputmethod.so` 的实际指令为 `ldr x2, [x0, #32]`，随后 `br x2`；
Destroy 直接调用 C++ delete，尚未包含上游后续的弱引用防护。
两份日志的寄存器及内存转储均验证了同一回调槽损坏。
模拟器部分系统符号已裁剪，按调用指令、虚表槽及 `OnInputStop` 字符串交叉定位，
没有把反汇编工具显示的“最近导出符号”误当作真实函数名。

旧版复现的明确时序：

1. `14:49:25.887` 点击最小化。
2. `14:49:25.914` 输入法返回 `12800016`，命中旧绑定的提前释放分支。
3. `14:49:25.915` 应用进入后台。
4. `14:49:25.945` IPC 线程调用已被覆盖的回调地址，崩溃。

原附件截图显示“编辑收藏设备”中的名称/IP 输入、端口校验提示及随后桌面状态；
复现使用“添加收藏设备”，两者共用 `src/ui/pages/home/favorites.rs` 的输入弹窗实现。
端口校验提示是复现步骤中的焦点切换，不是端口解析代码导致的崩溃。

证据：[两份日志对照](evidence/2026-10-08-ime-lifecycle/crash-comparison.json)、
[原日志](evidence/2026-10-08-ime-lifecycle/original-crash.log)、
[复现日志](evidence/2026-10-08-ime-lifecycle/baseline-crash.log)、
[毫秒级时序](evidence/2026-10-08-ime-lifecycle/baseline-timeline.txt)、
[真实系统回调指令](evidence/2026-10-08-ime-lifecycle/native-callback-disassembly.txt)。

## 回归方法与结果

同一台模拟器、同一签名和版本的 Release HAP 覆盖安装对比。两包 ArkTS 字节码完全一致，
原生 ELF allocated sections 分别与原始库、修复库完全一致。
修复包包含用户已有的 IME 文本/配置同步改动及本次生命周期修复。
构建与包 hash 见[产物校验](evidence/2026-10-08-ime-lifecycle/packages.json)。

前置条件：3120×2080 显示，应用窗口 `[515, 281, 2090, 1394]`；
打开收藏设备弹窗，IP 填入非空测试值，清空端口，预先启用模拟器内置中文输入法。
每轮发送拼音 `ceshi` 和空格按键，依次切换名称/IP/端口，触发并关闭端口校验提示，
重新聚焦 IP，最小化后恢复窗口。没有发送文件或连接测试地址。

| 检查 | 结果 |
| --- | --- |
| 未修复原生库 | 第 3 轮崩溃，PID 6030 退出，生成上述 cppcrash |
| 修复版同脚本 | 100/100 轮，547 秒，PID 始终 12556，无新增 fault |
| 出错分支覆盖 | 日志记录 100 次 `12800016`、100 次后台切换；含补充检查共 101 次成功 Attach |
| 历史宿主测试（清理前） | 13/13 通过：9 项生命周期、4 项文本/UTF-16 |
| ARM64 Release 原生库 | 构建、链接通过 |
| ArkTS Release HAP | 隔离工程 assembleHap 通过，已安装模拟器 |

历史宿主测试结果记录于删除测试文件之前。现已按要求删除新增的宿主生命周期集成测试、
C NDK double 及执行脚本。上述模拟器崩溃复现和 UI 回归使用真实系统库，未注入错误码。

测试命令与记录：

```sh
# 满足上述界面前置条件后，在本机运行留存脚本
python3 docs/verification/evidence/2026-10-08-ime-lifecycle/cycle.py fixed 100
```

[旧版循环结果](evidence/2026-10-08-ime-lifecycle/baseline-cycles.json)、
[修复版循环结果](evidence/2026-10-08-ime-lifecycle/fixed-cycles.json)、
[错误分支覆盖](evidence/2026-10-08-ime-lifecycle/fixed-branch-coverage.json)、
[宿主测试日志](evidence/2026-10-08-ime-lifecycle/host-tests.log)、
[修复版中文输入](evidence/2026-10-08-ime-lifecycle/fixed-1.jpeg)、
[修复版端口提示](evidence/2026-10-08-ime-lifecycle/fixed-notice.jpeg)。

### 功能回归发现的已有问题

首次打开输入框、中文候选选择和上屏正常。压力回归后关闭并重开输入弹窗，
仍能正常显示拼音候选并上屏“测试”，PID 保持 12556。
见[回归后的候选](evidence/2026-10-08-ime-lifecycle/fixed-reopen-pinyin.jpeg)、
[回归后的上屏](evidence/2026-10-08-ime-lifecycle/fixed-reopen-commit.jpeg)。

但“IP 框聚焦 → 最小化 → 恢复 → 直接点击名称框”会遇到输入法未重新激活的问题。
切回未修复 HAP 执行完全相同操作，也复现了该现象；恢复后点击原先聚焦的同一个输入框则能出现候选。
见[旧版正常恢复同一输入框](evidence/2026-10-08-ime-lifecycle/old-focus-after.jpeg)、
[旧版恢复后切换输入框](evidence/2026-10-08-ime-lifecycle/old-switch-after.jpeg)。
这是已有的焦点/输入法恢复问题，本次生命周期修复未覆盖它。
因此，100 轮结果表示生命周期崩溃回归通过，不代表每轮均成功上屏或全部输入体验问题已解决。

本次真实系统验证环境为 PC 模拟器 7.0.0.106，未在原 MateBook 的 6.1.0.117 上执行回归。
同机制旧版复现、寄存器/回调槽对齐和修复版对照构成本次根因判断依据。

已将验证通过的原生库更新到 `platform/ohos/entry/libs/arm64-v8a/libnear_send.so`，
并恢复模拟器上的修复版安装。[测试 HAP](../../dist/NearSend-1.2.2-ime-fixed.hap) 使用 default 测试签名，
不是 publishrelease 发布包。
验证产物及完整 hilog 留存在 `/tmp/nearsend-ime-validation/`；固定布局循环脚本也保存在证据目录。
证据目录按仓库现有规则被 Git 忽略，本文档保留在工作树。
