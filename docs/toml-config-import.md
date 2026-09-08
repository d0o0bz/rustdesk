# TOML 配置导入（install_config_import）

本组件通过命令行读取 `rustdesk-config-import.toml`，转换为内部配置后持久化，支持企业批量部署的配置预置。

## 启用方式

组件由 feature `toml-config-import` 门控，**默认不启用**：

```bash
cargo build --features toml-config-import
```

未启用时，所有新增代码不编译，上游行为完全不变。

## 配置文件规则

- **文件名**：MSI 自动导入时固定为 `rustdesk-config-import.toml`（与 MSI 同目录）；命令行导入不限制文件名。
- **位置**：任意，由命令行参数指定（相对路径按进程当前目录解析）
- **编码**：UTF-8，非 UTF-8 直接判为解析错误
- **大小上限**：1 MB，超限直接判为解析错误
- **格式**：TOML v1.0.0（`hbb_common` 依赖 `toml` 0.7）
- **路径安全**：路径中含 `..` 一律拒绝；Unix 下 `/etc/passwd`、`/etc/shadow` 也在拒绝名单中

## 导入跳过条件

以下情况 `import_from_path` 不会写入任何配置：

- **路径不是普通文件** → `TomlConfigNotFound`，退出码 **3**。
- **源文件未更新**：toml 的修改时间 `<=` 当前配置文件（`Config::file()`）的修改时间，视为已导入过，返回成功（退出码 0）。因此**重复导入同一份未改动的 toml 是空操作**；要强制重跑需 `touch` 一下 toml。
- **配置为空**：解析后 `is_empty()` 为真（所有服务器字段为空、端口为 0、三个子表等于默认值、`options` 与 `rendezvous_servers` 均为空），返回成功（退出码 0）。

读取时若遇到 `WouldBlock` / `PermissionDenied`，会重试 3 次（每次间隔 500 ms）再放弃。

## 配置文件示例

```toml
version = "1.0"

rendezvous_server = "rs1.rustdesk.com"
rendezvous_port = 21116   # 保留字段，当前不生效
relay_server = "relay.rustdesk.com"
relay_port = 21117        # 保留字段，当前不生效
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
| `version` | String | `"1.0"` | 配置格式版本。**当前不校验**：写错不报错，也不参与 `is_empty()` 判定 |
| `rendezvous_server` | String | `""` | ID/中继服务器地址，写入 `custom-rendezvous-server` |
| `rendezvous_port` | Integer | `0` | **保留字段，解析但不映射**，当前不写入任何配置项 |
| `relay_server` | String | `""` | 中继服务器地址，写入 `relay-server` |
| `relay_port` | Integer | `0` | **保留字段，解析但不映射**，当前不写入任何配置项 |
| `api_server` | String | `""` | API 服务器地址，写入 `api-server` |
| `security.password` | String | `""` | 永久密码（加密存储） |
| `security.access_mode` | String | `""` | 访问模式（full/view/disabled） |
| `security.enable_2fa` | Boolean | `false` | 启用双因素认证 |
| `security.whitelist_enabled` | Boolean | `false` | 启用白名单 |
| `security.whitelist` | Array | `[]` | 白名单设备 ID 列表 |
| `network.network_type` | String | `""` | 网络类型（direct/proxy），写入 `network-type` |
| `network.proxy.address` | String | `""` | SOCKS5 代理地址。**为空则整段 `[network.proxy]` 不写入** |
| `network.proxy.port` | Integer | `0` | SOCKS5 代理端口，与 address 拼成 `address:port` |
| `network.proxy.username` | String | `""` | SOCKS5 代理用户名 |
| `network.proxy.password` | String | `""` | SOCKS5 代理密码 |
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

> 顶层 `rendezvous_port` / `relay_port` 会被反序列化（也会让 `is_empty()` 返回 false），但 `FieldMapper` 不读取它们，因此**写了也不会生效**。端口请写在 `[[rendezvous_servers]]` 条目里。

### 多中继服务器

使用 TOML 数组表 `[[rendezvous_servers]]` 定义多个中继服务器，映射到 `MultiServerStore.rendezvous_servers`。`id_port`/`relay_port` 未指定时使用默认端口（21116/21117）。

#### 条目标识（id）

`id` 省略时自动生成 UUID。重复导入同一份配置时，合并按 **`id` 优先、`id_server` 兜底** 定位已有条目并更新它，因此不会因每次生成新的 UUID 而产生重复条目；更新时会**保留库里原有的 `id`**，避免 `current_config_id` 与既有默认标记失效。若希望跨版本稳定指向同一条目，建议在配置里显式写死 `id`。

合并是**整条替换**（`store.rendezvous_servers[i] = updated`），不是逐字段合并。因此：

- `id_port` / `relay_port` 省略时落到 `ServerConfig::default()` 的 21116 / 21117。
- `api_server` / `key` 省略时 `to_server_config` 产出 `None`，替换后**会清掉库里该条目已有的值**。重复导入已存在的条目时，这两项要和首次导入一样写全。

#### 默认项选取

- 某条 `is_default = true` 时，该条为默认（同时其余条目被取消默认，保持"唯一默认"）。
- **全部条目 `is_default` 均为 false 或省略，且单服务器配置（`custom-rendezvous-server`）为空时，自动取第一个 `rendezvous_servers` 条目为默认。**
- 单服务器配置非空时（顶层写了 `rendezvous_server`，或原本已有配置），**不改动默认项**——单服务器设置具有更高优先级。

若选中默认项时单服务器配置为空，导入会把该条目写入单服务器的 `custom-rendezvous-server`/`relay-server`/`api-server`/`key` 四个选项，使连接**立即切换到该服务器**；否则条目只是入库，连接仍沿用原有单服务器设置。

写入顺序是**先 `store.save()` 再 `apply_current`**，这样 apply 触发的同步钩子能按 `id_server` 找到刚入库的条目并更新它，而不是再追加一条。列表入库后还会调用 `MultiServerStore::publish_from_file_if_owner()`，把结果分发给服务进程与 UI 进程——仅 `store.save()` 不足以让其他进程看到新列表。

### 类型转换规则

- 布尔值 `true` → 内部存储 `"Y"`；**`false` 等同"未指定"，不写入**（不会写 `"N"`，也就无法用导入关掉一个已开启的开关）
- 数组 `["a","b"]` → 逗号分隔字符串 `"a,b"`
- 仅非空/为真/非默认值的字段才写入，未指定字段保留现有配置
- `[options]` 下的键会先记一条 `忽略未知配置项: <key>` 的 WARNING，随后**照原样写入**同名 option（即 passthrough，并非真的忽略）

## 命令行用法

```bash
# 从指定路径导入
rustdesk --import-toml-config /path/to/rustdesk-config-import.toml

