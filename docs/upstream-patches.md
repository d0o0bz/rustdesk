# 上游补丁登记

本文件记录所有对上游 RustDesk 文件的修改位置与目的，便于升级时复核。
每条记录包含：文件、改动类型、目的、与上游兼容性。

## macOS 双显卡低功耗优化（A+B 方案）

### A. 应用层 GPU 切换声明

- **文件**：`flutter/macos/Runner/Info.plist`
- **改动**：新增 `NSSupportsAutomaticGraphicsSwitching = true`
- **目的**：声明 RustDesk 不强制要求独显，macOS 在空闲时可关闭独显。
- **兼容性**：纯 plist 键新增，不影响上游其他平台构建。升级时若上游 Info.plist 未引入此键，需手动保留。

### B1. 截图管线低功耗配置

- **文件**：`libs/scrap/src/quartz/config.rs`
- **改动**：新增 `Config::low_power()` 关联函数（throttle=1/15s, queue_length=2）。
- **目的**：低功耗模式下降低 `CGDisplayStream` 合成频率，减少独显唤醒。
- **兼容性**：纯 additive 关联函数，不改 `Default` 实现，不影响现有调用方。

- **文件**：`libs/scrap/src/common/quartz.rs`
- **改动**：新增 `Capturer::new_with_config(display, config)` 构造器；原 `new` 委托到 `new_with_config(default)`。
- **目的**：允许调用方注入低功耗 `Config`。
- **兼容性**：新增 API + 原 `new` 行为不变。

### B2. macOS 平台低功耗 helper

- **文件**：`src/platform/macos.rs`
- **改动**：文件末尾新增 `pub fn apply_low_power_mode(enabled: bool)`，切换 `scrap::quartz::ENABLE_RETINA`。
- **目的**：低功耗时禁用 Retina 全分辨率捕获，减少 GPU 负载与独显唤醒。
- **兼容性**：纯 additive 函数，不改任何现有函数。

### B3. video_service 接入低功耗开关

- **文件**：`src/server/video_service.rs`
- **改动**：`create_capturer` 的 `#[cfg(not(windows))]` 分支内，新增 `#[cfg(target_os = "macos")]` 子分支读取 `low-power-mode` option，开启时调用 `apply_low_power_mode(true)` 并使用 `Config::low_power()`；否则恢复默认 Retina。
- **目的**：让本机端 `low-power-mode` 设置实际生效于捕获管线。
- **兼容性**：改动集中在 macOS 条件编译块内，其他平台路径不变。

### 设置：low_power_mode 开关

- **文件**：`libs/hbb_common/src/config.rs`
- **改动**：新增 `serde_field_bool!(LowPowerMode, "low-power-mode", ...)`，紧随 `ViewOnly` 之后。
- **目的**：提供本机端持久化的低功耗开关。
- **兼容性**：纯 additive 结构体定义，不改现有任何字段或函数。

- **文件**：`flutter/lib/consts.dart`
- **改动**：新增 `const String kOptionLowPowerMode = "low-power-mode";`。
- **兼容性**：纯 additive 常量。

### B4. 设置页低功耗开关（UI）

- **文件**：`flutter/lib/desktop/pages/desktop_setting_page.dart`
- **改动**：新增 `lowPowerMode()` 卡片（`isMacOS` 门控，仅 macOS 显示），内含绑定 `kOptionLowPowerMode` 的 `_OptionCheckBox`，并挂载到 General 标签页 `hwcodec()` 之后。
- **目的**：把已在 Rust 侧实现的低功耗能力暴露为用户可自行开关的设置项。
- **兼容性**：纯 additive 方法与一行挂载；其他平台不渲染。

### B5. 低功耗时禁用硬件编解码

