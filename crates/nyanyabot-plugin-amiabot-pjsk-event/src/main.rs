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
        name: "Amiabot PJSK Event".into(),
        plugin_id: "external.amiabot-pjsk-event".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "PJSK 活动查询插件，发送截图".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Amiabot PJSK Event plugin config".into()),
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
            name: "pjsk-event".into(),
            id: "cmd.pjsk-event".into(),
            description: "PJSK 活动查询（如 event, jpevent, cn查活动, enevent50）".into(),
            pattern: r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:event|查活动)(?P<id>[0-9]*)$".into(),
            match_raw: false,
            handler: "Handle".into(),
        }],
        ..Default::default()
    }
}

fn valid_server(s: &str) -> bool {
    matches!(s, "cn" | "jp" | "tw" | "en" | "kr")
}

fn parse_args(groups: &[String], default_server: &str) -> (String, String) {
    let mut server = String::new();
    let mut id = String::new();
    if !groups.is_empty() {
        let g0 = groups[0].trim().to_lowercase();
        if valid_server(&g0) {
            server = g0;
        }
    }
    if groups.len() >= 2 {
        id = groups[1].trim().to_string();
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
        if listener_id != "cmd.pjsk-event" {
            return Ok(HandleResult::ignored());
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult::ignored());
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
        if !valid_server(&server) {
            let _ = send_text(&mut host, &event_raw, "❌ 服务器参数无效", trace_id).await;
            return Ok(HandleResult::ignored());
        }
        if pages.is_empty() {
            let _ = send_text(&mut host, &event_raw, "❌ 服务未配置", trace_id).await;
            return Ok(HandleResult::ignored());
        }
        let mut q = BTreeMap::new();
        q.insert("server".into(), server.clone());
        if !id.is_empty() {
            q.insert("id".into(), id.clone());
        }
        let page_url = build_pages_url(&pages, "/pjsk/event", &q);
        let blob_id = format!(
            "pjsk-event-{server}-{}-{}",
            if id.is_empty() { "latest" } else { &id },
            now_unix()
        );
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
        Ok(HandleResult::handled())
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
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
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
    fn parse_event_with_id() {
        let (s, id) = parse_args(&["en".into(), "50".into()], "jp");
        assert_eq!((s.as_str(), id.as_str()), ("en", "50"));
    }

    #[test]
    fn parse_event_optional_id() {
        let (s, id) = parse_args(&["".into(), "".into()], "cn");
        assert_eq!((s.as_str(), id.as_str()), ("cn", ""));
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
