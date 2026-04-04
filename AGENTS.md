# AGENTS.md — AmiaBot 外置插件集合

> 面向 AI 编程代理的项目指南。阅读本文档可快速理解项目结构、接口约定和开发模式。

---

## 1. 项目概述

AmiaBot 是为 **NyaNyaBot** 宿主程序提供的一组外置插件（共 8 个）。每个插件作为**独立进程**运行，通过 **HashiCorp go-plugin** 框架（net/rpc 协议）与宿主通信。插件可响应 QQ 消息事件，执行 Bilibili/Pixiv/PJSK/Zeabur 等业务逻辑，并通过宿主调用 OneBot API 发送消息。

核心工作流：**事件解析 → 参数提取 → 配置读取 → 页面 URL 构建 → 调用截图插件 → 调用 blobserver 上传 → 发送消息**

---

## 2. 技术栈速查表

| 项目 | 版本/说明 |
|------|----------|
| Go | 1.25.6 |
| 模块路径 | `github.com/xiaocaoooo/amiabot` |
| go-plugin | `github.com/hashicorp/go-plugin` v1.7.0 |
| go-hclog | `github.com/hashicorp/go-hclog` v1.6.3 |
| PostgreSQL 驱动 | `github.com/lib/pq` v1.12.0 |
| 本地 SDK | `github.com/xiaocaoooo/amiabot-plugin-sdk`（replace 指向 `../amiabot-plugin-sdk`） |
| 通讯协议 | HashiCorp go-plugin + net/rpc |

---

## 3. 目录结构

```
AmiaBot/
├── go.mod                        # Go 模块定义，依赖本地 SDK (replace => ../amiabot-plugin-sdk)
├── go.sum
├── Makefile                      # 构建系统（8 个插件的编译目标）
├── .gitignore                    # 忽略 plugins/ 编译产物目录
├── cmd/                          # 每个插件一个子目录
│   ├── nyanyabot-plugin-amiabot-bilibili/
│   │   └── main.go               # Bilibili 链接识别插件
│   ├── nyanyabot-plugin-amiabot-pixiv/
│   │   ├── main.go               # Pixiv 作品识别插件
│   │   ├── utils.go              # 工具函数（URL 构建、消息发送、错误脱敏等）
│   │   └── main_test.go          # 测试
│   ├── nyanyabot-plugin-amiabot-pjsk-account/
│   │   ├── main.go               # PJSK 账户管理插件（数据库 CRUD）
│   │   ├── exports.go            # 跨插件导出方法实现（account.add/get/list/set_enabled/remove）
│   │   └── utils.go              # 工具函数
│   ├── nyanyabot-plugin-amiabot-pjsk-bind/
│   │   └── main.go               # PJSK 账户绑定插件（绑定/查询游戏 ID）
│   ├── nyanyabot-plugin-amiabot-pjsk-card/
│   │   ├── main.go               # PJSK 卡面查询插件
│   │   └── utils.go
│   ├── nyanyabot-plugin-amiabot-pjsk-event/
│   │   ├── main.go               # PJSK 活动查询插件
│   │   └── utils.go
│   ├── nyanyabot-plugin-amiabot-pjsk-song/
│   │   ├── main.go               # PJSK 歌曲查询插件（含模糊匹配）
│   │   ├── types.go              # 类型定义（MusicAlias, AliasData, MatchResult, AliasManager）
│   │   ├── alias_loader.go       # 别名数据加载器（远程+本地缓存）
│   │   ├── matcher.go            # 模糊搜索算法（精确→前缀/包含→子序列）
│   │   ├── matcher_test.go       # 匹配器测试
│   │   └── utils.go
│   └── nyanyabot-plugin-amiabot-zeabur-status/
│       └── main.go               # Zeabur 服务状态截图插件
└── plugins/                      # 编译产物目录（已 gitignore）
```

