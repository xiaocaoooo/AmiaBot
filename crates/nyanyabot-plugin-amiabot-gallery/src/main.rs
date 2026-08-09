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
        name: "Amiabot Gallery".into(),
        plugin_id: "external.amiabot-gallery".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "图库管理".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(json!({"amiabot_pages":"","gallery_api":""})),
        }),
        commands: vec![
            CommandListener {
                name: "create-tag".into(),
                id: "cmd.gallery-create-tag".into(),
                description: "创建标签".into(),
                pattern: r"(?i)^创建标签\s+(\S+)$".into(),
                match_raw: true,
                handler: "Handle".into(),
            },
            CommandListener {
                name: "upload".into(),
                id: "cmd.gallery-upload".into(),
                description: "上传图片".into(),
                pattern: r"(?i)^上传(?:\s+(\S+))?$".into(),
                match_raw: true,
                handler: "Handle".into(),
            },
            CommandListener {
                name: "view".into(),
                id: "cmd.gallery-view".into(),
                description: "查看图库".into(),
                pattern: r"(?i)^图库(?:\s+(\S+))?$".into(),
                match_raw: true,
                handler: "Handle".into(),
            },
        ],
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
        let tag = match_data
            .and_then(|m| m.groups.first().cloned())
            .unwrap_or_default();
        let path = match listener_id {
            "cmd.gallery-create-tag" => format!("/gallery/create?tag={tag}"),
            "cmd.gallery-upload" => format!("/gallery/upload?tag={tag}"),
            "cmd.gallery-view" => format!("/gallery/view?tag={tag}"),
            _ => return Ok(HandleResult {}),
        };
        if pages.is_empty() {
            let _ =
                plugin_common::send_text(&mut host, &event_raw, "amiabot_pages 未配置", trace_id)
                    .await;
            return Ok(HandleResult {});
        }
        let page_url = plugin_common::join_url(&pages, &path);
        match plugin_common::screenshot_and_upload(
            &mut host,
            &page_url,
            &format!("gallery-{listener_id}"),
            json!({}),
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