- **文件**：`libs/scrap/src/common/codec.rs`
- **改动**：`enable_hwcodec_option()` 内新增 `#[cfg(target_os = "macos")]` 分支，`Config::get_option("low-power-mode") == "Y"` 时返回 false。
- **目的**：低功耗模式下禁用 VideoToolbox，避免硬件编解码唤醒独显。该函数同时门控编码器（服务端被远程）与解码器（`hwcodec.rs` 的 `HwRamDecoder::try_get`，客户端远程他人），故一处改动双向生效。
- **兼容性**：仅新增 macOS 条件编译分支，其他平台逻辑完全不变；`enable-hwcodec` 原有开关语义保留。

### 多语言 key

- **文件**：`src/lang/template.rs` 及 `src/lang/*.rs`
- **改动**：末尾各追加 `Low power mode`、`Enable low-power mode on dual-GPU Macs`、`low_power_mode_tip` 三个 key；`en.rs` 只补 `low_power_mode_tip` 的英文文案，`cn.rs` / `tw.rs` 提供中英对照翻译，其余语言留空（回退英文）。
- **兼容性**：纯 additive 条目，不动任何既有 key。

## install_config_import 模块（TOML 配置导入）

### 新增独立模块（无上游修改）

- **文件**：`src/config_import.rs` 及 `src/config_import/{error,toml_config,toml_parser,path_safety,field_mapper,importer}.rs`
- **改动**：新增配置导入组件，含 TOML 解析、字段映射、路径安全校验、合并存储；导入仅经命令行 `--import-toml-config` 触发，安装目录不再被扫描。
- **兼容性**：纯新增模块，由 `#[cfg(feature = "toml-config-import")]` 门控，默认不启用，对上游零侵入。

### 上游文件修改

- **文件**：`src/lib.rs`
- **改动**：末尾新增 `#[cfg(feature = "toml-config-import")] mod config_import;`，带 `// dec: TOML配置导入` 注释。
- **兼容性**：feature 门控，未启用时不编译。

- **文件**：`Cargo.toml`
- **改动**：`[features]` 段新增 `toml-config-import = []`。
- **兼容性**：空 feature，默认不启用，不影响现有构建。

- **文件**：`src/core_main.rs`
- **改动**：两处 `#[cfg(feature = "toml-config-import")]` 门控的新增：① `core_main()` 参数分派链中 `--import-toml-config` 分支（`cfg!(feature)` 短路 + 分支内 `#[cfg]` 块）；② 文件末尾 `run_toml_import_from_args()` 函数（解析 `--json` 与路径、映射退出码、`std::process::exit`）。`// dec: TOML配置导入` 标记只在分支处有一处。
- **兼容性**：feature 门控，未启用时参数分支条件 `cfg!(feature)` 为 false 不匹配，行为不变。

### Windows 安装器改动

- **文件**：`res/msi/CustomActions/ImportTomlConfig.cpp`（新增）、`res/msi/CustomActions/CustomActions.def`、`res/msi/CustomActions/CustomActions.vcxproj`
- **改动**：新增 deferred custom action `ImportTomlConfig`（导出名加入 `.def`、源文件加入 `.vcxproj`）。以 SYSTEM 运行，但用 `WTSEnumerateSessions` + `WTSQueryUserToken` + `CreateProcessAsUserW` 在每个 `WTSActive` 会话里以用户身份执行 `RustDesk.exe --import-toml-config <toml>`，单会话等待上限 60 s。
- **目的**：让安装期导入落到真实登录用户的 `%APPDATA%` 配置，而不是 SYSTEM 的配置。
- **兼容性**：`Return="ignore"`，toml 缺失 / 无活动会话 / 导入失败一律只记日志不中断安装。