**命名规则**：插件目录名 = `nyanyabot-plugin-amiabot-<功能名>`，编译产物同名放入 `plugins/`。

---

## 4. 八个插件详细说明

### 4.1 Bilibili 链接识别 (`external.amiabot-bilibili`)

- **触发方式**：正则匹配消息中的 av 号、BV 号、b23.tv 短链
- **正则**：`(?i)\b(?:av(\d+)|(bv1[0-9a-zA-Z]+)|(?:(?:https?://)?b23\.tv/([a-z0-9]+)))\b`
- **MatchRaw**：`true`（匹配原始消息）
- **行为**：解析视频 ID → 调用截图插件截图 → 上传 blobserver → 发送图片 + 视频下载链接
- **文件**：`cmd/nyanyabot-plugin-amiabot-bilibili/main.go`（单文件，自包含）
- **依赖**：`external.screenshot`, `external.blobserver`
- **配置项**：`amiabot_pages`, `bilibili_downloader_server`

### 4.2 Pixiv 作品识别 (`external.amiabot-pixiv`)

- **触发方式**：正则匹配 `pixiv.net/artworks/:id` 链接
- **正则**：`(?i)\b(?:https?://)?(?:www\.)?pixiv\.net/(?:[a-z]{2}/)?artworks/(\d+)(?:[?#/][^\s]*)?`
- **MatchRaw**：`true`
- **行为**：截图信息卡 → 获取原图清单 → 多图时合并转发（forward）→ 单图直接发送
- **文件**：`cmd/nyanyabot-plugin-amiabot-pixiv/main.go` + `utils.go`
- **依赖**：`external.screenshot`, `external.blobserver`
- **配置项**：`amiabot_pages`, `amiabot_pages_download_base`

### 4.3 PJSK 账户管理 (`external.amiabot-pjsk-account`)

- **触发方式**：无命令，纯 Invoke 跨插件调用（被其他 PJSK 插件调用）
- **行为**：PostgreSQL 数据库 CRUD 操作
- **导出方法**：
  - `account.add` — 添加账户（已存在则设为启用）
  - `account.get` — 获取单个账户
  - `account.list_by_qq` — 按 QQ 号列出所有账户
  - `account.list_by_game_id` — 按游戏 ID 列出所有账户
  - `account.set_enabled` — 设置启用/禁用状态
  - `account.remove` — 删除账户
- **数据库表**：`pjsk_accounts`（主键 `qq_id + game_server + game_id`）
- **有效服务器**：`jp`, `cn`, `en`, `tw`, `kr`
- **文件**：`cmd/nyanyabot-plugin-amiabot-pjsk-account/main.go` + `exports.go` + `utils.go`
- **依赖**：无
- **配置项**：`database_url`（必填）, `default_server`

### 4.4 PJSK 账户绑定 (`external.amiabot-pjsk-bind`)

- **触发方式**：两个命令
  - **绑定命令**：`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?绑定(?P<id>\d+)$`
  - **ID查询命令**：`^(?:(?P<server>cn|jp|tw|en|kr)|(?:烤))id$`
- **示例**：`绑定12345`, `jp绑定12345`, `id`, `jpid`, `烤id`
- **MatchRaw**：`true`
- **行为**：
  - 绑定：调用 `account.add` 写入数据库 → 通过 pages `/pjsk/profile/raw` 获取用户名 → 返回 `绑定成功！\n[JP] <username>`
  - ID查询：指定 server 时返回该 server 的已启用 id，否则返回所有已启用的 id
- **文件**：`cmd/nyanyabot-plugin-amiabot-pjsk-bind/main.go`（单文件，自包含）
- **依赖**：`external.amiabot-pjsk-account`
- **配置项**：`amiabot_pages`（用于获取 profile 用户名）

### 4.5 PJSK 卡面查询 (`external.amiabot-pjsk-card`)

