# TOML 配置导入（install_config_import）

本组件通过命令行读取 `rustdesk-config-import.toml`，转换为内部配置后持久化，支持企业批量部署的配置预置。

## 启用方式

组件由 feature `toml-config-import` 门控，**默认不启用**：

```bash
cargo build --features toml-config-import
```

未启用时，所有新增代码不编译，上游行为完全不变。

## 配置文件规则

- **文件名**：`rustdesk-config-import.toml`（固定）
- **位置**：任意，由命令行参数指定
- **编码**：UTF-8
- **大小上限**：1 MB
- **格式**：TOML v1.0.0

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
api_server = "api.rustdesk.com"
key = "your_server_public_key"
is_default = true

[[rendezvous_servers]]
name = "Backup"
id_server = "rs2.rustdesk.com"
relay_server = "relay2.rustdesk.com"
key = "backup_server_public_key"

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
| `rendezvous_servers[].id` | String | 自动生成 | 条目唯一标识。省略时自动生成 UUID；显式指定可在重复导入时稳定复用同一条目 |
| `rendezvous_servers[].name` | String | `""` | 服务器显示名称 |
| `rendezvous_servers[].id_server` | String | `""` | ID/中继服务器地址 |
| `rendezvous_servers[].id_port` | Integer | `21116` | ID 服务器端口 |
| `rendezvous_servers[].relay_server` | String? | `None` | 中继服务器地址 |
| `rendezvous_servers[].relay_port` | Integer? | `21117` | 中继服务器端口 |
| `rendezvous_servers[].api_server` | String? | `None` | API 服务器地址 |
| `rendezvous_servers[].key` | String? | `None` | 服务器公钥，自建服务器必填，否则无法建立加密连接 |
| `rendezvous_servers[].is_default` | Boolean | `false` | 是否默认服务器 |

### 多中继服务器

使用 TOML 数组表 `[[rendezvous_servers]]` 定义多个中继服务器，映射到 `MultiServerStore.rendezvous_servers`。`id_port`/`relay_port` 未指定时使用默认端口（21116/21117）。

#### 条目标识（id）

`id` 省略时自动生成 UUID。重复导入同一份配置时，合并按 **`id` 优先、`id_server` 兜底** 定位已有条目并更新它，因此不会因每次生成新的 UUID 而产生重复条目；更新时会**保留库里原有的 `id`**，避免 `current_config_id` 与既有默认标记失效。若希望跨版本稳定指向同一条目，建议在配置里显式写死 `id`。

#### 默认项选取

- 某条 `is_default = true` 时，该条为默认（同时其余条目被取消默认，保持"唯一默认"）。
- **全部条目 `is_default` 均为 false 或省略，且单服务器配置（`custom-rendezvous-server`）为空时，自动取第一个 `rendezvous_servers` 条目为默认。**
- 单服务器配置非空时（顶层写了 `rendezvous_server`，或原本已有配置），**不改动默认项**——单服务器设置具有更高优先级。

若选中默认项时单服务器配置为空，导入会把该条目写入单服务器的 `custom-rendezvous-server`/`relay-server`/`api-server`/`key` 四个选项，使连接**立即切换到该服务器**；否则条目只是入库，连接仍沿用原有单服务器设置。

### 类型转换规则

- 布尔值 `true`/`false` → 内部存储 `"Y"`/`"N"`
- 数组 `["a","b"]` → 逗号分隔字符串 `"a,b"`
- 仅非默认/非空字段才写入，未指定字段保留现有配置

## 命令行用法

```bash
# 从指定路径导入
rustdesk --import-toml-config /path/to/rustdesk-config-import.toml

# JSON 格式输出
rustdesk --import-toml-config /path/to/rustdesk-config-import.toml --json
```

路径必填：安装目录不再被扫描，省略路径会返回退出码 3。

### 退出码

| 退出码 | 含义 |
|--------|------|
| 0 | 导入成功 |
| 1 | 权限不足 |
| 2 | 配置解析失败 |
| 3 | 配置文件不存在 |
| 4 | 未知错误 |

## Windows 安装器 PATH 选项

MSI 安装界面在「Create desktop icon」下方提供 **Add to PATH（command line usage）** 勾选项，勾选后安装目录会被追加到**系统 PATH**（`Part="last"`，不覆盖已有值；卸载时自动移除）。**默认勾选**。

勾选后可在任意目录直接运行：

```powershell
RustDesk --import-toml-config .\rustdesk-config-import.toml
```

静默部署时用命令行参数控制，无需交互：

```powershell
msiexec /i rustdesk-1.4.9-x86_64.msi ADDTOPATH=1 /qn
```

勾选状态写入注册表（`HKCR\$(RegKeyRoot)\ADDTOPATH`），升级或修改安装时会保留。注意：**已打开的终端不会自动获得新 PATH**，MSI 广播 `WM_SETTINGCHANGE` 后需新开终端。

## Windows 安装器自动导入

双击 MSI 安装时，安装器会在 `InstallFiles` 之后自动查找**与 MSI 同目录**的 `rustdesk-config-import.toml`，找到则在**每个已登录用户会话**中执行导入：

```
RustDesk.exe --import-toml-config "<MSI 所在目录>\rustdesk-config-import.toml"
```

说明：

- 配置写入**登录用户**的 `%APPDATA%\RustDesk\config`，而不是安装器 SYSTEM 身份的配置——这是通过 `WTSQueryUserToken` + `CreateProcessAsUser` 在用户会话中启动导入实现的（控制台会话和活动 RDP 会话都会处理，多用户登录时每个用户各导入一份）。
- 未找到 toml、无活动用户会话或导入失败都**只记日志、不中断安装**；安装日志中搜索 `ImportTomlConfig` 可查看细节。
- toml 不会被复制或删除，导入是幂等的（按条目 `id` 合并，见上文）。
- 该步骤仅在全新安装和升级时执行，卸载时不执行。
- 若 MSI 同目录没有 toml，仍可用命令行手动导入（见上文「命令行用法」与「PATH 选项」）。

## Linux / macOS

`--toml-config-import` 构建仍会生成 `install.sh`，但它只负责安装包本身，**不再复制 toml**——安装目录已不被扫描，复制过去也不会被读取。

导入必须在**登录用户**身份下执行（不要用 sudo，否则会写进 root 的配置而非用户配置）：

```bash
rustdesk --import-toml-config /path/to/rustdesk-config-import.toml
```

`install.sh` 安装完成后会打印上述用法提示。