- **文件**：`res/msi/Package/Fragments/CustomActions.wxs`、`res/msi/Package/Components/RustDesk.wxs`
- **改动**：声明 `ImportTomlConfig` 与 `ImportTomlConfig.SetParam`（参数格式 `<msi 路径>|<exe 路径>|<toml 文件名>`，`$(var.ProductLower)-config-import.toml`），并排入 `InstallFiles` 之后，条件 `NOT (Installed AND REMOVE AND NOT UPGRADINGPRODUCTCODE)`。
- **目的**：全新安装与升级时导入，卸载时不导入。
- **兼容性**：只在既有 `InstallExecuteSequence` 中追加两条 `<Custom>`，不动上游其它自定义动作。

- **文件**：`res/msi/Package/UI/MyInstallDirDlg.wxs`、`res/msi/Package/Language/Package.en-us.wxl`、`res/msi/Package/Fragments/ShortcutProperties.wxs`、`res/msi/Package/Package.wxs`、`res/msi/Package/Components/RustDesk.wxs`
- **改动**：新增 `ADDTOPATH` 属性与「Add to PATH (command line usage)」勾选项（默认 `1`），配套 `App.Path` 组件（`Environment Name="PATH" Part="last" System="yes"`）与两个把选择持久化到 `HKCR\$(var.RegKeyRoot)\ADDTOPATH` 的组件（`...AddToPathProperties1/0`），后者在 `Package.wxs` 中 `ComponentRef`。
- **目的**：把安装目录追加进系统 PATH，便于在任意目录跑 `--import-toml-config`。
- **兼容性**：纯 additive 对话框控件 + 新组件；`Part="last"` 不覆盖既有 PATH，卸载时由 Windows Installer 自动移除。

- **文件**：`build.py`
- **改动**：新增 `--toml-config-import` 命令行参数（映射到 feature 列表）；生成的 `install.sh` 不再复制 toml，改为打印「以登录用户身份运行 `rustdesk --import-toml-config <path>`」提示。
- **兼容性**：参数默认关闭；不带该参数时 feature 列表与上游一致。

## 多服务器配置（multi-server config）

`libs/hbb_common` 是独立 git 子仓库，这一组改动大部分落在其中，需在 `libs/hbb_common` 里单独提交。

### 新增文件（无上游修改）

- **文件**：`flutter/lib/models/server_config_model.dart`、`flutter/lib/common/widgets/server_config_dialog.dart`、`flutter/lib/common/widgets/server_config_widgets.dart`
- **改动**：多服务器配置的 model 与桌面/移动端共用的列表、编辑对话框组件。
- **兼容性**：纯新增 Dart 文件，未接入前不参与渲染。

### 上游文件修改

- **文件**：`libs/hbb_common/src/config.rs`
- **改动**：带 `// dec: 多配置支持` 标记的成组新增——`ServerConfig` 结构体与 `Default`（`id` 为 UUID、`id_port` = `RENDEZVOUS_PORT`、`relay_port` = `Some(RELAY_PORT)`）；`Config2` 增加 `rendezvous_servers` / `current_config_id` 两个带 `serde(default)` 的字段；`OPTION_AUTO_SWITCH_ENABLED` / `OPTION_MULTI_SERVER_STORE` / `MAX_SERVER_CONFIGS`（上限 5）常量；`ConfigState`、`MultiServerStore`（独立存储，后缀 `multi_config`，与 `Config2` 分开落盘）、`ConfigError`、`ServerConfigRepository`（`SERVER_OPTION_KEYS` 四键同步 + `SUPPRESS_DEFAULT_PROMOTION` 抑制默认项提升）、`ConfigManager`、`AvailabilityChecker`、`LatencyMonitor`、手动/自动切换器与单元测试。
- **目的**：支持保存多个中继服务器配置并在其间手动/自动切换。
- **兼容性**：`Config2` 只加带 `serde(default)` 的字段，旧配置文件仍可反序列化；其余均为 additive 结构体 / 函数 / 常量。升级时需重点确认 `Config2` 未被上游新增同名字段。

- **文件**：`src/ui_interface.rs`
- **改动**：新增 `// dec: 多配置支持` 的 glue 层，薄封装 `ConfigManager` 等能力后暴露给上层。
- **兼容性**：additive 函数。