- **触发方式**：命令匹配 `card` 或 `查卡` + 编号
- **正则**：`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:card|查卡)(?P<id>[0-9]+)$`
- **示例**：`card1`, `jpcard1`, `cn查卡5`
- **MatchRaw**：`true`
- **行为**：构建页面 URL → 截图 → 上传 → 发送图片
- **文件**：`cmd/nyanyabot-plugin-amiabot-pjsk-card/main.go` + `utils.go`
- **依赖**：`external.screenshot`, `external.blobserver`
- **配置项**：`amiabot_pages`, `default_server`

### 4.6 PJSK 活动查询 (`external.amiabot-pjsk-event`)

- **触发方式**：命令匹配 `event` 或 `查活动` + 可选编号
- **正则**：`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:event|查活动)(?P<id>[0-9]*)$`
- **示例**：`event`（当前活动）, `jpevent50`, `cn查活动`
- **MatchRaw**：`true`
- **行为**：构建页面 URL → 截图 → 上传 → 发送图片
- **文件**：`cmd/nyanyabot-plugin-amiabot-pjsk-event/main.go` + `utils.go`
- **依赖**：`external.screenshot`, `external.blobserver`
- **配置项**：`amiabot_pages`, `default_server`

### 4.7 PJSK 歌曲查询 (`external.amiabot-pjsk-song`)

- **触发方式**：命令 `song` + 歌曲名称/别名
- **正则**：`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?song(?P<name>.+)$`
- **示例**：`songtyw`, `jpsong消失`, `song1`
- **MatchRaw**：`true`
- **行为**：模糊匹配歌曲名 → 构建页面 URL → 截图 → 上传 → 发送图片（多个匹配时发送候选列表）
- **模糊匹配三级策略**：
  1. **精确匹配**（weight=1.0）：标准化后完全匹配标题或别名
  2. **前缀/包含匹配**（weight=0.8/0.7）：查询词是标题/别名的前缀或子串
  3. **子序列匹配**（weight=0.5）：查询词字符按序出现在标题/别名中
- **标准化处理**：片假名→平假名、繁体→简体、移除特殊字符、转小写
- **别名数据源**：默认从 MoeSekai-Hub 远程获取，支持本地文件缓存
- **文件**：`cmd/nyanyabot-plugin-amiabot-pjsk-song/` 下 6 个文件
- **依赖**：`external.screenshot`, `external.blobserver`
- **配置项**：`amiabot_pages`, `default_server`, `alias_data_url`, `alias_cache_dir`, `alias_cache_ttl`

### 4.8 Zeabur 状态展示 (`external.amiabot-zeabur-status`)

- **触发方式**：精确匹配 `status` 或 `状态`
- **正则**：`(?i)^(status|状态)$`
- **MatchRaw**：`false`（匹配处理后的消息）
- **行为**：构建状态页 URL → 截图 → 上传 → 发送图片
- **文件**：`cmd/nyanyabot-plugin-amiabot-zeabur-status/main.go`（单文件，自包含）
- **依赖**：`external.screenshot`, `external.blobserver`
- **配置项**：`amiabot_pages`

---

## 5. 插件接口规范

每个插件必须实现 `plugin.Plugin` 接口（定义在 SDK 的 `plugin/api.go`）：

```go
type Plugin interface {
    // 返回插件元信息、命令/事件监听器、导出方法、配置 Schema
    Descriptor(ctx context.Context) (Descriptor, error)

    // 接收宿主下发的配置（初始加载 + WebUI 热更新）
    Configure(ctx context.Context, config json.RawMessage) error

    // 跨插件调用入口（其他插件通过宿主 Invoke 调用此方法）
    Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error)

    // 事件/命令分发入口（宿主匹配到监听器后调用）
    Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *CommandMatch) (HandleResult, error)

    // 优雅关闭（释放资源：goroutine、DB 连接、文件句柄等）
    Shutdown(ctx context.Context) error
}
```

