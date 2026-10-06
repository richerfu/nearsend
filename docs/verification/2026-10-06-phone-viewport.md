# Phone 底部额外留白修复

## 原因与改动

系统 Window 和 XComponent 都覆盖完整屏幕。gpui-ohos 收到的 Keyboard avoid-area 可以同时满足 `visible=true`、所有矩形尺寸为 0。之前只凭 visible 或键盘请求标志进入键盘占用计算，随后把 NavigationIndicator 的高度计入占用，导致无实际键盘时也缩小 GPUI 视口。

页面已经正常预留底部避让，因此这次额外缩小形成了第二块白色区域。修复在 [gpui-ohos/window.rs](../../vendor/gpui-ohos/src/ohos/window.rs)：只有键盘区域存在实际非零高度，才计算键盘占用及相关区域的并集。

GPUI 核心保持上游原样。页面、导航栏和 `safe_area.rs` 的避让值未修改。测量用的日志及 Canvas 已移除，AppRoot 和 navigation 源码与此次工作前的内容逐字节一致。

## 明确验证

| Phone 模拟器，1320×2856 / 3.5 倍密度 | 修复前 | 修复后 |
| --- | --- | --- |
| GPUI 视口高度 | 启动时 816，随后错误缩为 788 个逻辑像素 | 稳定为 816 个逻辑像素 |
| 页面底部正常避让 | 28 个逻辑像素 | 28 个逻辑像素 |
| 导航栏 y / 高度 | 702 / 58 个逻辑像素 | 730 / 58 个逻辑像素 |

额外扣除的 **98 个物理像素**已移除，正常避让保持原值。见 [几何测量](evidence/2026-10-06-phone-viewport/geometry-check.json)、同目录的 before/after geometry 日志。

最终生产包已安装到 phone 和 2in1 模拟器；phone 接收、发送、设置三个页签及 2in1 正常布局截图已检查，证据为 `phone-final-*.png`、`2in1-final-home.png`。

尝试唤起模拟器输入法时，遇到输入法首次使用协议。未接受协议，因此没有将“实际键盘显示时的避让”计为已验证。本次新增判断仅排除零高度键盘区域，实际非零键盘区域的原有计算路径保留。

Rustfmt、diff check 和 ARM64 优化 Release 构建通过。三份签名包均通过校验，正式产物的 22 个原生有效分配节及 ArkTS ABC 与模拟器测试 HAP 一致。[包校验](evidence/2026-10-06-phone-viewport/package-verification.json)。

## 产物与真机

- [default Release HAP](../../dist/NearSend-1.2.1-ohos-viewport-default-release.hap)
- [publishrelease HAP](../../dist/NearSend-1.2.1-ohos-viewport-publishrelease.hap)
- [publishrelease APP](../../dist/NearSend-1.2.1-ohos-viewport-publishrelease.app)

版本为 1.2.1 / 1002001。default 包 SHA-256：`1fb40f8c1e34ae985af2c07b7797ae48d3a5257174cf6dea4aebb5e505b0c136`。

已用 default 签名覆盖安装并启动于 ALN-AL00 真机，设备版本与运行进程已核对，截图中底部位置正常。[安装记录](evidence/2026-10-06-phone-viewport/physical-installation.json)。