- **文件**：`src/ui.rs`
- **改动**：在 `get_connect_status` 之后新增 `// dec: 多配置支持 - 配置管理IPC接口` 一组方法（`get_all_configs` / `get_current_config` 等）。
- **兼容性**：只在既有 sciter IPC trait 实现里追加方法。

- **文件**：`src/flutter_ffi.rs`
- **改动**：新增 `// dec: 多配置支持 - 暴露给 Flutter 的多服务器配置接口`。
- **兼容性**：additive FFI 导出。

- **文件**：`flutter/lib/desktop/pages/desktop_setting_page.dart`、`flutter/lib/mobile/pages/settings_page.dart`
- **改动**：各追加一个服务器配置入口卡片（桌面端 8 行、移动端 8 行）。
- **兼容性**：纯 additive 挂载。

- **文件**：`src/lang/template.rs` 及 `src/lang/*.rs`
- **改动**：`template.rs` 末尾追加 19 条 key（`Edit`、`Use`、`In use`、`Available`、`Unavailable`、`Latency`、`Checking...`、`Required`、`Multiple server config`、`Auto switch server`、`Add server config`、`Edit server config`、`Delete server config`、`Delete server config tip`、`No server config`、`Not checked`、`Check availability`、`ID Server Port`、`Relay Server Port`）；`en.rs` 只补有英文文案差异的项，`cn.rs` / `tw.rs` 提供中文翻译，其余语言留空（回退英文）。
- **兼容性**：纯 additive 条目，不动既有 key。注意与低功耗 key 追加在同一个列表末尾，升级时按整段末尾一起核对。

## TOML 配置导入 / 多服务器配置修复

### libs/hbb_common/src/config.rs（第二批）

- **改动**：三个纯新增——`Config::get_stored_option(k)` 与 `Config::get_stored_options()`（只读 `CONFIG2.options`，不含 `DEFAULT_SETTINGS` / `OVERWRITE_SETTINGS`），以及 `MultiServerStore::try_save() -> io::Result<()>`；`MultiServerStore::save()` 改为转调 `try_save()` 并在失败时记 error。
- **目的**：`get_option` / `get_options` 返回的是**生效值**，无法区分"从未设置"与"构建预置"，导入据此判断单服务器配置是否被显式设置时会恒得非空答案；`save()` 原本吞掉落盘失败，并在失败后照样 publish 磁盘上的旧内容。
- **兼容性**：全部 additive。`save()` 签名与行为（除"写失败则不再 publish"）不变，子仓库内既有调用点无需改动；两个新访问器当前只被 `#[cfg(feature = "toml-config-import")]` 代码使用。

### src/core_main.rs（第二批）

- **改动**：`--import-toml-config` 分支内新增 `is_cli_setting_change_disabled()` 检查；`run_toml_import_from_args` 在导入成功后调用新增的 `publish_imported_store()`（`crate::ipc::set_options` 推送已存储的 option map），退出码新增 `5`（`InvalidServerConfig`），并为 1 / 2 两个分支补上错误详情。
- **目的**：该分支此前是唯一没有"设置已禁用"护栏的写配置入口；`publish()` 的返回值此前被直接丢弃，而 Windows 上没有 `sync_and_watch_config_dir`，已运行进程看不到导入结果。
- **兼容性**：全部在 `#[cfg(feature = "toml-config-import")]` 内，未启用该 feature 时不编译。

### src/config_import/**（含行为变更）

