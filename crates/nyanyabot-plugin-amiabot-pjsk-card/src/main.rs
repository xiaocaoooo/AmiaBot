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
        name: "Amiabot PJSK Card".into(),
        plugin_id: "external.amiabot-pjsk-card".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "PJSK 卡面查询".into(),
        dependencies: vec![
            "external.screenshot".to_string(),
            "external.blobserver".to_string(),
        ],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(json!({"amiabot_pages": "", "default_server": "jp"})),
        }),
        commands: vec![CommandListener {
            name: "cmd.pjsk-card".into(),
            id: "cmd.pjsk-card".into(),
            description: "PJSK 卡面查询".into(),
            pattern: r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:card|查卡)(?P<id>[0-9]+)$".into(),
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
            .to_string();
        if pages.is_empty() {
            let _ =
                plugin_common::send_text(&mut host, &event_raw, "amiabot_pages 未配置", trace_id)
                    .await;
            return Ok(HandleResult {});
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
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        Ok("OK".into())
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        Ok(())
    }
}

fn build_page_path(_full: &str, groups: &[String], default_server: &str) -> (String, String) {
    let server = groups
        .first()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty() && matches!(*s, "cn" | "jp" | "tw" | "en" | "kr"))
        .unwrap_or(default_server);
    let id = if groups.len() >= 2 {
        groups[1].as_str()
    } else if groups.len() == 1 && groups[0].chars().all(|c| c.is_ascii_digit()) {
        groups[0].as_str()
    } else {
        ""
    };
    (
        format!("/pjsk/card/{server}/{id}"),
        format!("pjsk-card-{server}-{id}"),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
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
