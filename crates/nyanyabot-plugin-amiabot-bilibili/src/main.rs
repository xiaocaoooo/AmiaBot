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
    build_pages_url, event_content, first_match_group, normalize_http_base, screenshot_and_upload,
    send_image, send_video, upload_remote_blob,
};
use regex::Regex;
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
    let short = short.trim();
    if short.is_empty() {
        return Err("short is empty".into());
    }
    // Follow redirects like Go default http.Client.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("https://b23.tv/{short}");
    let resp = client
        .get(&url)
        .header("User-Agent", "nyanyabot-plugin-amiabot-bilibili/0.1")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let final_url = resp.url().to_string();
    // Discard body like Go (up to 64KiB conceptually).
    let _ = resp.bytes().await;
    if final_url.trim().is_empty() {
        return Err("empty final url".into());
    }
    let aid = extract_aid(&final_url);
    let bvid = extract_bvid(&final_url);
    if aid.is_empty() && bvid.is_empty() {
        return Err(format!(
            "cannot extract aid/bvid from redirect url: {final_url}"
        ));
    }
    Ok((aid, bvid))
}

fn extract_aid(s: &str) -> String {
    let re = Regex::new(r"(?i)\bav(\d+)\b").unwrap();
    re.captures(s)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default()
}

fn extract_bvid(s: &str) -> String {
    let re = Regex::new(r"(?i)\b(bv1[0-9a-zA-Z]+)\b").unwrap();
    re.captures(s)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_uppercase()))
        .unwrap_or_default()
}

fn chrono_like_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn build_downloader_url(downloader_server: &str, id: &str) -> String {
    let base = normalize_http_base(downloader_server)
        .trim_end_matches('/')
        .to_string();
    if base.is_empty() || id.is_empty() {
        return String::new();
    }
    let mut enc = String::new();
    for b in id.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                enc.push(b as char)
            }
            _ => enc.push_str(&format!("%{b:02X}")),
        }
    }
    format!("{base}/bilibili/download/{enc}")
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

        // Screenshot via pages + screenshot/blob plugins (Go: silent if pages empty).
        let mut screenshot_url = String::new();
        if !cfg.amiabot_pages.is_empty() {
            let mut q = BTreeMap::new();
            if !aid.is_empty() {
                q.insert("aid".into(), aid.clone());
            }
            if !bvid.is_empty() {
                q.insert("bvid".into(), bvid.clone());
            }
            let page = build_pages_url(&cfg.amiabot_pages, "/bilibili/video", &q);
            let id_for_blob = if !aid.is_empty() {
                aid.clone()
            } else {
                bvid.clone()
            };
            match screenshot_and_upload(
                &mut host,
                &page,
                &format!("{id_for_blob}-image-{}", chrono_like_unix()),
                json!({}),
            )
            .await
            {
                Ok(url) => screenshot_url = url,
                Err(err) => warn!(error=%err, "bilibili screenshot failed"),
            }
        }

        // Downloader URL: {server}/bilibili/download/{id} (Go buildDownloaderURL).
        let id = if !aid.is_empty() {
            aid.clone()
        } else {
            bvid.clone()
        };
        let mut video_url = String::new();
        if !cfg.bilibili_downloader_server.is_empty() && !id.is_empty() {
            video_url = build_downloader_url(&cfg.bilibili_downloader_server, &id);
        }

        if screenshot_url.is_empty() && video_url.is_empty() {
            let _ = first_match_group(&match_data);
            return Ok(HandleResult {});
        }

        if !screenshot_url.is_empty() {
            let _ = send_image(&mut host, &event_raw, &screenshot_url, trace_id).await;
        }
        if !video_url.is_empty() {
            let mut final_video = video_url;
            match upload_remote_blob(&mut host, &final_video, &format!("{id}-video"), "video").await
            {
                Ok(url) => final_video = url,
                Err(err) => warn!(error=%err, "bilibili video blob upload failed; send raw url"),
            }
            if let Err(err) = send_video(&mut host, &event_raw, &final_video, trace_id).await {
                warn!(error=%err, "bilibili send_video failed");
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
    fn parse_bv() {
        let (aid, bvid, short) = parse_ids("看看 bv1xx411c7mD 这个", &None);
        assert!(aid.is_empty());
        assert!(bvid.to_lowercase().starts_with("bv1"));
        assert!(short.is_empty());
    }

    #[test]
    fn extract_from_redirect_url() {
        let url = "https://www.bilibili.com/video/BV1xx411c7mD?spm=1";
        assert_eq!(extract_aid(url), "");
        assert_eq!(extract_bvid(url), "BV1XX411C7MD");
        assert_eq!(
            extract_aid("https://www.bilibili.com/video/av170001"),
            "170001"
        );
    }

    #[test]
    fn downloader_url_shape() {
        assert_eq!(
            build_downloader_url("http://dl.example.com", "BV1xx"),
            "http://dl.example.com/bilibili/download/BV1xx"
        );
    }
}