- **改动**：跳过判定由「toml mtime 对比 `Config::file()` mtime」改为记录在 option `toml-import-source` 的 `路径|大小|mtime`；`[[rendezvous_servers]]` 合并由整条替换改为逐字段叠加；写入前新增条目校验（格式 / 文件内去重 / 数量上限）；布尔字段由 `bool` 改为 `Option<bool>`；`MappedConfig.options` 由 `HashMap` 改为 `BTreeMap`；`[options]` 改为先写入再被明确字段覆盖；新增 `version` 主版本校验；路径安全改为按路径段判断（`..` 子串不再误伤合法名字）；解析改为单次 `File::open`；含密钥的 struct 改为手工 `Debug` 打码。
- **目的**：修正"配了但静默不生效"与"写入后配置被写坏"两类缺陷，逐条说明见 `docs/toml-config-import.md`。
- **兼容性**：**含行为变更**——布尔 `false` 现在写入 `"N"`（此前等同未指定）；缺少 `name` / `id_server` 或带非法端口的服务器条目会被拒绝（退出码 5）而不是入库；同一份文件里重复的 `id` / `id_server` 会被拒绝而不是静默塌缩。现有部署若依赖旧行为，升级前需先检查 toml。

## 多服务器配置（第三批：设为默认 / 国际化 / Web）

### libs/hbb_common/src/config.rs（子仓库）

- **改动**：`ConfigManager::set_default_config` 由「只翻转 `is_default`」改为「校验 id 存在（`ConfigError::ConfigNotFound`）+ 复用 `ServerConfigRepository::promote_default` 置顶 + `save()`」；`ConfigError` / `SwitchError` 的 `#[error(...)]` 与 `ServerConfig::validate`、`ConfigValidator::validate_name` / `validate_server_address`、`ManualSwitcher::switch` 的中文文案改为英文句子；`InvalidFormat` 与 `ConfigUnavailable` 的 `Display` 由「前缀: {0}」改为只输出 `{0}`；末尾单测新增 `test_promote_default_pins_to_top` 与 `test_set_default_config_rejects_unknown_id`，并同步更新 `test_config_error_display` 的断言。
- **目的**：设置默认此前不置顶，与 `sync_from_active_options` 的 `promote_default` 行为不一致，会让默认项落在列表中段——而弹窗的拖拽守卫与卡片的上下移按钮都假设默认项固定在 index 0。错误文案原先是中文硬编码，英文用户会直接看到中文 toast；`Display` 改为输出整句英文后，文案本身即可作为 `translate` 的 key。
- **兼容性**：`set_default_config` 签名与返回类型不变，唯一调用方是 `src/ui_interface.rs`；`promote_default` 为模块私有，行为与同步路径一致。`InvalidFormat` / `ConfigUnavailable` 的 `Display` 去掉前缀属**行为变更**，仅影响展示文案，不影响控制流。

### src/ui_interface.rs

- **改动**：`use` 列表加入 `ConfigError`；新增私有 `translate_server_config_error(String)`（内部 `crate::client::translate`）；`add/update/delete/switch/set_default/move` 六处 `e.to_string()` 改为经该 helper 输出；`update_server_config` 与 `switch_server_config` 的 `"配置不存在"` 字面量改为 `ConfigError::ConfigNotFound.to_string()` 经 helper 输出。
- **目的**：错误文案经 `translate` 后按语言回退，英文用户看到英文、中文用户看到中文。
- **兼容性**：只对返回给 UI 的字符串做包一层，不改任何控制流与返回值约定（成功仍为 `"ok"`）。

### src/ui.rs（sciter 遗留接口）

- **改动**：`switch_to_config` 的 `"配置不存在"` 改为 `ConfigError::ConfigNotFound.to_string()`，`use` 列表加入 `ConfigError`。
- **兼容性**：仅替换字面量，其余分支不变。

### flutter/lib/common/widgets/server_config_widgets.dart

- **改动**：`ServerConfigCard` 新增必填 `onSetDefault` 回调；操作区在「检测」按钮前插入 `Icons.star_border_outlined` 图标按钮（tooltip `Set as default`），仅当 `!config.isDefault` 显示。
- **目的**：此前 `setDefault` 链路（Rust / FFI / Dart model）已就绪但没有任何 UI 入口，卡片只能显示 `Default` 标签、无法设置。
- **兼容性**：仅新增回调与一个条件渲染的按钮。

