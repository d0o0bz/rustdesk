# 兆芯 GPU 上窗口缩放 / 最大化闪退

在 deepin 25 + 兆芯（Zhaoxin）GPU 的组合上，RustDesk 主窗口**改变大小或点击最大化时立刻闪退**，但进程可以通过托盘重新打开主界面。

本文记录完整的定位过程、根因，以及解决方法。文中命令均以普通用户执行，需要 root 的已加 `sudo`。

## 现象

- 拖动主窗口边缘、或点击最大化 → 整个应用瞬间退出（SIGSEGV）。
- 启动后不做任何改变尺寸的操作时，可以正常使用。
- 托盘进程不受影响，可以再次打开主界面。

## 环境

```
XDG_SESSION_TYPE=x11
桌面环境：Deepin
GPU：ZX C-960 / KX-6000G（兆芯）
Mesa：24.3.0-1deepin9（deepin 发行版自带）
```

## 定位过程

### 1. 看崩溃堆栈

崩溃由 RustDesk 自带的 SIGSEGV 处理器记录，日志在 `~/.local/share/logs/RustDesk/`：

```bash
tail -40 ~/.local/share/logs/RustDesk/rustdesk_rCURRENT.log
```

关键输出：

```
ERROR [libs/hbb_common/src/platform/mod.rs:50] Got signal 11 and exit. stack:
gdk_cairo_draw_from_gl
gtk_container_propagate_draw
gtk_container_propagate_draw
gtk_container_propagate_draw
gtk_main_do_event
g_signal_emit_valist
g_signal_emit
g_main_context_iteration
g_application_run
main
```

`gdk_cairo_draw_from_gl` 是 GDK 把 GL 内容合成到窗口的调用，被 `gtk_main_do_event` → `propagate_draw` 触发——正好对应"缩放/最大化引发整窗重绘"。

### 2. 确认不是 Wayland 的锅

```bash
echo "session=$XDG_SESSION_TYPE  wayland=$WAYLAND_DISPLAY  desktop=$XDG_CURRENT_DESKTOP"
```

输出 `session=x11` 说明走的是 X11，排除 GTK3/Wayland 的限制（`gtk_window_maximize` 等在 Wayland 下不受支持）。

### 3. 找出可疑的 GL 驱动

```bash
glxinfo -B 2>/dev/null | head -8
```

```
Vendor: Shanghai Zhaoxin Semiconductor Co., Ltd (0x1d17)
Device: ZX C-960 (0x3a04)
```

同时 journal 里有伴随症状（之前容易被当成无害噪音）：

```bash
journalctl -u rustdesk --no-pager | grep -E "zx video|libcuda"
```

```
[zx video error] unsupport config attributes: 39! @ get_config_attributes_default L228
Cannot load libcuda.so.1
```

`zx` 即兆芯。至此把嫌疑锁定到兆芯的 GL 驱动栈。

## 根因

兆芯的 GLX vendor 实现在做 GL 合成（`gdk_cairo_draw_from_gl`）时存在缺陷，窗口重绘即触发段错误。更新厂商驱动（如 `KX-6000G-Linux+OS_x64-26.00.52`）**不能**解决。

## 解决方法：改用标准 Mesa 的软件渲染

libglvnd 支持用环境变量强制指定 GLX vendor。切到标准 Mesa 后，由于 Mesa 不认兆芯的硬件，会自动回落到 llvmpipe（软件光栅化），从而绕开有问题的驱动。

### 1. 先验证

```bash
__GLX_VENDOR_LIBRARY_NAME=mesa glxinfo -B 2>/dev/null | grep -E "Vendor|Device|Accelerated"
```

期望：

```
Vendor: Mesa (0xffffffff)
Device: llvmpipe (LLVM 17.0.6, 256 bits) (0xffffffff)
Accelerated: no
```

再用这个变量启动 RustDesk，缩放 / 最大化确认不再闪退：

```bash
__GLX_VENDOR_LIBRARY_NAME=mesa rustdesk
```

> 注意：这里**不需要** `LIBGL_ALWAYS_SOFTWARE=1`，也不需要开启 RustDesk 的
> "Always use software rendering" 选项。原因见下一节。

### 2. 固化

需要覆盖两条启动链。

**其一，服务链**（`--service → --server → --tray → 主界面`，服务用 `sudo -E` 传环境，所以整条链都会继承）：

```bash
sudo mkdir -p /etc/systemd/system/rustdesk.service.d
sudo tee /etc/systemd/system/rustdesk.service.d/gl.conf <<'EOF'
[Service]
Environment="__GLX_VENDOR_LIBRARY_NAME=mesa"
EOF
sudo systemctl daemon-reload
sudo systemctl restart rustdesk
```

用 drop-in 而非直接改 `/usr/lib/systemd/system/rustdesk.service`，这样包升级不会被覆盖。

**其二，从应用菜单 / 启动器启动**（这条链不经过服务）：

```bash
mkdir -p ~/.local/share/applications
cp /usr/share/applications/rustdesk.desktop ~/.local/share/applications/
sed -i 's|^Exec=rustdesk %u|Exec=env __GLX_VENDOR_LIBRARY_NAME=mesa rustdesk %u|g' \
    ~/.local/share/applications/rustdesk.desktop
update-desktop-database ~/.local/share/applications 2>/dev/null
```

