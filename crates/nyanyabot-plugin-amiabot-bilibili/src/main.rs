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
    mark_command_effective,
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
const DEFAULT_QUALITY: i64 = 80;

#[derive(Clone)]
struct Cfg {
    amiabot_pages: String,
    bilibili_downloader_server: String,
    /// Upstream qn; `None` means omit query (server default). Values `< 1` are treated as unset.
    quality: Option<i64>,
}

impl Default for Cfg {
    fn default() -> Self {
        Self {
            amiabot_pages: String::new(),
            bilibili_downloader_server: String::new(),
            quality: Some(DEFAULT_QUALITY),
        }
    }
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
            quality: parse_quality(v.get("quality")),
        }
    }
}

fn parse_quality(v: Option<&Value>) -> Option<i64> {
    let Some(v) = v else {
        return Some(DEFAULT_QUALITY);
    };
    if v.is_null() {
        return Some(DEFAULT_QUALITY);
    }
    let q = if let Some(n) = v.as_i64() {
        n
    } else if let Some(n) = v.as_u64() {
        i64::try_from(n).unwrap_or(-1)
    } else if let Some(s) = v.as_str() {
        let s = s.trim();
        if s.is_empty() {
            return Some(DEFAULT_QUALITY);
        }
        s.parse::<i64>().unwrap_or(-1)
    } else if let Some(n) = v.as_f64() {
        n as i64
    } else {
        return Some(DEFAULT_QUALITY);
    };
    if q < 1 {
        None
    } else {
        Some(q)
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
                    "bilibili_downloader_server":{"type":"string"},
                    "quality":{"type":"integer","minimum":1,"description":"下载画质 qn，默认 80（1080P）"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({
                "amiabot_pages":"",
                "bilibili_downloader_server":"",
                "quality": DEFAULT_QUALITY
            })),
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

/// Prefer bvid when both are present.
fn pick_video_id(aid: &str, bvid: &str) -> String {
    let bvid = bvid.trim();
    if !bvid.is_empty() {
        return bvid.to_string();
    }
    aid.trim().to_string()
}

fn extract_part(s: &str) -> Option<u32> {
    let re = Regex::new(r"(?i)[?&]p=(\d+)").unwrap();
    re.captures(s)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
        .and_then(|raw| raw.parse::<u32>().ok())
        .filter(|&p| p >= 1)
}

async fn resolve_b23(short: &str) -> Result<(String, String, Option<u32>), String> {
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
    let p = extract_part(&final_url);
    Ok((aid, bvid, p))
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

fn percent_encode_path_segment(id: &str) -> String {
    let mut enc = String::new();
    for b in id.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                enc.push(b as char)
            }
            _ => enc.push_str(&format!("%{b:02X}")),
        }
    }
    enc
}

/// New API: `{base}/api/v1/videos/{id}/download?p=&quality=`
fn build_downloader_url(
    downloader_server: &str,
    id: &str,
    p: Option<u32>,
    quality: Option<i64>,
) -> String {
    let base = normalize_http_base(downloader_server)
        .trim_end_matches('/')
        .to_string();
    if base.is_empty() || id.is_empty() {
        return String::new();
    }
    let enc = percent_encode_path_segment(id);
    let path = format!("/api/v1/videos/{enc}/download");
    let mut q = BTreeMap::new();
    if let Some(p) = p.filter(|&p| p >= 1) {
        q.insert("p".into(), p.to_string());
    }
    if let Some(quality) = quality.filter(|&q| q >= 1) {
        q.insert("quality".into(), quality.to_string());
    }
    build_pages_url(&base, &path, &q)
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
            return Ok(HandleResult::ignored());
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult::ignored());
        };
        let content = event_content(&event_raw);
        let (mut aid, mut bvid, short) = parse_ids(&content, &match_data);
        let mut part = extract_part(&content);
        if !short.is_empty() && aid.is_empty() && bvid.is_empty() {
            match resolve_b23(&short).await {
                Ok((a, b, p)) => {
                    aid = a;
                    bvid = b;
                    if part.is_none() {
                        part = p;
                    }
                }
                Err(err) => {
                    warn!(error=%err, "b23 resolve failed");
                    return Ok(HandleResult::ignored());
                }
            }
        }
        if aid.is_empty() && bvid.is_empty() {
            return Ok(HandleResult::ignored());
        }
        let cfg = self.cfg.read().clone();
        let id = pick_video_id(&aid, &bvid);

        // Screenshot via pages + screenshot/blob plugins (silent if pages empty).
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
            let id_for_blob = if id.is_empty() {
                pick_video_id(&aid, &bvid)
            } else {
                id.clone()
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

        // Downloader URL: {server}/api/v1/videos/{id}/download
        let mut video_url = String::new();
        if !cfg.bilibili_downloader_server.is_empty() && !id.is_empty() {
            video_url =
                build_downloader_url(&cfg.bilibili_downloader_server, &id, part, cfg.quality);
        }

        if screenshot_url.is_empty() && video_url.is_empty() {
            let _ = first_match_group(&match_data);
            return Ok(HandleResult::ignored());
        }
        mark_command_effective(&mut host, trace_id).await;

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
    fn pick_prefers_bvid() {
        assert_eq!(pick_video_id("170001", "BV1xx411c7mD"), "BV1xx411c7mD");
        assert_eq!(pick_video_id("170001", ""), "170001");
        assert_eq!(pick_video_id("", "BV1xx"), "BV1xx");
    }

    #[test]
    fn extract_part_from_query() {
        assert_eq!(
            extract_part("https://www.bilibili.com/video/BV1xx411c7mD?p=2&spm=1"),
            Some(2)
        );
        assert_eq!(
            extract_part("看看 BV1xx411c7mD 这个 &p=3 分P"),
            Some(3)
        );
        assert_eq!(extract_part("no part here"), None);
        assert_eq!(extract_part("?p=0"), None);
    }

    #[test]
    fn downloader_url_shape() {
        assert_eq!(
            build_downloader_url("http://dl.example.com", "BV1xx", None, None),
            "http://dl.example.com/api/v1/videos/BV1xx/download"
        );
        assert_eq!(
            build_downloader_url("http://dl.example.com", "BV1xx", Some(2), Some(64)),
            "http://dl.example.com/api/v1/videos/BV1xx/download?p=2&quality=64"
        );
        assert_eq!(
            build_downloader_url("http://dl.example.com", "170001", None, Some(80)),
            "http://dl.example.com/api/v1/videos/170001/download?quality=80"
        );
    }

    #[test]
    fn parse_quality_defaults_and_invalid() {
        assert_eq!(parse_quality(None), Some(80));
        assert_eq!(parse_quality(Some(&json!(64))), Some(64));
        assert_eq!(parse_quality(Some(&json!("112"))), Some(112));
        assert_eq!(parse_quality(Some(&json!(0))), None);
        assert_eq!(parse_quality(Some(&json!(-1))), None);
    }
}
