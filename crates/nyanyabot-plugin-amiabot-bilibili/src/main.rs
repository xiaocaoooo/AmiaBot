use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, first_match_group, redact_secrets, screenshot_and_upload,
    send_image, send_text, upload_remote_blob,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::warn;
use tracing_subscriber::EnvFilter;

const BILI_PATTERN: &str =
    r"(?i)\b(?:av(\d+)|(bv1[0-9a-zA-Z]+)|(?:(?:https?://)?b23\.tv/([a-z0-9]+)))\b";

#[derive(Clone, Default)]
struct Cfg {
    amiabot_pages: String,
    bilibili_downloader_server: String,
}

impl Cfg {
    fn from_value(v: &Value) -> Self {
        Self {
            amiabot_pages: plugin_common::normalize_http_base(
                v.get("amiabot_pages")
                    .and_then(|x| x.as_str())
                    .unwrap_or(""),
            ),
            bilibili_downloader_server: plugin_common::normalize_http_base(
                v.get("bilibili_downloader_server")
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
        name: "Amiabot Bilibili".into(),
        plugin_id: "external.amiabot-bilibili".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "B站视频解析与卡片".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Bilibili 插件配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "amiabot_pages":{"type":"string"},
                    "bilibili_downloader_server":{"type":"string"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({"amiabot_pages":"","bilibili_downloader_server":""})),
        }),
        commands: vec![CommandListener {
            name: "bilibili".into(),
            id: "cmd.bilibili".into(),
            description: "解析 bilibili av/bv/b23 链接".into(),
            pattern: BILI_PATTERN.into(),
            match_raw: false,
            handler: "Handle".into(),
        }],
        ..Default::default()
    }
}

fn parse_ids(content: &str, match_data: &Option<CommandMatch>) -> (String, String, String) {
    if let Some(m) = match_data {
        let aid = m.groups.first().cloned().unwrap_or_default();
        let bvid = m.groups.get(1).cloned().unwrap_or_default();
        let short = m.groups.get(2).cloned().unwrap_or_default();
        if !aid.trim().is_empty() || !bvid.trim().is_empty() || !short.trim().is_empty() {
            return (
                aid.trim().to_string(),
                bvid.trim().to_string(),
                short.trim().to_string(),
            );
        }
    }
    let re = Regex::new(BILI_PATTERN).unwrap();
    if let Some(caps) = re.captures(content) {
        return (
            caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string(),
            caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string(),
            caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string(),
        );
    }
    (String::new(), String::new(), String::new())
}

async fn resolve_b23(short: &str) -> Result<(String, String), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("https://b23.tv/{}", short.trim());
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let target = if loc.is_empty() {
        resp.url().to_string()
    } else {
        loc
    };
    let re = Regex::new(BILI_PATTERN).unwrap();
    if let Some(caps) = re.captures(&target) {
        return Ok((
            caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string(),
            caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string(),
        ));
    }
    // also try BV/av in path
    let re2 = Regex::new(r"(?i)/(av(\d+)| (BV1[0-9A-Za-z]+))").unwrap();
    let _ = re2;
    let re3 = Regex::new(r"(?i)(?:/av(\d+)|/(BV1[0-9A-Za-z]+))").unwrap();
    if let Some(caps) = re3.captures(&target) {
        return Ok((
            caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string(),
            caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string(),
        ));
    }
    Err(format!("unable to resolve b23: {target}"))
}

#[derive(Debug, Deserialize)]
struct DownloaderResp {
    #[serde(default)]
    url: String,
    #[serde(default)]
    download_url: String,
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
        if listener_id != "cmd.bilibili" {
            return Ok(HandleResult {});
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let content = event_content(&event_raw);
        let (mut aid, mut bvid, short) = parse_ids(&content, &match_data);
        if !short.is_empty() && aid.is_empty() && bvid.is_empty() {
            match resolve_b23(&short).await {
                Ok((a, b)) => {
                    aid = a;
                    bvid = b;
                }
                Err(err) => {
                    warn!(error=%err, "b23 resolve failed");
                    return Ok(HandleResult {});
                }
            }
        }
        if aid.is_empty() && bvid.is_empty() {
            return Ok(HandleResult {});
        }
        let cfg = self.cfg.read().clone();
        if !cfg.amiabot_pages.is_empty() {
            let mut q = BTreeMap::new();
            if !aid.is_empty() {
                q.insert("aid".into(), aid.clone());
            }
            if !bvid.is_empty() {
                q.insert("bvid".into(), bvid.clone());
            }
            let page = build_pages_url(&cfg.amiabot_pages, "/bilibili/video", &q);
            match screenshot_and_upload(
                &mut host,
                &page,
                &format!("bilibili-{}{}", aid, bvid),
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
                        &format!("截图失败：{}", redact_secrets(&err.to_string())),
                        trace_id,
                    )
                    .await;
                }
            }
        } else {
            let label = if !bvid.is_empty() {
                format!("BV{bvid}")
            } else {
                format!("av{aid}")
            };
            // bvid already includes BV prefix sometimes
            let label = if bvid.to_lowercase().starts_with("bv") {
                bvid.clone()
            } else if !bvid.is_empty() {
                format!("BV{bvid}")
            } else {
                label
            };
            let _ = send_text(
                &mut host,
                &event_raw,
                &format!("检测到 B 站视频：{label}（amiabot_pages 未配置，仅文本）"),
                trace_id,
            )
            .await;
        }

        // optional downloader
        if !cfg.bilibili_downloader_server.is_empty() {
            let mut q = BTreeMap::new();
            if !aid.is_empty() {
                q.insert("aid".into(), aid.clone());
            }
            if !bvid.is_empty() {
                q.insert("bvid".into(), bvid.clone());
            }
            let api = build_pages_url(&cfg.bilibili_downloader_server, "/api/video", &q);
            if let Ok(client) = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                && let Ok(resp) = client.get(&api).send().await
                && resp.status().is_success()
                && let Ok(body) = resp.json::<DownloaderResp>().await
            {
                let media = if !body.download_url.is_empty() {
                    body.download_url
                } else {
                    body.url
                };
                if !media.is_empty() {
                    match upload_remote_blob(
                        &mut host,
                        &media,
                        &format!("bili-media-{}{}", aid, bvid),
                        "video",
                    )
                    .await
                    {
                        Ok(url) => {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("下载地址：{url}"),
                                trace_id,
                            )
                            .await;
                        }
                        Err(err) => warn!(error=%err, "bilibili media upload failed"),
                    }
                }
            }
        }
        let _ = first_match_group(&match_data);
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
    fn parse_bv() {
        let (aid, bvid, short) = parse_ids("看看 bv1xx411c7mD 这个", &None);
        assert!(aid.is_empty());
        assert!(bvid.to_lowercase().starts_with("bv1"));
        assert!(short.is_empty());
    }
}