**Descriptor 结构要点**：
- `PluginID`：全局唯一，格式 `external.amiabot-<功能名>`
- `Dependencies`：依赖的其他插件 PluginID 列表
- `Exports`：导出的跨插件方法（`ExportSpec{Name, Description, ParamsSchema, ResultSchema}`）
- `Config`：JSON Schema + 默认值，用于 WebUI 渲染配置表单
- `Commands`：命令监听器列表（`CommandListener{Name, ID, Pattern, MatchRaw, Handler}`）
- `Events`：事件监听器列表

**main() 标准启动模式**：
```go
func main() {
    plugin.Serve(&plugin.ServeConfig{
        HandshakeConfig: transport.Handshake(),
        Plugins: plugin.PluginSet{
            transport.PluginName: &transport.Map{PluginImpl: &YourPlugin{}},
        },
    })
}
```

**宿主交互接口**：
```go
type hostCaller interface {
    // 调用 OneBot API（发消息、获取信息等）
    CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error)
    // 调用其他插件的导出方法
    CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error)
}
// 获取宿主 RPC 客户端
host := transport.Host()
```

---

## 6. 构建、运行、测试命令

```bash
# 构建全部插件 → plugins/ 目录
make build

# 构建单个插件
make build-bilibili    # → plugins/nyanyabot-plugin-amiabot-bilibili
make build-pixiv       # → plugins/nyanyabot-plugin-amiabot-pixiv
make build-account     # → plugins/nyanyabot-plugin-amiabot-pjsk-account
make build-bind        # → plugins/nyanyabot-plugin-amiabot-pjsk-bind
make build-card        # → plugins/nyanyabot-plugin-amiabot-pjsk-card
make build-event       # → plugins/nyanyabot-plugin-amiabot-pjsk-event
make build-song        # → plugins/nyanyabot-plugin-amiabot-pjsk-song
make build-zeabur      # → plugins/nyanyabot-plugin-amiabot-zeabur-status

# 运行测试
make test              # go test ./...

# 代码格式化
make fmt               # go fmt ./...

# 整理依赖
make tidy              # go mod tidy

# 清理编译产物
make clean             # rm -rf plugins/
```

**宿主加载规则**：宿主启动时扫描 `./plugins` 目录，文件名以 `nyanyabot-plugin-` 开头且具备可执行权限的文件会被作为插件加载。

---

## 7. 插件间依赖关系

```
┌─────────────────────────┐
│      NyaNyaBot 宿主      │
└──────────┬──────────────┘
           │ go-plugin (net/rpc)
           ▼
┌──────────────────────────────────────────────┐
│  external.amiabot-bilibili ──依赖──→ external.screenshot    │
│  external.amiabot-pixiv    ──依赖──→ external.screenshot    │
│  external.amiabot-pjsk-card   ──依赖──→ external.screenshot │
│  external.amiabot-pjsk-event  ──依赖──→ external.screenshot │
│  external.amiabot-pjsk-song   ──依赖──→ external.screenshot │
│  external.amiabot-zeabur-status ──依赖──→ external.screenshot│
│                                                  │
│  external.amiabot-bilibili ──依赖──→ external.blobserver    │
│  external.amiabot-pixiv    ──依赖──→ external.blobserver    │
│  external.amiabot-pjsk-card   ──依赖──→ external.blobserver │
│  external.amiabot-pjsk-event  ──依赖──→ external.blobserver │
│  external.amiabot-pjsk-song   ──依赖──→ external.blobserver │
│  external.amiabot-zeabur-status ──依赖──→ external.blobserver│
│                                                  │
│  external.amiabot-pjsk-bind ──Invoke──→ external.amiabot-pjsk-account│
│  其他 PJSK 插件 ──Invoke──→ external.amiabot-pjsk-account   │
│  external.amiabot-pjsk-bind ──HTTP──→ amiabot-pages         │
└──────────────────────────────────────────────┘
```