原 desktop 文件里 `Exec=` 出现两次（主入口和 "Open a New Window" action），所以 `sed` 必须带 `g`。用户级目录优先级高于 `/usr/share/applications`，且不会被 RustDesk 清除（RustDesk 只清理 `~/.config/autostart/` 下的自启项）。

### 3. 确认生效

```bash
systemctl show rustdesk -p Environment
grep -n "^Exec" ~/.local/share/applications/rustdesk.desktop
```

deepin 的启动器可能缓存 desktop 文件，若菜单启动未生效，注销重登录一次即可。

### 撤销

删除 `/etc/systemd/system/rustdesk.service.d/gl.conf` 和 `~/.local/share/applications/rustdesk.desktop` 即可。

### 关于性能

该变量只影响 GL（界面合成），VA-API 视频编解码走独立路径，因此远程画面的帧率不受软件渲染拖累。

## 踩坑记录：为什么不能用 `allow-always-software-render`

RustDesk 设置页里有个 Linux 专有项 "Always use software rendering"（选项 key
`allow-always-software-render`），开启后会在启动时设置 `LIBGL_ALWAYS_SOFTWARE=1`
（见 `src/core_main.rs`）。在这类机器上**不要用它**，原因是：

1. 兆芯的 mesa loader 只加载 `*_vndri.so` 后缀的模块：

   ```
   MESA-LOADER: failed to open swrast: /usr/lib64/dri/swrast_vndri.so: No such file or directory
   libGL error: failed to load driver: swrast
   ```

2. 而系统里安装的是 deepin 自带的标准 Mesa，只提供 `swrast_dri.so`：

   ```bash
   ls /usr/lib/x86_64-linux-gnu/dri/ | grep -E "swrast|vndri"
   # kms_swrast_dri.so
   # swrast_dri.so
   # cx4_vndri.so        <- 兆芯的硬件驱动
   ```

   没有任何 `swrast_vndri.so`，`apt install libgl1-mesa-dri` 也补不出来——这不是缺包，是两套 mesa 的模块命名不兼容。

3. 结果：开启软件渲染后 GL 上下文创建失败（`X_GLXCreateNewContext` 报 `BadValue`），
   Flutter 起不来，**主界面完全无法显示**——比崩溃更糟。

另外两个已验证无效的手段，可不必再试：

- `GSK_RENDERER=cairo`：只影响 GTK 自己的 GSK 渲染器，崩溃点在 Flutter embedder
  的 GL 合成路径，不受它管辖。
- `GALLIUM_DRIVER=llvmpipe`：被兆芯的 loader 忽略，`glxinfo` 仍显示兆芯硬件。

## 自愈机制的改进

`libs/hbb_common/src/platform/mod.rs` 的 SIGSEGV 处理器会在识别出显卡相关堆栈时
自动开启 `allow-always-software-render`。原检测列表没有覆盖 `gdk_cairo_draw_from_gl`，
所以这类崩溃不会被自动规避。

现已补上，并加了保护：

- 只有系统里确实存在**该机器 loader 能加载的**软件 GL 驱动时才开启软件渲染
  （`software_gl_available()` 会同时识别 `_dri.so` 与 `_vndri.so` 两种命名，
  避免上文"有 `swrast_dri.so` 却加载不了"的误判）；
- 若软件渲染已经开启却仍然崩溃，则**回退**关闭它并提示改用其他手段，
  避免把用户留在"主界面完全打不开"的状态；
- 两者都不满足时只记录并提示更新显卡驱动。

## 相关：deepin 上托盘图标不显示

排查同一台机器时遇到的另一个问题，一并记录。

**现象**：`ps` 里看不到 `rustdesk --tray` 进程，托盘图标也不出现。

**原因**：托盘的启动条件是存在 `rustdesk --server` 进程（`src/core_main.rs`），
而 `--server` 只由 root 的 `--service` 在检测到用户登录后 spawn。若服务没起来，
托盘永远不会出现。

服务起不来的常见原因是配置里的 `stop-service` 为 `Y`——`start_os_service()` 一启动
就会调 `check_if_stop_service()`，发现该选项便执行
`systemctl disable` + `systemctl stop` 把自己关掉（`src/platform/linux.rs`）。
表现是 `systemctl enable --now` 明明创建了 symlink，服务却立刻变回 `disabled` + `inactive (dead)`。

**注意选项的存储位置**：`Config::get_option` / `set_option` 读写的是 **Config2**，
即 `RustDesk2.toml`，**不是** `RustDesk.toml`（后者只存 `enc_id` / `salt` / `key_pair` 等）。
改配置或排查时找错文件会白费功夫。

排查与修复：

```bash
sudo systemctl status rustdesk --no-pager
sudo grep -rnE "stop-service" /root/.config/rustdesk/     # 服务以 root 运行，读 root 的配置
sudo rustdesk --option stop-service ""                    # 清空
sudo systemctl enable --now rustdesk
ps -eo pid,user,args | grep rustdesk | grep -v grep
```

正常应看到 `rustdesk --service`(root)、`rustdesk --server`、`rustdesk --tray` 三个进程。

两个补充点：

- 服务一旦运行，RustDesk 会在每次启动 GUI 时删除 `~/.config/autostart/rustdesk.desktop`
  （`check_autostart_config()`，见上游 issue #4863）。所以"启动器右键 → 开机自动启动"
  与 systemd 服务**二选一**，不要同时用。
- 想只要"登录后自动显示主界面"、不需要托盘和无人值守，用启动器的"开机自动启动"即可，
  不必启用 systemd 服务。
