# AmiaBot

面向 **NyaNyaBot** 的外部插件集合（Rust edition 2024 重写版）。

每个插件都是独立的 `nyanyabot-plugin-*` 进程，通过 [`nyanyabot-proto`](https://github.com/xiaocaoooo/nyanyabot-proto) 走本机 gRPC。公共 HTTP/WS/DB 辅助在 `crates/plugin-common`（只依赖 proto 运行时，不依赖宿主 crate）。

## 插件列表（15）

| 二进制 | 典型插件 ID | 说明 |
|--------|-------------|------|
| `nyanyabot-plugin-screenshot` | `external.screenshot` | 导出 `screenshot.build_url` |
| `nyanyabot-plugin-blobserver` | `external.blobserver` | 导出 `blob.upload_remote` |
| `nyanyabot-plugin-amiabot-bilibili` | `external.amiabot-bilibili` | B 站链接解析/截图 |
| `nyanyabot-plugin-amiabot-pixiv` | `external.amiabot-pixiv` | Pixiv 作品 |
| `nyanyabot-plugin-amiabot-gallery` | `external.amiabot-gallery` | 图库命令 |
| `nyanyabot-plugin-amiabot-onebot-websocket-client` | `external.amiabot-onebot-websocket-client` | 正向 OneBot WS 桥 |
| `nyanyabot-plugin-amiabot-query` | `external.amiabot-query` | 用户/群查询页 |
| `nyanyabot-plugin-amiabot-zeabur-status` | `external.amiabot-zeabur-status` | Zeabur 状态截图 |
| `nyanyabot-plugin-amiabot-pjsk-account` | `external.amiabot-pjsk-account` | PJSK 账户存储（PostgreSQL） |
| `nyanyabot-plugin-amiabot-pjsk-bind` | `external.amiabot-pjsk-bind` | 账户绑定命令 |
| `nyanyabot-plugin-amiabot-pjsk-card` | `external.amiabot-pjsk-card` | 卡面查询 |
| `nyanyabot-plugin-amiabot-pjsk-event` | `external.amiabot-pjsk-event` | 活动查询 |
| `nyanyabot-plugin-amiabot-pjsk-song` | `external.amiabot-pjsk-song` | 歌曲模糊匹配/查询 |
| `nyanyabot-plugin-amiabot-pjsk-profile` | `external.amiabot-pjsk-profile` | Profile |
| `nyanyabot-plugin-amiabot-pjsk-b30` | `external.amiabot-pjsk-b30` | B30 |

Descriptor / 命令 ID / 导出方法名由各 crate 下 `snapshots/descriptor.json` 快照测试锁定。

## 目录结构

```text
AmiaBot/
  crates/plugin-common/           # 公共辅助（URL、脱敏、发送、JSONata 子集等）
  crates/nyanyabot-plugin-*/      # 每插件一个 binary crate
  crates/xtask/                   # cargo xtask stage
  plugins/                        # stage 后的 release 二进制
```

路径依赖：`nyanyabot-proto = { path = "../nyanyabot-proto" }`。

## 环境要求

- Rust **edition 2024** 工具链
- 并列检出 `nyanyabot-proto`（实际运行还需要 NyaNyaBot 宿主）
- 可选：PostgreSQL（`external.amiabot-pjsk-account`）
- 可选：外网（截图/Blob/上游 HTTP）

## 构建与 stage

```bash
# release 编译 15 个插件并复制到 ./plugins/
cargo xtask stage
```

Windows 下 xtask 会自动处理 `.exe` 后缀。

随后让 NyaNyaBot 宿主加载该目录（复制/软链到宿主 `plugins/`，或在 Docker 中挂载）。

两边都 stage 后的示例：

```text
NyaNyaBot/
  nyanyabot
  plugins/
    nyanyabot-plugin-builtin-status
    nyanyabot-plugin-echo
    nyanyabot-plugin-screenshot          # 来自 AmiaBot
    nyanyabot-plugin-blobserver
    ...
```

## 配置

插件配置写在宿主 `data/config.json` 的 `plugins.<plugin_id>` 下。

常见字段：

- `amiabot_pages` — 需要截图的页面基址
- screenshot / blobserver 服务地址（对应插件内）
- `database_url` — PJSK 账户插件的 PostgreSQL
- `url` / `access_token` / `event_filter` — 正向 OneBot WS 客户端（JSONata 风格子集过滤）

完整 schema/default 见各插件 Descriptor 与 snapshot JSON。

### 典型依赖

多数面向用户的插件依赖：

- `external.screenshot`
- `external.blobserver`

PJSK profile/B30 等还会依赖 `external.amiabot-pjsk-account`。

宿主根据 Descriptor 的 `dependencies` 做拓扑排序。

## 开发

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask stage
```

重点测试：

- 每个插件的 Descriptor 快照
- `plugin-common` 单测（URL、脱敏、JSONata 子集）
- screenshot URL 构造
- blobserver 本地 mock HTTP 上传
- 有数据库时的 PJSK CRUD：

```bash
export NYANYABOT_TEST_DATABASE_URI='postgres://user:pass@127.0.0.1:5432/db'
cargo test -p nyanyabot-plugin-amiabot-pjsk-account
```

### 修改插件时注意

1. 保持 `plugin_id`、listener id、export 名、二进制名稳定
2. 公共逻辑放进 `plugin-common`
3. Descriptor 变更后刷新快照：

```bash
UPDATE_SNAPSHOTS=1 cargo test -p nyanyabot-plugin-<name> descriptor_matches_snapshot
```

4. 用 stage 后的二进制在真实宿主上做端到端验证

## 协议提醒

插件必须：

1. 监听 `127.0.0.1:0`
2. stdout 只打印一行 readiness JSON
3. 日志只写 stderr
4. 校验 `x-nyanyabot-token`
5. 通过 `HostClient` 调 OneBot / 依赖插件（不能伪造 `caller_plugin_id`）

详见 [nyanyabot-proto 中文说明](https://github.com/xiaocaoooo/nyanyabot-proto/blob/main/README_zh.md)。

## 相关链接

- [NyaNyaBot](https://github.com/xiaocaoooo/NyaNyaBot) — 宿主
- [nyanyabot-proto](https://github.com/xiaocaoooo/nyanyabot-proto) — gRPC 与运行时
- English: [README.md](./README.md)

## 许可

MIT（见 workspace package 元数据）。
