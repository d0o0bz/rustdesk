# RustDesk Flutter 构建镜像

本目录存放用于构建 RustDesk Flutter 版本的 Docker 相关文件。

## 文件说明

- `Dockerfile.flutter`：基于 `debian:12` 的构建镜像，预装 Rust 1.75、Flutter 3.24.5、
  vcpkg、flutter_rust_bridge_codegen 等工具链。仅烘焙工具链与系统依赖，项目源码在运行时
  通过 `-v "$PWD:/workspace"` 挂载（见 `build-flutter.sh`）。
- `flutter_linux_3.24.5-stable.tar.xz`：Flutter SDK 压缩包，**需预先下载**后随构建上下文
  COPY 进镜像，避免构建期从网络下载（国内访问官方源较慢）。
- `pre-download-vcpkg.sh`：在能访问 `github.com` 的机器上预下载 vcpkg 各端口所需的源码 tarball
  （`ffmpeg`/`opus`/`amd-amf`/`libvpx` 等），输出到 `docker/vcpkg-downloads/`，由 `run-flutter.sh`
  挂载进容器复用，避免构建环境直连 `github.com` 失败（详见下文"网络代理说明"）。`aom`/`libyuv`
  的源为本地 `file://` 仓库，不归此脚本管理。

## 构建镜像

构建上下文必须是仓库根目录，使 `docker/...` 路径可被 `COPY` 正确解析。直接用 `build-image.sh`
（自带最多 5 次重试，配合 Dockerfile 的 BuildKit 缓存挂载实现断点续传）：

```bash
cd /path/to/rustdesk
docker/build-image.sh            # 等价于下方 docker build，但失败自动重试 5 次
# 或手写：
docker build -f docker/Dockerfile.flutter -t rustdesk-flutter-builder .
```

> 镜像预装 `cmake 3.30.1`（vcpkg baseline 要求 ≥ 3.30.1，Debian 12 自带仅 3.25.1，会导致 vcpkg
> 在运行时自行下载 cmake）。构建时可通过 `--build-arg CMAKE_VERSION=3.30.1` 调整版本（默认 3.30.1）。
> cmake 默认经 `gh-proxy.com` 代理从 GitHub 下载（直连该域名在构建环境不通，`curl` 会报
> `exit code 18`）。若 gh-proxy.com 不可用，可把 `Dockerfile.flutter` 中 `CMAKE_URL` 的
> `https://gh-proxy.com/https://github.com/` 前缀改为其它可达的 GitHub 代理，或预先把
> `cmake-*-linux-x86_64.tar.gz` 放到本地后用 `COPY` 注入（类似 Flutter SDK 的做法）。

> 镜像还预装 `ninja`（≥1.12.1，Debian 12 自带 1.11.1 过低）、`meson`（1.8.2，Debian 12 自带
> 1.0.1 过低）、`pkgconf`。这些均为 vcpkg 运行时自动获取的工具，其 GitHub/pip 下载在代理环境同样
> 易失败（`curl exit 18`）。ninja 经 gh-proxy 从 GitHub 下载预编译包（版本由 `NINJA_VERSION`
> 构建参数控制，默认 1.12.1），meson 经清华大学 PyPI 镜像 `pip3 install`，pkgconf 来自 apt。
> 预装后 vcpkg 检测到版本满足即跳过运行时下载。

### 构建重试与断点续传

`Dockerfile.flutter` 的 cmake 下载层使用 BuildKit 缓存挂载
（`RUN --mount=type=cache,target=/opt/cmake-dl`），把下载的 `cmake.tgz` 落在构建缓存卷里而非容器
层内。因此：

- **单次 build 内**：`curl -C -` 在下载中断时自动从已下字节续传（`--retry 10` 兜底）。
- **跨 build**：标准 `docker build` 失败的层不保留容器内的中间文件，但缓存卷 `/opt/cmake-dl`
  在多次 build 间保留，所以重跑 build 时 `curl -C -` 能从缓存卷里已下载的部分继续，而非从头开始。

`docker/build-image.sh` 在 Docker 层外包了一层重试循环（最多 5 次，设 `DOCKER_BUILDKIT=1`），
与上面的缓存挂载配合：某次 build 因网络中断失败，下一次 build 会从断点续传，通常几次内成功。
该脚本接受任意 `docker build` 参数透传（如 `--build-arg`）。

