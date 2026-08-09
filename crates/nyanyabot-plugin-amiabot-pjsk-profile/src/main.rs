use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, event_user_id, redact_secrets, screenshot_and_upload,
    send_image, send_text,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

struct Plug {
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot PJSK Profile".into(),
        plugin_id: "external.amiabot-pjsk-profile".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "PJSK 个人信息查询插件，发送 profile 截图".into(),
        dependencies: vec![
            "external.amiabot-pjsk-account".into(),
            "external.screenshot".into(),
            "external.blobserver".into(),
        ],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Amiabot PJSK Profile plugin config".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址；为空则无法生成截图 URL"},
                    "default_server":{"type":"string","description":"默认服务器 (jp/cn/en/tw/kr)，不填时为 jp"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({"amiabot_pages":"","default_server":"jp"})),
        }),
        commands: vec![CommandListener {
            name: "pjsk-profile".into(),
            id: "cmd.pjsk-profile".into(),
            description: "查看 PJSK 个人信息（如 profile, 个人信息, cn个人信息）".into(),
            pattern: r"^(?:(?P<server>cn|jp|tw|en|kr))?(?:个人信息|profile)$".into(),
            match_raw: true,
            handler: "Handle".into(),
        }],
        ..Default::default()
    }
}

fn valid_server(s: &str) -> bool {
    matches!(s, "cn" | "jp" | "tw" | "en" | "kr")
}

fn parse_server(groups: &[String]) -> String {
    groups
        .first()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| valid_server(s))
        .unwrap_or_default()
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

struct BoundAccount {
    server: String,
    game_id: String,
}

async fn resolve_bound_account(
    host: &mut HostClient,
    qq: i64,
    mut server: String,
    default_server: &str,
) -> Result<Result<BoundAccount, String>, StructuredError> {
    let list = host
        .call_dependency(
            "external.amiabot-pjsk-account",
            "account.list_by_qq",
            &json!({"qq_id": qq, "enabled_only": true}),
        )
        .await?;
    let ok = list
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let accounts = list
        .get("accounts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !ok {
        return Ok(Err("❌ 查询失败".into()));
    }
    if accounts.is_empty() {
        return Ok(Err(
            "你还没有绑定任何账号。\n请发送 绑定+你的游戏ID 进行绑定，如 jp绑定12345".into(),
        ));
    }
    if server.is_empty()
        && let Ok(pref) = host
            .call_dependency(
                "external.amiabot-pjsk-account",
                "account.get_preferred_server",
                &json!({"qq_id": qq}),
            )
            .await
    {
        let cur = pref
            .get("server")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if valid_server(&cur) {
            server = cur;
        }
    }
    if server.is_empty() {
        let ds = default_server.trim().to_lowercase();
        server = if valid_server(&ds) { ds } else { "jp".into() };
    }
    let mut target = None;
    for a in &accounts {
        let gs = a
            .get("game_server")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let enabled = a.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        if gs == server && enabled {
            let gid = a
                .get("game_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !gid.is_empty() {
                target = Some(gid);
                break;
            }
        }
    }
    match target {
        Some(game_id) => Ok(Ok(BoundAccount { server, game_id })),
        None => {
            let lines: Vec<String> = accounts
                .iter()
                .filter_map(|a| {
                    a.get("game_server")
                        .and_then(|v| v.as_str())
                        .map(|s| format!("[{}]", s.to_uppercase()))
                })
                .collect();
            Ok(Err(format!(
                "你尚未绑定 [{}] 服务器的账号。\n你已绑定的服务器: {}\n可以通过 服务器前缀+个人信息 切换，如 cn个人信息",
                server.to_uppercase(),
                lines.join(", ")
            )))
        }
    }
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        *self.cfg.write() = config;
        Ok(())
    }
    async fn invoke(&self, _: &str, _: Value, _: &str) -> Result<Value, StructuredError> {
        Err(StructuredError::not_found("method is not exported"))
    }
    async fn handle(
        &self,
        listener_id: &str,
        event_raw: Value,
        match_data: Option<CommandMatch>,
        trace_id: &str,
    ) -> Result<HandleResult, StructuredError> {
        if listener_id != "cmd.pjsk-profile" {
            return Ok(HandleResult {});
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let cfg = self.cfg.read().clone();
        let pages = cfg
            .get("amiabot_pages")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let default_server = cfg
            .get("default_server")
            .and_then(|v| v.as_str())
            .unwrap_or("jp")
            .to_string();
        let _content = match_data
            .as_ref()
            .map(|m| m.full.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| event_content(&event_raw));
        let groups = match_data.map(|m| m.groups).unwrap_or_default();
        let server = parse_server(&groups);
        let qq = event_user_id(&event_raw);
        match resolve_bound_account(&mut host, qq, server, &default_server).await {
            Ok(Ok(acc)) => {
                if pages.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "❌ 服务未配置", trace_id).await;
                    return Ok(HandleResult {});
                }
                let mut q = BTreeMap::new();
                q.insert("server".into(), acc.server.clone());
                q.insert("id".into(), acc.game_id.clone());
                let page_url = build_pages_url(&pages, "/pjsk/profile", &q);
                let blob_id = format!("pjsk-profile-{}-{}-{}", acc.server, acc.game_id, now_unix());
                match screenshot_and_upload(&mut host, &page_url, &blob_id, json!({})).await {
                    Ok(url) => {
                        let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 截图失败: {}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            Ok(Err(msg)) => {
                let _ = send_text(&mut host, &event_raw, &msg, trace_id).await;
            }
            Err(err) => {
                let _ = send_text(
                    &mut host,
                    &event_raw,
                    &format!("❌ 查询绑定失败: {}", redact_secrets(&err.to_string())),
                    trace_id,
                )
                .await;
            }
        }
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        Ok("OK".into())
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();
    let host = Arc::new(AsyncRwLock::new(None));
    run_plugin_with_host(
        Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(json!({})),
        }),
        host,
    )
    .await
}

#[cfg(test)]
mod unit_tests {
    use super::parse_server;

    #[test]
    fn parse_server_from_groups() {
        assert_eq!(parse_server(&["cn".into()]), "cn");
        assert_eq!(parse_server(&["".into()]), "");
        assert_eq!(parse_server(&[]), "");
    }
}

#[cfg(test)]
mod descriptor_snapshot_tests {
    use super::plugin_descriptor;

    #[test]
    fn descriptor_matches_snapshot() {
        let mut d = plugin_descriptor();
        nyanyabot_proto::ensure_descriptor_arrays(&mut d);
        let actual = serde_json::to_string_pretty(&d).expect("serialize descriptor");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots/descriptor.json");
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("{actual}\n")).unwrap();
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing descriptor snapshot at {}: {e}", path.display()));
        assert_eq!(
            actual.trim(),
            expected.trim(),
            "descriptor snapshot mismatch; run with UPDATE_SNAPSHOTS=1"
        );
    }
}