- `external.screenshot`：截图服务（外部插件，不在本仓库）
- `external.blobserver`：文件上传/URL 转换服务（外部插件，不在本仓库）
- `external.amiabot-pjsk-account`：被其他 PJSK 插件通过 `CallDependency` → `Invoke` 调用

---

## 8. 配置项速查

| 配置项 | 类型 | 用途 | 使用插件 | 默认值 |
|--------|------|------|----------|--------|
| `amiabot_pages` | string | Amiabot Pages 服务地址 | 所有截图类插件、PJSK Bind | `""` |
| `amiabot_pages_download_base` | string | Pixiv 下载基地址 | Pixiv | `""`（回退 amiabot_pages） |
| `bilibili_downloader_server` | string | Bilibili 视频下载服务地址 | Bilibili | `""` |
| `database_url` | string | PostgreSQL 连接字符串 | PJSK Account（必填） | `""` |
| `default_server` | string | 默认游戏服务器 | PJSK Card/Event/Song/Account | `"jp"` |
| `alias_data_url` | string | 歌曲别名数据远程 URL | PJSK Song | MoeSekai-Hub URL |
| `alias_cache_dir` | string | 别名缓存目录 | PJSK Song | `"/tmp/nyanyabot"` |
| `alias_cache_ttl` | int | 别名缓存有效期（秒） | PJSK Song | `3600` |

配置通过 JSON Schema 声明，宿主 WebUI 可渲染配置表单，保存后通过 `Configure()` 热更新下发。

---

## 9. 开发约定

### 9.1 插件命名
- 目录/文件名：`nyanyabot-plugin-amiabot-<功能名>`
- PluginID：`external.amiabot-<功能名>`
- Makefile 目标：`build-<功能名>`

### 9.2 并发安全
- 主结构体使用 `sync.RWMutex` 保护共享配置（`cfg`）
- `Configure()` 写锁更新配置，`Handle()` 读锁读取配置
- `Handle()` 可能被宿主并发调用

### 9.3 panic 兜底
- 业务处理函数使用 `defer recover()` 防止插件进程崩溃
- panic 时通过 `sendError()` 向用户发送友好的错误消息

### 9.4 错误脱敏
- `sanitizeError()` 函数移除错误信息中的：
  - URL → `[服务地址]`
  - IP:端口 → `[内部地址]`
  - 文件路径 → `[路径]`
- 消息长度限制 100 字符

### 9.5 标准处理流程
```
1. json.Unmarshal(eventRaw, &evt)    // 解析事件
2. 提取 msgType, groupID, userID, rawMessage
3. e.mu.RLock() 读取配置             // 读锁保护
4. 解析参数（正则 / match.Groups）
5. 构建页面 URL（buildPagesURL）
6. 调用截图插件（buildScreenshotViaPlugin → CallDependency "external.screenshot"）
7. 调用 blobserver 上传（uploadViaBlobPlugin → CallDependency "external.blobserver"）
8. 发送消息（sendImage/sendVideo/sendText/sendForward → CallOneBot）
```

### 9.6 工具函数复用
各插件的 `utils.go` 中存在大量重复的工具函数（`sendImage`, `uploadViaBlobPlugin`, `buildScreenshotViaPlugin`, `normalizeHTTPBase`, `sanitizeError` 等）。这是**有意为之**——每个插件是独立编译的 `package main`，无法直接共享代码。如需修改这些函数，需要在**所有使用它们的插件中同步修改**。

### 9.7 配置 Schema
使用 JSON Schema 声明配置结构体，配合 `json.RawMessage` 类型的 `Default` 字段。`Configure()` 中推荐先准备带默认值的结构体，再 `Unmarshal` 覆盖。

---

## 10. 常见任务指引

### 如何添加新插件