## 使用镜像

```bash
docker run --rm -v "$PWD:/workspace" --memory=12g \
    rustdesk-flutter-builder /workspace/docker/build-flutter.sh
```

> 注意：必须使用该目录下 `Dockerfile.flutter` 构建出的镜像（`rustdesk-flutter-builder`）。
> 该镜像在 `/opt/vcpkg` 预装并 bootstrap 了 vcpkg，`build-flutter.sh` 在 `[3/5]` 步骤将其加入
> `PATH` 并调用 `vcpkg install`。若用其它镜像（/opt/vcpkg 不存在），该步骤会报
> `vcpkg: command not found`。

### 推荐：用 run-flutter.sh 运行（持久化 cargo git 缓存）

`build-flutter.sh` 会预克隆一个较大的 git 依赖 `rustdesk-org/wezterm`（用于 `portable-pty`），
其裸仓库落在容器内的 `/usr/local/cargo/git/db/...`。若直接用 `docker run --rm`，容器退出后该缓存
随容器销毁，**下次运行会重新经代理克隆 wezterm（大仓库、易断开）**。

本目录的 `run-flutter.sh` 会把 `/usr/local/cargo/git` 挂载到宿主机缓存目录
（`$HOME/.cache/rustdesk-flutter/cargo-git`，可用 `CARGO_GIT_CACHE` 环境变量覆盖），使该缓存在
多次 `docker run` 间保留，wezterm 只需克隆一次。注意**只挂载 `git` 子目录**，不要挂载整个
`CARGO_HOME`，以免覆盖容器内 `/usr/local/cargo/bin` 的工具链与 registry 镜像配置。

```bash
# 直接运行（输出追到 /tmp/build.log，另开终端 tail -f /tmp/build.log 观察）
docker/run-flutter.sh --memory=16g

# 等价于手写：
docker run --rm -v "$PWD:/workspace" \
    -v "$HOME/.cache/rustdesk-flutter/cargo-git:/usr/local/cargo/git" \
    -v "$HOME/.cache/rustdesk-flutter/cargo-registry:/usr/local/cargo/registry" \
    --memory=16g rustdesk-flutter-builder /workspace/docker/build-flutter.sh
```

cargo 的两类下载缓存都持久化到宿主机，避免每次重建容器重新拉取：

- `CARGO_GIT_CACHE`（默认 `~/.cache/rustdesk-flutter/cargo-git`）：挂载到 `/usr/local/cargo/git`，
  缓存 git 依赖（含庞大的 wezterm bare clone）。
- `CARGO_REG_CACHE`（默认 `~/.cache/rustdesk-flutter/cargo-registry`）：挂载到
  `/usr/local/cargo/registry`，缓存 crates.io 的 `.crate` 包与索引，省去 `cargo build` 时的
  `Downloading crates ...`。

> 注：只挂载 `git` 与 `registry` 两个子目录，不要挂载整个 `CARGO_HOME`，以免覆盖容器内
> `/usr/local/cargo/bin` 的工具链与 `config.toml` 中的 rsproxy 镜像配置。

`run-flutter.sh` 接受任意额外参数透传给 `docker run`（如 `--memory`、`--cpus`）；镜像名可用
`RUSTDESK_FLUTTER_IMAGE` 环境变量覆盖。

### 快速编译检查：用 check-flutter.sh 运行（仅 cargo check）

`run-flutter.sh` 会跑完整的 `build-flutter.sh`（子模块、pub get、bridge 代码生成、vcpkg、
`cargo build`）。若你只改了 Rust 侧代码、想快速验证能否编译（如开发 `toml-config-import`
功能时），可用 `docker/check-flutter.sh` 只跑 `cargo check`，省去 Flutter 端的耗时步骤。

该脚本复刻了 `build-flutter.sh` 中 `cargo check` 所必需的**全部环境**，因此不会重蹈手动
`cargo check` 因缺 vcpkg 头文件而失败的覆辙：

