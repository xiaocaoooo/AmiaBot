use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, redact_secrets, screenshot_and_upload, send_image, send_text,
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
        name: "Amiabot PJSK Card".into(),
        plugin_id: "external.amiabot-pjsk-card".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "PJSK 卡面查询插件，发送截图".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Amiabot PJSK Card plugin config".into()),
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
            name: "pjsk-card".into(),
            id: "cmd.pjsk-card".into(),
            description: "PJSK 卡面查询（如 card1, jpcard1, cn查卡5）".into(),
            pattern: r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:card|查卡)(?P<id>[0-9]+)$".into(),
            match_raw: true,
            handler: "Handle".into(),
        }],
        ..Default::default()
    }
}

fn valid_server(s: &str) -> bool {
    matches!(s, "cn" | "jp" | "tw" | "en" | "kr")
}

/// Parse server/id from host-provided capture groups (positional).
fn parse_args(groups: &[String], default_server: &str) -> (String, String) {
    let mut server = String::new();
    let mut id = String::new();
    match groups.len() {
        0 => {}
        1 => {
            let g0 = groups[0].trim();
            if valid_server(&g0.to_lowercase()) {
                server = g0.to_lowercase();
            } else if g0.chars().all(|c| c.is_ascii_digit()) {
                id = g0.to_string();
            }
        }
        _ => {
            let g0 = groups[0].trim().to_lowercase();
            if valid_server(&g0) {
                server = g0;
            }
            id = groups[1].trim().to_string();
            // When server absent, first capture may be empty and second is id;
            // host still pushes empty string for optional group.
            if id.is_empty() && groups[0].chars().all(|c| c.is_ascii_digit()) {
                id = groups[0].trim().to_string();
            }
        }
    }
    if server.is_empty() {
        let ds = default_server.trim().to_lowercase();
        server = if valid_server(&ds) { ds } else { "jp".into() };
    }
    (server, id)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
        if listener_id != "cmd.pjsk-card" {
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
            .unwrap_or("jp");
        let _content = match_data
            .as_ref()
            .map(|m| m.full.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| event_content(&event_raw));
        let groups = match_data.map(|m| m.groups).unwrap_or_default();
        let (server, id) = parse_args(&groups, default_server);
        if id.is_empty() {
            let _ = send_text(
                &mut host,
                &event_raw,
                "❌ 参数不完整，请使用格式: card+编号",
                trace_id,
            )
            .await;
            return Ok(HandleResult {});
        }
        if pages.is_empty() {
            let _ = send_text(&mut host, &event_raw, "❌ 服务未配置", trace_id).await;
            return Ok(HandleResult {});
        }
        let mut q = BTreeMap::new();
        q.insert("server".into(), server.clone());
        q.insert("id".into(), id.clone());
        let page_url = build_pages_url(&pages, "/pjsk/card", &q);
        let blob_id = format!("pjsk-card-{server}-{id}-{}", now_unix());
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
    use super::parse_args;

    #[test]
    fn parse_with_server() {
        let (s, id) = parse_args(&["cn".into(), "5".into()], "jp");
        assert_eq!((s.as_str(), id.as_str()), ("cn", "5"));
    }

    #[test]
    fn parse_id_only_uses_default() {
        let (s, id) = parse_args(&["".into(), "12".into()], "jp");
        assert_eq!((s.as_str(), id.as_str()), ("jp", "12"));
    }

    #[test]
    fn parse_single_digit_group() {
        let (s, id) = parse_args(&["99".into()], "tw");
        assert_eq!((s.as_str(), id.as_str()), ("tw", "99"));
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
