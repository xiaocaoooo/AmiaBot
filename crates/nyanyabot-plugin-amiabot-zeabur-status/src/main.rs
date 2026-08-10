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
        name: "Amiabot Zeabur Status".into(),
        plugin_id: "external.amiabot-zeabur-status".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "Zeabur 状态截图".into(),
        dependencies: vec![
            "external.screenshot".to_string(),
            "external.blobserver".to_string(),
        ],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(json!({"amiabot_pages": ""})),
        }),
        commands: vec![CommandListener {
            name: "cmd.zeabur-status".into(),
            id: "cmd.zeabur-status".into(),
            description: "Zeabur 状态截图".into(),
            pattern: r"(?i)^(status|状态)$".into(),
            match_raw: false,
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
        if listener_id != "cmd.zeabur-status" {
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
            .to_string();
        if pages.is_empty() {
            // Go: silent no-op when pages host empty
            return Ok(HandleResult::ignored());
        }
        let default_server = cfg
            .get("default_server")
            .and_then(|v| v.as_str())
            .unwrap_or("jp");
        let full = match_data
            .as_ref()
            .map(|m| m.full.clone())
            .unwrap_or_default();
        let groups = match_data.map(|m| m.groups).unwrap_or_default();
        let (path, blob_id) = build_page_path(&full, &groups, default_server);
        let page_url = plugin_common::join_url(&pages, &path);
        match plugin_common::screenshot_and_upload(
            &mut host,
            &page_url,
            &blob_id,
            serde_json::json!({}),
        )
        .await
        {
            Ok(url) => {
                let _ = plugin_common::send_image(&mut host, &event_raw, &url, trace_id).await;
            }
            Err(err) => {
                let _ = plugin_common::send_text(
                    &mut host,
                    &event_raw,
                    &format!("失败: {}", plugin_common::redact_secrets(&err.to_string())),
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

fn build_page_path(_full: &str, _groups: &[String], _default_server: &str) -> (String, String) {
    // Go buildStatusPageURL: {amiabot_pages}/status/zeabur
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    ("/status/zeabur".into(), format!("zeabur-status-{ts}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();
    let host = Arc::new(AsyncRwLock::new(None));
    let plugin = Arc::new(Plug {
        host: host.clone(),
        cfg: RwLock::new(json!({})),
    });
    run_plugin_with_host(plugin, host).await
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