- 容器内设置 `VCPKG_ROOT=/opt/vcpkg`，并把 `/opt/vcpkg/installed` 软链到
  `/workspace/vcpkg_installed`（manifest 模式 vcpkg 安装目录），使 `magnum-opus` 等 crate
  能通过 vcpkg 找到 `opus/opus_multistream.h` 等 native 头文件；
- 设置 git `insteadOf` 代理（`gh-proxy.com`）、`CARGO_NET_GIT_FETCH_WITH_CLI=true`；
- 挂载与 `run-flutter.sh` 完全一致的缓存目录（cargo git / registry / vcpkg downloads /
  pub-cache），并设置 `CARGO_HOME` / `PUB_CACHE`。

```bash
# 默认：cargo check --features toml-config-import（会先 vcpkg install --triplet x64-linux）
docker/check-flutter.sh

# 已装过 vcpkg，跳过安装只做 check（复用 /workspace/vcpkg_installed）
SKIP_VCPKG=1 docker/check-flutter.sh

# 自定义 features
CARGO_FEATURES="toml-config-import,hwcodec" docker/check-flutter.sh

# 透传额外 docker run 参数（如限制内存）
docker/check-flutter.sh --memory=16g
```

可选环境变量：

- `CARGO_FEATURES`：传给 `cargo check --features` 的值，默认 `toml-config-import`。
- `CARGO_CHECK_ARGS`：追加到 `cargo check` 的额外参数（如 `--lib`、`--all-targets`）。
- `SKIP_VCPKG`：设为任意值跳过 `vcpkg install`（仅在 native 依赖已就绪时用，否则 check 会因
  找不到头文件而失败；首次运行需留空以触发安装）。
- `CARGO_GIT_CACHE` / `CARGO_REG_CACHE` / `VCPKG_DL_CACHE` / `PUB_CACHE`：同 `run-flutter.sh`，
  覆盖各宿主机缓存目录。
- `RUSTDESK_FLUTTER_IMAGE`：覆盖镜像名。

> 注：vcpkg 通过 manifest 模式装到挂载树内的 `/workspace/vcpkg_installed`，因此该目录在多次
> `docker run` 间持久保留，`SKIP_VCPKG=1` 后续可反复复用，无需每次重装。若 `vcpkg install`
> 因 github tarball 下载失败（构建环境直连 github 不通），请先按"vcpkg downloads 缓存"小节
> 预下载并放入 `docker/vcpkg-downloads/`，或确认该目录已同步到构建机。

### 步骤幂等（可重入）

脚本对前 3 个步骤做了"已完成即跳过"的哨兵标记，标记文件统一存放在挂载源码树内的
`/workspace/.build-flutter-markers/`，因此**容器重启后依然生效**，可省去重复的子模块拉取、
`flutter pub get`、bridge 代码生成：

- `[1/5]` git submodules → 标记 `submodule`
- `[1.5/5]` flutter pub get → 标记 `pubget`
- `[2/5]` flutter_rust_bridge codegen → 标记 `bridge`

如需强制重跑某一步，设置对应环境变量即可：

```bash
# 强制重跑整个流程（清空所有标记）
rm -rf /workspace/.build-flutter-markers

# 或仅强制重跑单个步骤
FORCE_SUBMODULE=1 FORCE_PUBGET=1 FORCE_BRIDGE=1 \
    docker run --rm -v "$PWD:/workspace" --memory=12g \
    rustdesk-flutter-builder /workspace/docker/build-flutter.sh
```

> 标记是"跑过即记为完成"，不校验产物有效性。若某步中途失败但前次成功过，可能误判为已完成；
> 遇到诡异问题时直接删除 `.build-flutter-markers` 目录全量重跑。

### 可选环境变量

- `JOBS`：cargo 并行任务数（默认自动）。
- `SKIP_VCPKG`：设为任意值可跳过 vcpkg 安装（仅在 native 依赖已就绪时使用，否则后续 cargo
  build 链接会失败）。
- `BUILD_DEB=1`：在 cargo build 成功后，额外运行 `flutter build linux --release`（生成
  `flutter/build/linux/x64/release/bundle/`）并调用 `build.py --flutter --skip-cargo` 打包，最终在
  仓库根产出 `rustdesk-<version>.deb`（经 `-v "$REPO_ROOT:/workspace"` 挂载对宿主机可见）。默认关闭。
  注意该 deb 不含 DRM 捕获变体；如需 `--drm` 变体需另行处理（见 `docs/upstream-patches.md`）。
