use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, event_self_id, join_url, redact_secrets, screenshot_and_upload,
    send_forward, send_image, send_text, upload_remote_blob,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::warn;
use tracing_subscriber::EnvFilter;

const PIXIV_PATTERN: &str =
    r"(?i)\b(?:https?://)?(?:www\.)?pixiv\.net/(?:[a-z]{2}/)?artworks/(\d+)(?:[?#/][^\s]*)?";

#[derive(Clone, Default)]
struct Cfg {
    amiabot_pages: String,
    amiabot_pages_download_base: String,
}

impl Cfg {
    fn from_value(v: &Value) -> Self {
        Self {
            amiabot_pages: plugin_common::normalize_http_base(
                v.get("amiabot_pages")
                    .and_then(|x| x.as_str())
                    .unwrap_or(""),
            ),
            amiabot_pages_download_base: plugin_common::normalize_http_base(
                v.get("amiabot_pages_download_base")
                    .and_then(|x| x.as_str())
                    .unwrap_or(""),
            ),
        }
    }
}

struct Plug {
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Cfg>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot Pixiv".into(),
        plugin_id: "external.amiabot-pixiv".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "Pixiv 作品卡片与原图".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Pixiv 插件配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "amiabot_pages":{"type":"string"},
                    "amiabot_pages_download_base":{"type":"string"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({"amiabot_pages":"","amiabot_pages_download_base":""})),
        }),
        commands: vec![CommandListener {
            name: "pixiv-artwork".into(),
            id: "cmd.pixiv-artwork".into(),
            description: "解析 pixiv artwork 链接".into(),
            pattern: PIXIV_PATTERN.into(),
            match_raw: false,
            handler: "Handle".into(),
        }],
        ..Default::default()
    }
}

fn extract_pid(content: &str, match_data: &Option<CommandMatch>) -> String {
    if let Some(m) = match_data
        && let Some(g) = m.groups.first()
    {
        let g = g.trim();
        if !g.is_empty() {
            return g.to_string();
        }
    }
    let re = Regex::new(PIXIV_PATTERN).unwrap();
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default()
}

#[derive(Debug, Deserialize, Default)]
struct MediaManifest {
    #[serde(default)]
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    items: Vec<MediaItem>,
}

#[derive(Debug, Deserialize, Default)]
struct MediaItem {
    #[serde(default)]
    index: i64,
    #[serde(default)]
    path: String,
}

