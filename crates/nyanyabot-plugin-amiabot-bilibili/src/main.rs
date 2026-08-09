use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

struct Plug {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot Bilibili".into(),
        plugin_id: "external.amiabot-bilibili".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "Bilibili 链接解析与截图".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({"type":"object","properties":{"amiabot_pages":{"type":"string"}}})),
            default: Some(json!({"amiabot_pages":""})),
        }),
        commands: vec![CommandListener {
            name: "bilibili".into(),
            id: "cmd.bilibili".into(),
            description: "解析 bilibili 链接".into(),
            pattern: r"(?i)(?:https?://)?(?:www\.)?(?:bilibili\.com|b23\.tv)\S+".into(),
            match_raw: true,
            handler: "Handle".into(),
        }],
        ..Default::default()
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
        if listener_id != "cmd.bilibili" {
            return Ok(HandleResult {});
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let pages = self
            .cfg
            .read()
            .get("amiabot_pages")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let link = match_data.map(|m| m.full).unwrap_or_default();
        if pages.is_empty() {
            let _ =
                plugin_common::send_text(&mut host, &event_raw, "amiabot_pages 未配置", trace_id)
                    .await;
            return Ok(HandleResult {});
        }
        let page_url = format!(
            "{}?url={}",
            plugin_common::join_url(&pages, "/bilibili"),
            urlencoding_basic(&link)
        );
        match plugin_common::screenshot_and_upload(&mut host, &page_url, "bilibili", json!({}))
            .await
        {
            Ok(url) => {
                let _ = plugin_common::send_image(&mut host, &event_raw, &url, trace_id).await;
            }
            Err(err) => {
                let _ = plugin_common::send_text(
                    &mut host,
                    &event_raw,
                    &format!("失败: {err}"),
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
fn urlencoding_basic(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
