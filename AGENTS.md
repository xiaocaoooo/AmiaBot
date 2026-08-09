# AmiaBot Agent Notes

## Stack
- Rust 2024 workspace of NyaNyaBot plugins
- Shared helpers: `crates/plugin-common` (depends only on `nyanyabot-proto`)
- HTTP: `reqwest` + rustls; WS: `tokio-tungstenite`; PJSK account DB: `sqlx`

## Plugins (15)
screenshot, blobserver, bilibili, pixiv, gallery, onebot-websocket-client, query, zeabur-status,
pjsk-account/bind/card/event/song/profile/b30

## Build
```bash
cargo xtask stage   # release binaries -> plugins/
```

## Notes
- Keep Descriptor/plugin_id/commands/exports stable
- No AmiaBot business logic inside `nyanyabot-proto`