### flutter/lib/common/widgets/server_config_dialog.dart

- **改动**：构造 `ServerConfigCard` 时接入 `onSetDefault`：调用 `state.setDefault(item.id)`，成功 `refresh()` + `showToast(translate('Successful'))`，失败 `showToast(err)`。
- **兼容性**：与既有 `onSwitch` / `_move` 同构的追加分支。

### flutter/lib/web/bridge.dart

- **改动**：`class RustdeskImpl` 内新增 12 个多服务器接口与 5 个私有 helper；列表以 JSON 存于 `option:flutter:local` 的 `multi-server-configs`，切换 / 编辑在用项时写回 `custom-rendezvous-server` / `relay-server` / `api-server` / `key` 四个 option；自动开关存于 `auto-switch-enabled`；`mainCheckServerConfig` 返回 `'null'`、`mainGetServerConfigDir` 返回空串。
- **目的**：Web 端此前完全没有这些方法的实现，`bind.mainGetAllServerConfigs()` 等调用在 Web 构建里无法解析。
- **兼容性**：纯新增方法，不动既有实现。Web 上无 TCP 探测能力，可用性一律保持「未检测」。

### src/lang/template.rs 及 src/lang/*.rs

- **改动**：末尾追加 18 个新 key（`Set as default` 之外的 17 条为错误文案英文句子：`Config name is required`、`Config name cannot exceed 50 characters`、`ID server address is required`、`Server address is required`、`Invalid ID server port`、`Invalid relay server port`、`Duplicate config name`、`This ID server address already exists`、`Maximum of 5 configs reached`、`The last config cannot be deleted`、`The default config cannot be deleted`、`The default config is pinned to the top`、`Invalid priority position`、`Config not found`、`The ID server is unreachable`、`Disconnect the remote session before switching config`、`Switching is protected, try again later`）；`cn.rs` / `tw.rs` 填中文译文，其余语言留空（回退英文）。另补登此前遗漏的 `Move up` / `Move down`（界面在用但从未登记，`cn` / `tw` 填 `上移` / `下移`），并把仓库中已存在但 `tw` 为空的 `Set as default` 补上 `設為預設`。
- **兼容性**：纯 additive 条目，不动任何既有非空翻译。

## vcpkg overlay port 补丁（aom / libyuv）

这两个 overlay port 位于 `res/vcpkg/aom/`、`res/vcpkg/libyuv/`，通过 `vcpkg_from_git` 取源码。
取源码 URL 由 portfile 中的环境变量切换：

- `res/vcpkg/aom/portfile.cmake`：若设 `AOM_SRC_URL` 则用之，否则回落 `https://aomedia.googlesource.com/aom`。
- `res/vcpkg/libyuv/portfile.cmake`：若设 `LIBYUV_SRC_URL` 则用之，否则回落 `https://chromium.googlesource.com/libyuv/libyuv`。

本地 Docker 构建（`docker/build-flutter.sh`）会 `export AOM_SRC_URL` / `LIBYUV_SRC_URL` 指向容器内
`file:///workspace/docker/aom-src/aom_aomedia.googlesource.com` 与
`file:///workspace/docker/libyuv-src/libyuv`（见 `docker/README.md` "网络代理说明"），使 vcpkg 跳过网络
直接读本地 git 仓库；GitHub CI 不设置这两个变量，portfile 走上游 googlesource 真实 URL。升级 aom /
libyuv 上游 tag 时，下列补丁需随上游改动重新核对或 rebase。

### aom 补丁

- **文件**：`res/vcpkg/aom/aom-uninitialized-pointer.diff`
- **改动**：`build/cmake/aom_configure.cmake` 对 MSVC 新增 `/wd4703`（抑制"局部指针变量可能未初始化"告警）。
- **目的**：避免将该告警升级为 error 阻断 MSVC 构建。
- **兼容性**：纯 additive 编译选项，MSVC-only；非 MSVC 构建无影响。