fn build_pages_asset_url(pages_host: &str, asset_path: &str) -> String {
    let base = plugin_common::normalize_http_base(pages_host);
    let asset_path = asset_path.trim();
    if base.is_empty() || asset_path.is_empty() {
        return String::new();
    }
    if asset_path.starts_with("http://") || asset_path.starts_with("https://") {
        return asset_path.to_string();
    }
    join_url(&base, asset_path)
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        *self.cfg.write() = Cfg::from_value(&config);
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
        if listener_id != "cmd.pixiv-artwork" {
            return Ok(HandleResult {});
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let content = event_content(&event_raw);
        let pid = extract_pid(&content, &match_data);
        if pid.is_empty() {
            return Ok(HandleResult {});
        }
        let cfg = self.cfg.read().clone();
        if cfg.amiabot_pages.is_empty() {
            let _ = send_text(&mut host, &event_raw, "❌ 服务未配置", trace_id).await;
            return Ok(HandleResult {});
        }
        let mut q = BTreeMap::new();
        q.insert("pid".into(), pid.clone());
        let page = build_pages_url(&cfg.amiabot_pages, "/pixiv/illust/info", &q);
        match screenshot_and_upload(
            &mut host,
            &page,
            &format!(
                "pixiv-artwork-{}-{}",
                pid,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            ),
            json!({}),
        )
        .await
        {
            Ok(url) => {
                let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
            }
            Err(err) => {
                let _ = send_text(
                    &mut host,
                    &event_raw,
                    &format!("❌ 截图失败：{}", redact_secrets(&err.to_string())),
                    trace_id,
                )
                .await;
                return Ok(HandleResult {});
            }
        }

        let download_base = if cfg.amiabot_pages_download_base.is_empty() {
            cfg.amiabot_pages.clone()
        } else {
            cfg.amiabot_pages_download_base.clone()
        };
        let manifest_url = build_pages_url(&download_base, "/pixiv/illust/media", &q);
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(_) => return Ok(HandleResult {}),
        };
        let manifest = match client.get(&manifest_url).send().await {
            Ok(resp) if resp.status().is_success() => resp.json::<MediaManifest>().await.ok(),
            Ok(resp) => {
                let _ = send_text(
                    &mut host,
                    &event_raw,
                    &format!("❌ 获取原图失败：HTTP {}", resp.status()),
                    trace_id,
                )
                .await;
                None
            }
            Err(err) => {
                let _ = send_text(
                    &mut host,
                    &event_raw,
                    &format!("❌ 获取原图失败：{}", redact_secrets(&err.to_string())),
                    trace_id,
                )
                .await;
                None
            }
        };
        let Some(manifest) = manifest else {
            return Ok(HandleResult {});
        };
        if manifest.items.is_empty() {
            let _ = send_text(&mut host, &event_raw, "⚠️ 未获取到可发送的原图", trace_id).await;
            return Ok(HandleResult {});
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut media_urls = Vec::new();
        for item in &manifest.items {
            let media_url = build_pages_asset_url(&download_base, &item.path);
            if media_url.is_empty() {
                continue;
            }
            let blob_id = format!("pixiv-media-{}-{}-{}", pid, item.index, ts);
            match upload_remote_blob(&mut host, &media_url, &blob_id, "image").await {
                Ok(u) => media_urls.push(u),
                Err(err) => {
                    warn!(error=%err, "pixiv media upload failed");
                    media_urls.push(media_url);
                }
            }
        }
        if media_urls.is_empty() {
            let _ = send_text(&mut host, &event_raw, "⚠️ 未获取到可发送的原图", trace_id).await;
            return Ok(HandleResult {});
        }
        if media_urls.len() == 1 {
            let _ = send_image(&mut host, &event_raw, &media_urls[0], trace_id).await;
            return Ok(HandleResult {});
        }
        let self_id = event_self_id(&event_raw);
        let total = media_urls.len();
        let nodes: Vec<Value> = media_urls
            .into_iter()
            .enumerate()
            .map(|(i, url)| {
                json!({
                    "type":"node",
                    "data":{
                        "user_id": self_id,
                        "nickname":"AmiaBot Pixiv",
                        "content":[
                            {"type":"text","data":{"text": format!("P{} / {}", i+1, total)}},
                            {"type":"image","data":{"file": url}}
                        ]
                    }
                })
            })
            .collect();
        if let Err(err) = send_forward(&mut host, &event_raw, Value::Array(nodes), trace_id).await {
            let _ = send_text(
                &mut host,
                &event_raw,
                &format!("❌ 发送原图失败：{}", redact_secrets(&err.to_string())),
                trace_id,
            )
            .await;
        }
        let _ = manifest.kind;
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
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();
    let host = Arc::new(AsyncRwLock::new(None));
    run_plugin_with_host(
        Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(Cfg::default()),
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
        let desc = plugin_descriptor();
        let got = serde_json::to_value(&desc).unwrap();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots/descriptor.json");
        if std::env::var("UPDATE_SNAPSHOTS").ok().as_deref() == Some("1") {
            std::fs::create_dir_all(path.parent().unwrap()).ok();
            std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap()).unwrap();
            return;
        }
        let raw = std::fs::read_to_string(&path).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(got, expected, "run UPDATE_SNAPSHOTS=1");
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    #[test]
    fn extract_artwork_id() {
        let pid = extract_pid("https://www.pixiv.net/artworks/12345678 nice", &None);
        assert_eq!(pid, "12345678");
    }
}