- `FORCE_SUBMODULE` / `FORCE_PUBGET` / `FORCE_BRIDGE`：见上，强制重跑对应步骤。

## 离线预下载项

以下资源在 Dockerfile 中已尽量使用国内镜像，但若网络不通，可手动预下载后放入本目录：

- [flutter_linux_3.24.5-stable.tar.xz](https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_3.24.5-stable.tar.xz)：Flutter SDK 本体，放入 `docker/` 后由
  `Dockerfile.flutter` 通过 `COPY` 注入。

## 网络代理说明（Dockerfile 内已实现）

- 系统 apt：清华镜像（`mirrors.tuna.tsinghua.edu.cn`）。
- Rust / crates：rsproxy.cn。
- vcpkg git 仓库：Gitee 镜像（`gitee.com/mirrors/vcpkg`）。
- vcpkg 预编译二进制：bootstrap 脚本中硬编码的 GitHub 下载 URL 已通过 `sed` 改写为
  `ghproxy.net` 代理前缀，并增强 curl 重试（`--retry 10 --retry-all-errors -C -`）以应对
  慢速网络。
- Flutter SDK 本体：随构建上下文 `COPY` 本地预下载的压缩包注入（官方
  `storage.googleapis.com` 国内虽可达，但 661M 文件建议用迅雷等工具预先下载）。
  下载地址：

  ```text
  https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_3.24.5-stable.tar.xz
  ```

  下载后放入本目录 `docker/flutter_linux_3.24.5-stable.tar.xz` 即可被 `Dockerfile.flutter`
  通过 `COPY` 注入。
- Flutter precache / pub：清华镜像（`FLUTTER_STORAGE_BASE_URL` / `PUB_HOSTED_URL`）。
- **Cargo 的 GitHub git 依赖**：`libs/scrap/Cargo.toml` 中 `hwcodec`、`rust-webm`(webm)、
  `nokhwa` 三个 crate 以 `git = "https://github.com/rustdesk-org/..."` 形式引入，构建环境无法
  直连 `github.com:443`。`build-flutter.sh` 在运行时通过 git `insteadOf` 将
  `https://github.com/` 整体重定向到 `https://gh-proxy.com/https://github.com/`，由 gh-proxy.com
  透明代理（commit 哈希保持不变，cargo 锁定依赖可正常解析）。该代理已通过容器内 `git ls-remote https://gh-proxy.com/https://github.com/rustdesk-org/hwcodec` 实测可用；若其不可用，可将该行
  替换为其它 GitHub 代理前缀（如 `https://ghproxy.net/https://github.com/`，同样已验证）。

  `rust-webm` 还含一个嵌套子模块 `src/sys/libwebm`，其源为
  `https://chromium.googlesource.com/webm/libwebm`，构建环境无法直连、且 gh-proxy 对该源返回
  403。脚本额外通过 `insteadOf` 将其重定向到 GitHub 官方镜像
  `https://gh-proxy.com/https://github.com/webmproject/libwebm`（`webmproject/libwebm`，已实测
  经 gh-proxy 可达）。

  `hwcodec` 也含一个嵌套子模块 `rustdesk-org/externals.git`，gh-proxy.com 对该路径返回
  `403 Web page content is not allowed`。脚本额外通过 `insteadOf` 将该子模块单独重定向到
  `https://ghproxy.net/https://github.com/rustdesk-org/externals.git`（ghproxy.net 可正常服务，
  已实测可达）。这条规则更具体，会优先于上方通用的 `github.com/` → `gh-proxy.com` 规则生效。

  vcpkg 的两个 overlay port 以 `vcpkg_from_git` 拉取源码，原源在构建环境（rustdesk-dev）均不可达，
  且 vcpkg 内部的 git fetch **不读取** git 全局 `insteadOf`，经代理的镜像也不可靠。因此均改用
  **本地 git 仓库**：`portfile.cmake` 的 `URL` 直接指向容器内 `/workspace` 挂载树下预克隆好的仓库，
  vcpkg 通过 `file://` 取源码，完全不走网络。

  - **aom**（`res/vcpkg/aom/portfile.cmake`）：原源 `aomedia.googlesource.com/aom`，本地仓库路径
    `docker/aom-src/aom_aomedia.googlesource.com`，需含两个 REF：
    - `3.12.1` = `10aece4157eb79315da205f39e19bf6ab3ee30d0`（默认）
    - `3.9.1` = `8ad484f8a18ed1853c094e7d3a4e023b2a92df28`（设 `USE_AOM_391=1` 时）
  - **libyuv**（`res/vcpkg/libyuv/portfile.cmake`）：原源 `chromium.googlesource.com/libyuv/libyuv`，
    本地仓库路径 `docker/libyuv-src/libyuv`，需含 REF
    `0faf8dd0e004520a61a603a4d2996d5ecc80dc3f`。

  这两个本地仓库需在**能访问原源的机器**（如本机MBP）上准备好，再拷贝到构建机：

  ```bash
  # 在能联网的机器上
  # aom（来自 googlesource）
  git clone https://aomedia.googlesource.com/aom docker/aom-src/aom_aomedia.googlesource.com
  git -C docker/aom-src/aom_aomedia.googlesource.com fetch origin \
      10aece4157eb79315da205f39e19bf6ab3ee30d0 \
      8ad484f8a18ed1853c094e7d3a4e023b2a92df28
  # libyuv（来自 GitHub）
  git clone https://github.com/libyuv/libyuv docker/libyuv-src/libyuv
  git -C docker/libyuv-src/libyuv fetch origin \
      0faf8dd0e004520a61a603a4d2996d5ecc80dc3f
  # 拷到构建机 rustdesk-dev 的同一相对路径下
  ```

  `build-flutter.sh` 在 `[check]` 阶段会分别检测两个目录存在且所需 REF 均可用，缺失则报错退出
  （容器内无法再克隆）。这两个目录已加入仓库根 `.gitignore`（`docker/aom-src/`、`docker/libyuv-src/`）。

  > 注：vcpkg 默认浅克隆（`--depth 1`）无法从本地非 bare 仓库解析固定 REF，脚本已通过
  > `export VCPKG_USE_SHALLOW=0` 强制全量克隆本地源；同时为容器 git 配置 `safe.directory "*"`，
  > 以免本地仓库属主（宿主机 uid）与容器内用户不一致触发 dubious ownership 报错。

