# AmiaBot

External **NyaNyaBot** plugin suite, rewritten in Rust (edition 2024).

Each plugin is a separate `nyanyabot-plugin-*` process speaking local gRPC via [`nyanyabot-proto`](https://github.com/xiaocaoooo/nyanyabot-proto). Shared HTTP/WS/DB helpers live in `crates/plugin-common` (depends only on the proto runtime — no host crate coupling).

## Plugins (15)

| Binary | Plugin ID (typical) | Notes |
|--------|---------------------|--------|
| `nyanyabot-plugin-screenshot` | `external.screenshot` | `screenshot.build_url` export |
| `nyanyabot-plugin-blobserver` | `external.blobserver` | `blob.upload_remote` export |
| `nyanyabot-plugin-amiabot-bilibili` | `external.amiabot-bilibili` | Bilibili link → screenshot |
| `nyanyabot-plugin-amiabot-pixiv` | `external.amiabot-pixiv` | Pixiv artwork |
| `nyanyabot-plugin-amiabot-gallery` | `external.amiabot-gallery` | Gallery commands |
| `nyanyabot-plugin-amiabot-onebot-websocket-client` | `external.amiabot-onebot-websocket-client` | Forward OneBot WS bridge |
| `nyanyabot-plugin-amiabot-query` | `external.amiabot-query` | User/group query pages |
| `nyanyabot-plugin-amiabot-zeabur-status` | `external.amiabot-zeabur-status` | Zeabur status screenshot |
| `nyanyabot-plugin-amiabot-pjsk-account` | `external.amiabot-pjsk-account` | PJSK account store (PostgreSQL) |
| `nyanyabot-plugin-amiabot-pjsk-bind` | `external.amiabot-pjsk-bind` | Account bind commands |
| `nyanyabot-plugin-amiabot-pjsk-card` | `external.amiabot-pjsk-card` | Card query |
| `nyanyabot-plugin-amiabot-pjsk-event` | `external.amiabot-pjsk-event` | Event query |
| `nyanyabot-plugin-amiabot-pjsk-song` | `external.amiabot-pjsk-song` | Song fuzzy match + query |
| `nyanyabot-plugin-amiabot-pjsk-profile` | `external.amiabot-pjsk-profile` | Profile |
| `nyanyabot-plugin-amiabot-pjsk-b30` | `external.amiabot-pjsk-b30` | B30 |

Descriptor / command IDs / export names are covered by per-crate snapshot tests under `crates/*/snapshots/descriptor.json`.

## Layout

```text
AmiaBot/
  crates/plugin-common/           # shared helpers (URL, redact, send_*, jsonata subset, …)
  crates/nyanyabot-plugin-*/      # one binary crate per plugin
  crates/xtask/                   # cargo xtask stage
  plugins/                        # staged release binaries (after xtask)
```

Path dependency: `nyanyabot-proto = { path = "../nyanyabot-proto" }`.

## Requirements

- Rust edition **2024** toolchain
- Sibling checkout of `nyanyabot-proto` (and a NyaNyaBot host to actually run plugins)
- Optional PostgreSQL for `external.amiabot-pjsk-account`
- Optional network access for screenshot/blob/upstream HTTP APIs

## Build & stage

```bash
# release-build all 15 plugins and copy into ./plugins/
cargo xtask stage
```

Windows builds append `.exe` automatically in the xtask copier.

Then point the NyaNyaBot host at this directory (copy/symlink into the host `plugins/` folder, or mount both in Docker).

Example host-side layout after staging both repos:

```text
NyaNyaBot/
  nyanyabot
  plugins/
    nyanyabot-plugin-builtin-status
    nyanyabot-plugin-echo
    nyanyabot-plugin-screenshot          # from AmiaBot
    nyanyabot-plugin-blobserver
    ...
```

## Configuration

Plugin configs are stored under the host `data/config.json` key `plugins.<plugin_id>`.

Common patterns:

- `amiabot_pages` — base URL for HTML pages that get screenshotted
- `screenshot` / `blobserver` service endpoints (on those plugins)
- `database_url` — PJSK account plugin PostgreSQL URL
- `url` / `access_token` / `event_filter` — OneBot forward WS client (JSONata-like subset filter)

Exact schemas/defaults are in each plugin’s Descriptor (`config.schema` / `config.default`) and snapshot JSON.

### Typical dependency edges

Many user-facing plugins depend on:

- `external.screenshot`
- `external.blobserver`

PJSK profile/B30-style plugins also depend on `external.amiabot-pjsk-account`.

The host resolves dependency order from Descriptor `dependencies`.

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask stage
```

Highlights:

- Descriptor snapshot tests on every plugin crate
- `plugin-common` unit tests (URL join, secret redact, JSONata subset)
- screenshot URL builder tests
- blobserver local mock HTTP upload test
- PJSK account CRUD against PostgreSQL when available:

```bash
export NYANYABOT_TEST_DATABASE_URI='postgres://user:pass@127.0.0.1:5432/db'
cargo test -p nyanyabot-plugin-amiabot-pjsk-account
```

### Implementing / changing a plugin

1. Keep `plugin_id`, listener ids, export names, and binary name stable
2. Prefer helpers in `plugin-common` over duplicating host calls
3. Refresh descriptor snapshots when Descriptor changes:

```bash
UPDATE_SNAPSHOTS=1 cargo test -p nyanyabot-plugin-<name> descriptor_matches_snapshot
```

4. Run through a real host (`NyaNyaBot`) with staged binaries for end-to-end checks

## Protocol reminder

Plugins must:

1. Listen on `127.0.0.1:0`
2. Print one readiness JSON line on stdout
3. Log only to stderr
4. Validate `x-nyanyabot-token`
5. Use `HostClient` for OneBot and cross-plugin calls (never forge `caller_plugin_id`)

Details: [nyanyabot-proto README](https://github.com/xiaocaoooo/nyanyabot-proto/blob/main/README.md).

## Related

- [NyaNyaBot](https://github.com/xiaocaoooo/NyaNyaBot) — host
- [nyanyabot-proto](https://github.com/xiaocaoooo/nyanyabot-proto) — gRPC + runtime
- Chinese readme: [README_zh.md](./README_zh.md)

## License

MIT (workspace package metadata).