- **文件**：`res/vcpkg/aom/aom-install.diff`
- **改动**：`CMakeLists.txt` 引入 `GNUInstallDirs` / `CMakePackageConfigHelpers`，为 `aom` target 设置
  `PUBLIC_HEADER`、install 规则（含 `EXPORT unofficial-aom-targets`）并安装 CMake config 包
  （`cmake/aom-config.cmake.in` 为新增模板）；新增 `cmake/aom-config.cmake.in`。
- **目的**：aom 上游默认不安装 CMake 包配置文件，vcpkg 需要 `aom-config.cmake` 供下游 `find_package`
  解析。
- **兼容性**：additive install 规则，不改既有编译逻辑。重 patch 时需确认上游 `aom-config.cmake.in`
  模板路径与 `PUBLIC_HEADERS` 列表仍匹配。

- **文件**：`res/vcpkg/aom/aom-avx2.diff`
- **改动**：`build/cmake/cpu.cmake` 在 `ENABLE_AVX2` 时编译期探测 `__m256i` 是否可用，不可用时回退
  关闭 AVX2；并把 `xx_loadu_2x64` 从 `aom_dsp/x86/synonyms.h` 移除、补到 `synonyms_avx2.h`。
- **目的**：aom v3.9.0 起 AVX2 路径需要 `__m256i` 定义，旧编译器（如 MSVC）缺该类型时会编译失败，
  故加探测 + 回退。
- **兼容性**：仅作用于 x86 AVX2 分支。注意：`aom-avx2.diff` 默认在 aom 3.12.1 路径下**被注释关闭**
  （portfile 中 `# aom-avx2.diff`），仅在设 `USE_AOM_391=1`（拉 3.9.1）时才启用。重 patch 时按所选
  REF 决定该补丁的开关。

### libyuv 补丁

- **文件**：`res/vcpkg/libyuv/fix-cmakelists.patch`
- **改动**：`CMakeLists.txt` — 提升 `CMAKE_MINIMUM_REQUIRED` 到 3.12；删除 shared library 目标；
  仅构建 static 库 `yuv` 并加 `PUBLIC_HEADER`；用 `GLOB_RECURSE` 收集 `include/libyuv/*.h` 作为安装头
  文件；新增 `INSTALL(TARGETS ... EXPORT libyuv-targets ...)` 与 `INSTALL(EXPORT ...)` 以导出 CMake
  targets；修正 JPEG 链接到 static 库（`PUBLIC`）。
- **目的**：让 libyuv 上游 CMake 支持 vcpkg 的静态库安装与 `find_package` 推导（导出
  `libyuv-targets`）。
- **兼容性**：改动集中在 install/导出规则与库类型，不改变功能代码。重 patch 时需确认上游
  `CMakeLists.txt` 结构与 `include/libyuv/*.h` 头路径未变。

## 升级复核清单

升级上游 tag 时，按以下顺序核对：