- **vcpkg downloads 缓存（github tarball）**
  多个 vcpkg 端口（如 `amd-amf`、`ffmpeg` 及其依赖 `libvpx`/`libyuv`/`mfx-dispatch`/`opus`/
  `libjpeg-turbo` 等）直接从 `github.com` 下载源码 tarball，构建环境无法直连，且 **git 的
  `insteadOf` 不覆盖 curl 下载**。  解决方式：在能联网的机器上预下载这些 tarball，再放进宿主的
  `docker/vcpkg-downloads/`（默认；可用 `VCPKG_DL_CACHE` 环境变量覆盖），`run-flutter.sh` 会将其
  挂载到容器内 `/opt/vcpkg/downloads`。vcpkg 命中缓存即跳过网络。

  本目录提供了预下载脚本 `pre-download-vcpkg.sh`，在能访问 `github.com` 的机器上（如本机
  E.C.MBP）运行即可一次性拉齐所有所需 tarball。脚本已内置 `gh-proxy.com` 镜像前缀、按各 port
  的 `vcpkg_from_github REF` 拼出**精确文件名**（vcpkg 按文件名命中缓存，文件名不对则会重新走网络）、
  并用 `-s` 检查实现**幂等**（已存在的跳过）。下载后 `rsync` 到构建机即可：

  ```bash
  # 在能联网的机器上
  docker/pre-download-vcpkg.sh
  # 把结果同步到构建机 rustdesk-dev
  rsync -a --progress docker/vcpkg-downloads/ \
    d0o0b@10.35.1.223:/home/d0o0b/Documents/rustdesk-dev/docker/vcpkg-downloads/
  ```

  若不使用脚本而手写下载，文件名必须与 vcpkg 构建日志里显示的目标文件名完全一致（如
  `GPUOpen-LibrariesAndSDKs-AMF-v1.4.35.tar.gz`）。运行构建时若报 SHA512 校验失败，说明镜像内容
  与原文件不一致，需换源重新下载该 tarball。

  > 注意：`pre-download-vcpkg.sh` 只覆盖经 `github.com` tarball 下载的端口；`aom` 与 `libyuv`
  > 的源已改为本地 `file://` 仓库（见上文"网络代理说明"），需按对应小节在能联网的机器上
  > `git clone` 并 `rsync` 整个 `docker/aom-src/`、`docker/libyuv-src/` 目录，不受此脚本管理。

  部分大型仓库（如 `rustdesk-org/wezterm`，用于 `portable-pty`）经代理克隆时可能在中途断开
  （`early EOF` / `unexpected disconnect`）。脚本设置 `CARGO_NET_GIT_FETCH_WITH_CLI=true` 让
  cargo 改用系统 `git`（使上述 `insteadOf` 与重试配置生效），并调大 `http.postBuffer`、关闭
  `core.compression`、放宽 `lowSpeedTime`，以提升大仓库克隆的健壮性。

  对 `rustdesk-org/wezterm`（用于 `portable-pty`）这类特大仓库，cargo 没有 git 依赖的浅克隆
  开关，脚本会先用 `git clone --depth 1 --bare` 将其按锁定分支
  `rustdesk/pty_based_0.8.1` 预克隆进 cargo 的 git 缓存目录
  `${CARGO_HOME}/git/db/wezterm-af88228f2de8e1aa`，cargo 检测到本地已有即跳过完整网络拉取，
  从而避免大仓历史经代理传输中途断开。