1. 在 `cmd/` 下创建目录：`nyanyabot-plugin-amiabot-<功能名>/`
2. 创建 `main.go`，实现 `plugin.Plugin` 接口的 5 个方法
3. 创建 `utils.go`，复制通用工具函数（`sendImage`, `uploadViaBlobPlugin`, `buildScreenshotViaPlugin`, `normalizeHTTPBase` 等）
4. 在 `main()` 中使用标准 `plugin.Serve` 启动模式
5. 在 `Makefile` 的 `PLUGINS` 列表中添加新插件名
6. 添加对应的 `build-<功能名>` 目标
7. 更新 `go.mod`（如有新依赖）：`make tidy`

### 如何修改配置项

1. 修改插件结构体的 `cfg` 字段
2. 更新 `Descriptor()` 中的 `Config.Schema`（JSON Schema）和 `Config.Default`
3. 更新 `Configure()` 中的解析逻辑
4. 在业务处理函数中使用新配置项

### 如何添加新命令

1. 在 `Descriptor()` 的 `Commands` 列表中添加 `CommandListener`
2. 指定 `Name`, `ID`, `Pattern`（正则）, `MatchRaw`, `Handler`
3. 在 `Handle()` 中添加 `listenerID` 的 case 分支
4. 实现对应的处理函数

### 如何添加跨插件导出方法

1. 在 `Descriptor()` 的 `Exports` 列表中添加 `ExportSpec`（指定 `ParamsSchema` 和 `ResultSchema`）
2. 在 `Invoke()` 中添加 `method` 的 case 分支
3. 实现处理函数，接收 `json.RawMessage` 参数，返回 `json.RawMessage` 结果
4. 调用方通过 `host.CallDependency(ctx, "target.plugin.id", "method.name", params)` 调用

### 如何调试单个插件

```bash
# 构建单个插件
make build-bilibili

# 直接运行（宿主会通过 go-plugin 的子进程机制自动启动，无需手动运行）
# 但你可以在测试中独立调用各函数
go test ./cmd/nyanyabot-plugin-amiabot-pixiv/ -v
go test ./cmd/nyanyabot-plugin-amiabot-pjsk-song/ -v
```

### 如何修改工具函数

由于各插件是独立 `package main`，工具函数在多个 `utils.go` 中重复存在。修改时需要：
1. 确认哪些插件包含该函数（搜索全局）
2. 在**所有**包含该函数的文件中进行相同的修改
3. 常见的重复函数：`sendImage`, `sendText`, `sendError`, `sanitizeError`, `uploadViaBlobPlugin`, `buildScreenshotViaPlugin`, `normalizeHTTPBase`, `buildPagesURL`, `hostCaller` 接口定义

---

## 11. 项目间关系

```
┌─────────────────────┐
│    NyaNyaBot         │ ← 宿主程序（../NyaNyaBot）
│  (Plugin Host)       │
└────────┬────────────┘
         │ 加载 & go-plugin RPC
         ▼
┌─────────────────────┐
│    AmiaBot           │ ← 本仓库（8 个外置插件）
│  (Plugin Collection) │
└──┬──────────────┬───┘
   │              │
   ▼              ▼
┌──────────┐ ┌──────────────┐
│ SDK      │ │ amiabot-pages│
│ ../amiabot│ │ ../amiabot   │
│ -plugin- │ │ -pages       │
│ sdk      │ │              │
└──────────┘ └──────────────┘
```

| 项目 | 路径 | 关系 |
|------|------|------|
| **amiabot-plugin-sdk** | `../amiabot-plugin-sdk` | 本地依赖（go.mod replace），提供 Plugin 接口、OneBot 类型、transport 层 |
| **NyaNyaBot** | `../NyaNyaBot` | 宿主程序，加载并运行本仓库的插件 |
| **amiabot-pages** | `../amiabot-pages` | 页面渲染服务，插件截图的页面由它提供 |
| **external.screenshot** | 外部插件 | 截图服务，被大多数本仓库插件依赖 |
| **external.blobserver** | 外部插件 | 文件上传/URL 转换服务，被大多数本仓库插件依赖 |
