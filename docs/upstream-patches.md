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

## install_config_import 模块（TOML 配置导入）

### 新增独立模块（无上游修改）

- **文件**：`src/config_import.rs` 及 `src/config_import/{error,toml_config,toml_parser,install_dir,field_mapper,importer}.rs`
- **改动**：新增配置导入组件，含 TOML 解析、字段映射、安装目录检测、合并存储。
- **兼容性**：纯新增模块，由 `#[cfg(feature = "toml-config-import")]` 门控，默认不启用，对上游零侵入。

### 上游文件修改

- **文件**：`src/lib.rs`
- **改动**：末尾新增 `#[cfg(feature = "toml-config-import")] mod config_import;`，带 `// dec: TOML配置导入` 注释。
- **兼容性**：feature 门控，未启用时不编译。

- **文件**：`Cargo.toml`
- **改动**：`[features]` 段新增 `toml-config-import = []`。
- **兼容性**：空 feature，默认不启用，不影响现有构建。

- **文件**：`src/core_main.rs`
- **改动**：三处 `#[cfg(feature = "toml-config-import")]` 门控的新增：① `core_main()` 开头自动导入钩子；② `--import-toml-config` 参数分支；③ `run_toml_import_from_args` 函数。均带 `// dec: TOML配置导入` 注释。
- **兼容性**：feature 门控，未启用时参数分支条件 `cfg!(feature)` 为 false 不匹配，行为不变。

## vcpkg overlay port 补丁（aom / libyuv）

这两个 overlay port 位于 `res/vcpkg/aom/`、`res/vcpkg/libyuv/`，通过 `vcpkg_from_git` 从
**本地 `file://` 仓库**（见 `docker/README.md` "网络代理说明"）取源码，PATCHES 字段在源码层打补丁。
升级 aom / libyuv 上游 tag 时，下列补丁需随上游改动重新核对或 rebase。

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
6. `flutter/lib/consts.dart` — 确认 `kOptionLowPowerMode` 常量仍在。
7. `src/lib.rs` — 确认 `#[cfg(feature = "toml-config-import")] mod config_import;` 仍在。
8. `Cargo.toml` — 确认 `toml-config-import` feature 仍在。
9. `src/core_main.rs` — 确认三处 `// dec: TOML配置导入` 标记点仍在且 `#[cfg]` 门控完整。
10. `res/vcpkg/aom/portfile.cmake` 与 `res/vcpkg/aom/*.diff` — 确认三个补丁仍能 apply 到所选 aom REF
    （3.12.1 或 3.9.1）；`aom-avx2.diff` 的开关与所选 REF 一致（3.12.1 默认关、3.9.1 默认开）。
11. `res/vcpkg/libyuv/portfile.cmake` 与 `res/vcpkg/libyuv/fix-cmakelists.patch` — 确认补丁仍能 apply
    到 libyuv REF `0faf8dd0e004520a61a603a4d2996d5ecc80dc3f`，且 `PUBLIC_HEADER` 头路径未随上游变动。