## 路径与目录说明

- Flutter SDK 压缩包 `flutter_linux_3.24.5-stable.tar.xz` 解压到 `/opt` 后得到
  `/opt/flutter`，该目录本身即为 Flutter 框架的 git 仓库（含 `.git`）。
- dropdown-menu 补丁（`.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff`）
  在构建时 `COPY` 进 `/tmp` 并 `git apply` 到 `/opt/flutter`（**不是** `/opt/flutter/flutter`）。

## 注意事项

- `docker/flutter_linux_3.24.5-stable.tar.xz` 体积约 661M，请勿误提交进 git。建议加入
  `.gitignore`，例如：

  ```gitignore
  docker/flutter_linux_*-stable.tar.xz
  ```

- `docker/aom-src/` 与 `docker/libyuv-src/` 是 vcpkg 本地 `file://` 源（见上文"网络代理说明"），
  体积大且非项目源码，已加入仓库根 `.gitignore`，请勿提交进 git：

  ```gitignore
  docker/aom-src/
  docker/libyuv-src/
  ```

- `build-flutter.sh` 在挂载的源码树内生成步骤哨兵目录 `.build-flutter-markers/`（记录哪些构建
  步骤已完成，详见上文"步骤幂等"），同样建议加入 `.gitignore`：

  ```gitignore
  .build-flutter-markers/
  ```

- 构建上下文必须是仓库根目录，否则 `docker/...` 与 `.github/...` 的 `COPY` 路径无法解析。

- **Git 安全名单（dubious ownership）**：运行时挂载的 `/workspace`（源码）与 `/opt/flutter`
  （Flutter SDK git 仓库）其目录所有者与容器内运行用户不一致，Git 会拒绝执行
  `git submodule update`、`git apply` 等操作并报错 `fatal: detected dubious ownership
  in repository at ...`。镜像已在构建阶段通过以下命令将这两个目录加入全局安全名单，
  重新构建镜像后即可生效，无需在脚本中额外处理：

  ```dockerfile
  RUN git config --global --add safe.directory /workspace \
      && git config --global --add safe.directory /opt/flutter
  ```

  若更换挂载点后仍出现同样报错，将其路径也加入安全名单即可。

- **运行时网络前提**：构建容器需能访问 `gh-proxy.com`（用于代理 GitHub 的 git 依赖与 Flutter
  自更新探测）。若网络无法访问 ghproxy，请改用其它可达的 GitHub 代理前缀（改
  `build-flutter.sh` 中的 `insteadOf` 一行），或在能联网的机器上预先 clone 这三个仓库后通过
  `[patch]` / 本地路径方式离线注入。
