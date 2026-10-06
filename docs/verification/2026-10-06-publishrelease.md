# NearSend 1.2.1 正式签名 APP

用户确认真机整体效果正常后，使用已有 `publishrelease` 配置重新执行 Release `assembleApp`，版本为 **1.2.1 / 1002001**，包名 `com.richerfu.nearsend`，debug 标记为 false。

- [publishrelease APP](../../dist/NearSend-1.2.1-publishrelease.app)
- [publishrelease HAP](../../dist/NearSend-1.2.1-publishrelease.hap)
- [SHA-256](../../dist/NearSend-1.2.1-publishrelease.sha256)

两份产物均通过 SDK 签名校验，profile 类型为 release。22 个原生有效分配节及 ArkTS ABC 与已验证并安装到真机的 default Release 包一致。APP 内嵌 HAP 与单独 HAP 的代码和资源一致，pack.info 的 JSON 内容一致；单独 HAP 额外含 .pages.info。构建完成后原签名配置已恢复，真机当前测试包保持不变。

[完整产物校验](evidence/2026-10-06-publishrelease/package-verification.json)。