# JSON 格式输出
rustdesk --import-toml-config /path/to/rustdesk-config-import.toml --json
```

路径必填：安装目录不再被扫描，省略路径会返回退出码 3。参数解析只认 `--json` 开关和**第一个不以 `--` 开头的参数**（作为路径），顺序不限，其余参数忽略。

输出规则：`--json` 时固定输出 `{"code": N, "message": "..."}` 到 stdout；否则成功信息走 stdout、失败信息走 stderr。无论哪种情况进程都以该退出码结束。

### 退出码

| 退出码 | 触发条件 |
|--------|------|
| 0 | 导入成功，**或**命中跳过条件（源文件未更新 / 配置为空） |
| 1 | 权限不足（`io::ErrorKind::PermissionDenied`） |
| 2 | 配置解析失败：语法错误、非 UTF-8、超过 1 MB、读取失败、路径安全校验拒绝 |
| 3 | 配置文件不存在（含路径不是普通文件、命令行未给路径） |
| 4 | 其他错误（`StoreError`） |

「跳过」与「成功」退出码相同，都是 0，只能从日志区分（`配置文件未更新，跳过导入` / `空 TOML 配置，跳过导入`）。

当前调用链下 **1 和 4 实际不会触发**：文件读取阶段的 I/O 错误都先经 `From<io::Error> for TomlParseError` 转成 `FileReadError`（落到 2），而 `merge_and_store()` 的所有写入都吞掉失败只记日志、永远返回 `Ok`。也就是说读/写权限问题表现为退出码 **2**，而不是 1。

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

- 配置写入**登录用户**的 `%APPDATA%\RustDesk\config`，而不是安装器 SYSTEM 身份的配置——这是通过 `WTSQueryUserToken` + `CreateProcessAsUser` 在用户会话中启动导入实现的。只处理 `WTSActive` 状态的会话（控制台会话和活动 RDP 会话都算），多用户登录时每个用户各导入一份；会话已存在用户 profile，无需 `LoadUserProfile`。
- 每个会话的导入最多等 **60 秒**，超时不杀进程也不失败安装，只记一条日志继续。
- 未找到 toml、无活动用户会话或导入失败都**只记日志、不中断安装**；安装日志中搜索 `ImportTomlConfig` 可查看细节（退出码含义见上文退出码表）。
- toml 不会被复制或删除，导入是幂等的：条目按 `id` / `id_server` 合并，且未改动的 toml 会被 mtime 检查直接跳过（见「导入跳过条件」）。
- 该步骤仅在全新安装和升级时执行，卸载时不执行。
- 若 MSI 同目录没有 toml，仍可用命令行手动导入（见上文「命令行用法」与「PATH 选项」）。

## Linux / macOS

`--toml-config-import` 构建仍会生成 `install.sh`，但它只负责安装包本身，**不再复制 toml**——安装目录已不被扫描，复制过去也不会被读取。

导入必须在**登录用户**身份下执行（不要用 sudo，否则会写进 root 的配置而非用户配置）：

```bash
rustdesk --import-toml-config /path/to/rustdesk-config-import.toml
```

`install.sh` 安装完成后会打印上述用法提示。
