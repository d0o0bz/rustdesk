# TOML 配置导入（install_config_import）

本组件在安装/启动时从安装目录读取 `rustdesk.toml`，转换为内部配置后持久化，支持企业批量部署的配置预置。

## 启用方式

组件由 feature `toml-config-import` 门控，**默认不启用**：

```bash
cargo build --features toml-config-import
```

未启用时，所有新增代码不编译，上游行为完全不变。

## 配置文件规则

- **文件名**：`rustdesk.toml`（固定）
- **位置**：应用程序安装目录下
- **编码**：UTF-8
- **大小上限**：1 MB
- **格式**：TOML v1.0.0

### 跨平台安装目录检测顺序

| 平台 | 候选路径（按优先级） |
|------|---------------------|
| Windows | 注册表 `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\RustDesk\InstallLocation` → 可执行文件所在目录 |
| macOS | `/Applications/RustDesk.app/Contents/MacOS/` → 可执行文件所在目录 |
| Linux | `/usr/bin/rustdesk/` → `/opt/rustdesk/` → `$XDG_DATA_DIRS` 下 `rustdesk/` → 可执行文件所在目录 |

## 配置文件示例

```toml
version = "1.0"

rendezvous_server = "rs1.rustdesk.com"
rendezvous_port = 21116
relay_server = "relay.rustdesk.com"
relay_port = 21117
api_server = "api.rustdesk.com"

[security]
password = "your_password"
access_mode = "full"
enable_2fa = false
whitelist_enabled = true
whitelist = ["device_id_1", "device_id_2"]

[network]
network_type = "direct"

[network.proxy]
address = "127.0.0.1"
port = 1080
username = "user"
password = "proxy_password"

[display]
image_quality = "high"
view_style = "scroll"
scroll_style = "auto"
show_remote_cursor = true
disable_audio = false
disable_clipboard = false

[[rendezvous_servers]]
name = "Primary"
id_server = "rs1.rustdesk.com"
id_port = 21116
relay_server = "relay1.rustdesk.com"
relay_port = 21117
is_default = true

[[rendezvous_servers]]
name = "Backup"
id_server = "rs2.rustdesk.com"
relay_server = "relay2.rustdesk.com"

[options]
custom_resolution = "1920x1080"
```

## 字段说明

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `version` | String | `"1.0"` | 配置格式版本 |
| `rendezvous_server` | String | `""` | 中继服务器地址 |
| `rendezvous_port` | Integer | `0` | 中继服务器端口 |
| `relay_server` | String | `""` | 中继服务器地址 |
| `relay_port` | Integer | `0` | 中继服务器端口 |
| `api_server` | String | `""` | API 服务器地址 |
| `security.password` | String | `""` | 永久密码（加密存储） |
| `security.access_mode` | String | `""` | 访问模式（full/view/disabled） |
| `security.enable_2fa` | Boolean | `false` | 启用双因素认证 |
| `security.whitelist_enabled` | Boolean | `false` | 启用白名单 |
| `security.whitelist` | Array | `[]` | 白名单设备 ID 列表 |
| `network.network_type` | String | `""` | 网络类型（direct/proxy） |
| `network.proxy.address` | String | `""` | SOCKS5 代理地址 |
| `network.proxy.port` | Integer | `0` | SOCKS5 代理端口 |
| `network.proxy.username` | String | `""` | SOCKS5 代理用户名 |
| `network.proxy.password` | String | `""` | SOCKS5 代理密码（加密存储） |
| `display.image_quality` | String | `""` | 图像质量 |
| `display.view_style` | String | `""` | 视图风格 |
| `display.scroll_style` | String | `""` | 滚动风格 |
| `display.show_remote_cursor` | Boolean | `false` | 显示远程光标 |
| `display.disable_audio` | Boolean | `false` | 禁用音频 |
| `display.disable_clipboard` | Boolean | `false` | 禁用剪贴板 |
| `options.*` | String | — | 扩展选项（未知键记 WARNING 后写入） |
| `rendezvous_servers[].name` | String | `""` | 服务器显示名称 |
| `rendezvous_servers[].id_server` | String | `""` | ID/中继服务器地址 |
| `rendezvous_servers[].id_port` | Integer | `21116` | ID 服务器端口 |
| `rendezvous_servers[].relay_server` | String? | `None` | 中继服务器地址 |
| `rendezvous_servers[].relay_port` | Integer? | `21117` | 中继服务器端口 |
| `rendezvous_servers[].is_default` | Boolean | `false` | 是否默认服务器 |

### 多中继服务器

使用 TOML 数组表 `[[rendezvous_servers]]` 定义多个中继服务器，映射到 `Config2.rendezvous_servers`。导入时按 `id` 合并：已存在则更新，否则追加。`id` 未指定时自动生成 UUID，`id_port`/`relay_port` 未指定时使用默认端口（21116/21117）。

### 类型转换规则

- 布尔值 `true`/`false` → 内部存储 `"Y"`/`"N"`
- 数组 `["a","b"]` → 逗号分隔字符串 `"a,b"`
- 仅非默认/非空字段才写入，未指定字段保留现有配置

## 命令行用法

```bash
# 从安装目录自动检测并导入
rustdesk --import-toml-config

# 从指定路径导入
rustdesk --import-toml-config /path/to/rustdesk.toml

# JSON 格式输出
rustdesk --import-toml-config --json
```

### 退出码

| 退出码 | 含义 |
|--------|------|
| 0 | 导入成功 |
| 1 | 权限不足 |
| 2 | 配置解析失败 |
| 3 | 配置文件不存在 |
| 4 | 未知错误 |

## 自动导入

启用 feature 后，应用每次启动时会在 `core_main()` 初始化阶段自动调用 `ConfigImporter::import_from_install_dir()`，检测安装目录下的 `rustdesk.toml` 并导入。通过修改时间窗检查保证幂等性（源文件需比现有配置新且比可执行文件旧才会重新导入）。
