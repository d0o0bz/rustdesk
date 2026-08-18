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

## 升级复核清单

升级上游 tag 时，按以下顺序核对：

1. `flutter/macos/Runner/Info.plist` — 确认 `NSSupportsAutomaticGraphicsSwitching` 键仍在。
2. `libs/hbb_common/src/config.rs` — 确认 `LowPowerMode` 结构体未被上游新增同名类型冲突。
3. `src/server/video_service.rs` — 确认 `create_capturer` 的 macOS 分支结构未变；若上游重构，需重新挂载 `low-power-mode` 读取点。
4. `libs/scrap/src/quartz/config.rs` 与 `common/quartz.rs` — 确认 `Config::low_power()` 与 `Capturer::new_with_config()` 仍在。
5. `src/platform/macos.rs` — 确认 `apply_low_power_mode` 函数仍在。
6. `flutter/lib/consts.dart` — 确认 `kOptionLowPowerMode` 常量仍在。