1. `flutter/macos/Runner/Info.plist` — 确认 `NSSupportsAutomaticGraphicsSwitching` 键仍在。
2. `libs/hbb_common/src/config.rs` — 确认 `LowPowerMode` 结构体未被上游新增同名类型冲突。
3. `src/server/video_service.rs` — 确认 `create_capturer` 的 macOS 分支结构未变；若上游重构，需重新挂载 `low-power-mode` 读取点。
4. `libs/scrap/src/quartz/config.rs` 与 `common/quartz.rs` — 确认 `Config::low_power()` 与 `Capturer::new_with_config()` 仍在。
5. `src/platform/macos.rs` — 确认 `apply_low_power_mode` 函数仍在。
6. `flutter/lib/desktop/pages/desktop_setting_page.dart` — 确认 `lowPowerMode()` 卡片与其 General 挂载点仍在；若设置页重构，需重新挂载。
7. `libs/scrap/src/common/codec.rs` — 确认 `enable_hwcodec_option()` 内的 macOS `low-power-mode` 分支仍在。
8. `src/lang/template.rs` — 确认 `Low power mode`、`Enable low-power mode on dual-GPU Macs`、`low_power_mode_tip` 三个 key 仍在。
9. `flutter/lib/consts.dart` — 确认 `kOptionLowPowerMode` 常量仍在。
10. `src/lib.rs` — 确认 `#[cfg(feature = "toml-config-import")] mod config_import;` 仍在。
11. `Cargo.toml` — 确认 `toml-config-import` feature 仍在。
12. `src/core_main.rs` — 确认 `--import-toml-config` 分支与 `run_toml_import_from_args()` 两处仍在且 `#[cfg]` 门控完整。
13. `res/msi/` — 确认 `ImportTomlConfig` 自定义动作（`.cpp` / `.def` / `.vcxproj` / `CustomActions.wxs` / `RustDesk.wxs`）与 `ADDTOPATH` 相关属性、组件、对话框控件仍在。
14. `build.py` — 确认 `--toml-config-import` 参数与 `install.sh` 用法提示仍在。
15. `libs/hbb_common/src/config.rs` — 确认 `ServerConfig`、`Config2` 的 `rendezvous_servers` / `current_config_id`、`MultiServerStore` 独立存储、`ServerConfigRepository`、`ConfigManager` 等 `// dec: 多配置支持` 标记块仍在；重点确认 `Config2` 未被上游新增同名字段。
16. `src/ui.rs`、`src/ui_interface.rs`、`src/flutter_ffi.rs` — 确认 `// dec: 多配置支持` 的 IPC / glue / FFI 接口仍在。
17. `res/vcpkg/aom/portfile.cmake` 与 `res/vcpkg/aom/*.diff` — 确认三个补丁仍能 apply 到所选 aom REF
    （3.12.1 或 3.9.1）；`aom-avx2.diff` 的开关与所选 REF 一致（3.12.1 默认关、3.9.1 默认开）。
18. `res/vcpkg/libyuv/portfile.cmake` 与 `res/vcpkg/libyuv/fix-cmakelists.patch` — 确认补丁仍能 apply
    到 libyuv REF `0faf8dd0e004520a61a603a4d2996d5ecc80dc3f`，且 `PUBLIC_HEADER` 头路径未随上游变动。
19. `libs/hbb_common/src/config.rs` — 确认 `Config::get_stored_option` / `get_stored_options` 与 `MultiServerStore::try_save` 仍在，且 `save()` 仍转调 `try_save()`。
20. `src/core_main.rs` — 确认 `--import-toml-config` 分支内的 `is_cli_setting_change_disabled()` 检查、末尾 `publish_imported_store()` 与退出码 `5` 映射仍在。
21. `src/config_import/**` — 确认第二批改动的判定逻辑仍在（`toml-import-source` 记录式跳过、逐字段合并、写入前校验、`Option<bool>`、`BTreeMap`），以及与 `docs/toml-config-import.md` 的描述一致。
22. `libs/hbb_common/src/config.rs` — 确认 `set_default_config` 仍走「校验存在 + `promote_default` 置顶 + `save()`」，以及 `ConfigError` / `SwitchError` 的英文 `#[error(...)]` 文案未被上游改回。
23. `src/ui_interface.rs` — 确认 `translate_server_config_error` helper 与六处调用、两处 `ConfigError::ConfigNotFound` 仍在。
24. `src/lang/template.rs` — 确认第三批 18 个错误文案 key 与 `Move up` / `Move down` 仍在，且 `Set as default` 未被上游移除。
25. `flutter/lib/web/bridge.dart` — 确认 12 个多服务器接口仍在；上游若为该类的接口补了实现，需合并而不是简单覆盖。
