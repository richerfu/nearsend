# 平板文件夹选择排查与兼容修复（2026-10-07）

## 结论与边界

用户环境：HarmonyOS 6.1.0、MatePad Edge、API 6.1.1 (24)、中文、Wi-Fi；操作“发送 → 文件夹”提示“选择文件失败”。真机暂不方便连接，用户要求继续模拟器排查。

**已修复通用 FilesPlugin 错误拒绝支持文件夹选择的平板，并统一 NearSend 的文件夹入口；MatePad Edge 原始故障尚未在模拟器复现，不能据此宣称该机型故障已确认解决。**

- API 24 tablet 模拟器中，原普通 Release 能打开选择器，选择包含嵌套文件的目录后正常加入 1 个文件（41 B）。
- 诊断包记录 `deviceType=tablet`、`sdkApiVersion=24`、旧 `FolderSelection` SysCap 为 `true`。
- 在该设备上通过诊断入口调用未修改的通用 FilesPlugin，会立即抛出 `OpenHarmony folder selection requires a 2-in-1 device with FolderSelection capability`。这是已实测的插件缺陷，但原 NearSend 使用独立的 `pick-directory`，两者不能混为同一根因。
- 已反汇编比较旧 `NearSend-1.2.1-publishrelease.hap` 与当前普通 Release：NearSend 自定义 `supportsDirectoryPicker`、`pickDirectory` 的函数体相同，未发现这部分的旧包差异。

## 实现

- `openharmony-ability/plugins/files/.../FilesPlugin.ets`：删除 folder 的设备名称/旧 SysCap 前置拦截，由系统 `DocumentViewPicker.select(FOLDER)` 判定实际支持情况；保留单目录、不使用文件后缀过滤的参数。
- `getSelectedIndex` 仍使用原来的 API / 设备 / SysCap 条件，未扩大该独立接口的调用范围。
- 插件在调用前、返回后检查 Ability 活跃状态；系统错误的 message 中保留操作类型、设备类型、API 版本和错误码，便于后续真机定位。
- NearSend 的 `pick_folders` / `pick_save_directory` 统一调用 `FilesExt::show_file_dialog(OPEN_FOLDER)`，删除独立 `pick-directory` 的 Rust / ArkTS 路径。
- 保留 NearSend 的 tablet / 2in1 入口规则、手机普通文件多选、5 分钟交互超时、URI 持久授权与激活；没有删除权限校验或降级为只选单个文件。
- 通用插件源码同步到本地 `openharmony-ability-pr82` 的 `feat/pr82-nearsend-integration` 工作树和 NearSend vendor。Rust 插件契约没有改变，Cargo.lock 仍锁 `2a5d247c`；本次未提交、未推送。
- GPUI 与 gpui-ohos 渲染、线程、手势代码未修改。

当前 [OpenHarmony Picker 文档](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/reference/apis-core-file-kit/js-apis-file-picker.md) 对 `selectMode` 区分 API 11–24 的 `FolderSelection` 与 API 26 起的 `UserFileService`。本次采用真实系统调用判定能力，避免把设备名称或单一旧能力名称当成通用支持条件；该文档变化不能单独证明 API 24 MatePad Edge 的故障原因。

## 验证

| 项目 | 环境 / 结果 |
| --- | --- |
| 通用插件修复前 | API 24 tablet，SysCap 为 true，仍被 2in1 判断拒绝；诊断入口实测 |
| 文件夹完整返回 | API 24 tablet，HarmonyOS 6.1.0.125；普通 Release 选中 Download，递归加入测试目录 `NearSend-folder-qa-20261007/nested/pad-folder.txt`，1 个文件 / 41 B |
| 取消文件夹选择 | API 24 tablet，返回原发送页，保留已选文件，无失败提示 |
| 手机普通文件选择 | API 26 phone；打开系统文件选择器，取消后返回发送页，无失败提示；未做实际文件多选的设备测试 |
| 2in1 文件夹选择 | API 26 MateBook Pro；打开系统路径选择器，选中空 Download 后正常返回，显示原有“未添加到可发送文件”提示，无 picker / 持久授权失败 |
| 插件边界检查 | 执行生产 ETS 经 TypeScript 转译的代码，模拟系统 API：6/6，通过有能力平板（旧 SysCap=false）、手机默认文件模式及多选参数、取消、系统 801 错误、Ability 销毁、拒绝多目录请求 |
| arm64 原生 Release | `ohrs build --arch arm64 --release ... -- --locked`，通过 |
| ArkTS Release | Hvigor `assembleHap`，通过 |
| 静态检查 | 变更 FilesPlugin 的 `oxk lint`、`oxk format`，`file_picker.rs` rustfmt 检查及 git diff 检查通过 |
| Rust 插件宿主单测 | **未通过执行门槛**：macOS 链接缺少 OHOS `native_window` 系统库；不是通过结果，未为此恢复非 OHOS 兼容代码 |

嵌套测试文件通过模拟器系统 Picker 正常授权后由隔离诊断包创建；交付包已移除所有诊断入口和测试文件写入逻辑。未验证跨设备实际传输、网络目录、云端目录或该型号真机。

截图与检查记录：

- [通用插件修复前的失败](evidence/2026-10-07-tablet-folder-picker/plugin-before.jpeg)
- [平板修复包加入嵌套文件](evidence/2026-10-07-tablet-folder-picker/tablet-selected.jpeg)
- [平板取消选择后](evidence/2026-10-07-tablet-folder-picker/tablet-cancel.jpeg)
- [手机系统文件选择器](evidence/2026-10-07-tablet-folder-picker/phone-picker.jpeg)
- [2in1 系统文件夹选择器](evidence/2026-10-07-tablet-folder-picker/2in1-picker.jpeg)
- [6 项插件边界检查](evidence/2026-10-07-tablet-folder-picker/plugin-check.log)

原始本地日志与临时检查脚本：`/tmp/nearsend-pad-folder-2026-10-07/`。

## 产物

`dist/NearSend-1.2.1-pad-folder-default-release.hap`，版本 `1.2.1 / 1002001`，`debug=false`，用户提供的 default 签名。

SHA-256：`78107934b4df33866af8e3edcdbe78452d043c589b0f39b19d98377d15b8d02e`。

已安装并启动于 API 24 tablet、API 26 phone、API 26 2in1 模拟器；没有安装到当前连接的 ALN-AL00 真机，没有生成新的 publishrelease APP。

已核对包内无 `PAD_FOLDER_PROBE` / `pick-directory`，并比较打包前后的 native ELF：22 个有文件内容的 allocated section 完全相同（打包过程会裁剪符号，整文件 hash 因此不同）。原项目 `entry/libs` 和 `dist/arm64-v8a` 已同步本次原生库，避免下次 ArkTS 打包误用旧库。

## 磁盘处理

清理 NearSend 非交付架构/调试缓存、旧 `nearsend-pr82/target`、无运行中构建的 `rust-ffmpeg-sys/target`，合计释放约 20 GiB。保留源码、签名、已有安装包和模拟器数据。下载 API 24 镜像后已成功启动；最终已删除隔离打包工程及其中的签名配置副本，可用空间约 16 GiB（随 swap 和构建缓存变化）。
