mod message;

mod client;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use client::{GalleryClient, GalleryConfig, ImageDetail, download_bytes};
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, event_self_id, first_match_group, mark_command_effective,
    redact_secrets, screenshot_and_upload, send_image, send_text, upload_remote_blob,
};
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

const OPEN_PATTERN: &str = r"(?i)^gallery\s+open\s+(?P<gallery_name>\S+)\s*$";
const UPLOAD_PATTERN: &str =
    r"(?i)^(?:gallery\s+upload|上传)\s+(?P<gallery_name>\S+)\s*$";
const VIEW_PATTERN: &str =
    r"(?i)^看\s+(?P<input>\S+?)(?:\s*\*\s*(?P<num>\d+))?\s*$";

const MAX_VIEW_COUNT: u32 = 5;

struct Plug {
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<GalleryConfig>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot Gallery".into(),
        plugin_id: "external.amiabot-gallery".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "画廊插件：创建画廊、上传图片、按 UUID 或画廊名查看".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Amiabot Gallery 插件配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "gallery_server":{"type":"string","description":"Gallery Server 地址，默认 http://127.0.0.1:3000"},
                    "amiabot_pages":{"type":"string","description":"Amiabot Pages 地址（可选，用于重复图对比卡）"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({
                "gallery_server":"http://127.0.0.1:3000",
                "amiabot_pages":""
            })),
        }),
        commands: vec![
            CommandListener {
                name: "gallery-open".into(),
                id: "cmd.gallery-open".into(),
                description: "创建画廊：gallery open <name>".into(),
                pattern: OPEN_PATTERN.into(),
                match_raw: false,
                handler: "HandleOpen".into(),
            },
            CommandListener {
                name: "gallery-upload".into(),
                id: "cmd.gallery-upload".into(),
                description: "上传图片到画廊：gallery upload <name> / 上传 <name>".into(),
                pattern: UPLOAD_PATTERN.into(),
                match_raw: false,
                handler: "HandleUpload".into(),
            },
            CommandListener {
                name: "gallery-view".into(),
                id: "cmd.gallery-view".into(),
                description: "查看图片：看 <uuid|画廊名> 或 看 <画廊名>*N".into(),
                pattern: VIEW_PATTERN.into(),
                match_raw: false,
                handler: "HandleView".into(),
            },
        ],
        ..Default::default()
    }
}

fn match_group(match_data: &Option<CommandMatch>, idx: usize) -> String {
    match_data
        .as_ref()
        .and_then(|m| m.groups.get(idx))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn parse_image_uuid(input: &str) -> Option<Uuid> {
    Uuid::parse_str(input.trim()).ok()
}

/// Only meaningful for gallery-name view. Defaults to 1, clamps to 1..=MAX_VIEW_COUNT.
fn parse_view_count(raw: &str) -> u32 {
    let n = raw.trim().parse::<u32>().unwrap_or(1);
    if n < 1 {
        1
    } else if n > MAX_VIEW_COUNT {
        MAX_VIEW_COUNT
    } else {
        n
    }
}

fn human_bytes(n: i64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let kb = n as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1} MB");
    }
    format!("{:.2} GB", mb / 1024.0)
}

fn build_image_meta_text(image: &ImageDetail) -> String {
    let mut lines = vec![format!("图片 {}", image.id)];
    if let Some(name) = image.name.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        lines.push(format!("名称：{name}"));
    }
    if !image.aliases.is_empty() {
        lines.push(format!("别名：{}", image.aliases.join(", ")));
    }
    if image.width > 0 && image.height > 0 {
        lines.push(format!("尺寸：{}×{}", image.width, image.height));
    }
    if image.size_bytes > 0 {
        lines.push(format!("大小：{}", human_bytes(image.size_bytes)));
    }
    if let Some(ts) = image.created_at {
        lines.push(format!("时间：{}", ts.format("%Y-%m-%d %H:%M:%S UTC")));
    }
    lines.join("\n")
}

#[derive(Debug)]
struct UploadOutcome {
    index: usize,
    uploaded: Option<Uuid>,
    duplicate: Option<Uuid>,
    failure: Option<String>,
}

fn build_upload_summary(source_label: &str, gallery: &str, outcomes: &[UploadOutcome]) -> String {
    let mut lines = vec![format!(
        "上传完成（来源：{source_label}，画廊：{gallery}）"
    )];
    for o in outcomes {
        if let Some(id) = o.uploaded {
            lines.push(format!("第 {} 张：成功 {}", o.index, id));
        } else if let Some(id) = o.duplicate {
            lines.push(format!("第 {} 张：感知重复，已有图片 {}", o.index, id));
        } else if let Some(err) = &o.failure {
            lines.push(format!("第 {} 张：失败 — {err}", o.index));
        }
    }
    lines.join("\n")
}

async fn send_gallery_image(
    host: &mut HostClient,
    event: &Value,
    client: &GalleryClient,
    image: &ImageDetail,
    trace_id: &str,
) -> Result<(), String> {
    let render = client.build_render_url(image.id);
    let blob_id = format!(
        "gallery-image-{}-{}",
        image.id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let onebot_url = upload_remote_blob(host, &render, &blob_id, "image")
        .await
        .map_err(|e| e.to_string())?;
    send_image(host, event, &onebot_url, trace_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Fisher–Yates sample up to n items without replacement.
fn sample_random<T: Clone>(items: &[T], n: usize) -> Vec<T> {
    if items.is_empty() || n == 0 {
        return Vec::new();
    }
    let mut idxs: Vec<usize> = (0..items.len()).collect();
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    if seed == 0 {
        seed = 1;
    }
    for i in (1..idxs.len()).rev() {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        let j = (seed as usize) % (i + 1);
        idxs.swap(i, j);
    }
    let take = n.min(items.len());
    idxs.into_iter()
        .take(take)
        .map(|i| items[i].clone())
        .collect()
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }

    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        *self.cfg.write() = GalleryConfig::from_value(&config);
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
            return Ok(HandleResult::ignored());
        };
        let cfg = self.cfg.read().clone();
        let client = GalleryClient::new(&cfg);
        let _ = event_content(&event_raw);

        match listener_id {
            "cmd.gallery-open" => {
                let name = first_match_group(&match_data);
                if name.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "请输入画廊名称", trace_id).await;
                    return Ok(HandleResult::ignored());
                }
                mark_command_effective(&mut host, trace_id).await;
                match client.create_gallery(&name).await {
                    Ok(g) => {
                        let aliases = if g.aliases.is_empty() {
                            "-".into()
                        } else {
                            g.aliases.join(", ")
                        };
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!(
                                "画廊已创建\n名称：{}\nID：{}\n别名：{aliases}",
                                g.name, g.id
                            ),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::handled());
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 创建画廊失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::handled());
                    }
                }
            }

            "cmd.gallery-upload" => {
                let gallery_name = first_match_group(&match_data);
                if gallery_name.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "请输入画廊名称", trace_id).await;
                    return Ok(HandleResult::ignored());
                }
                let gallery = match client.resolve_gallery(&gallery_name).await {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("找不到画廊：{gallery_name}（请先 gallery open）"),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::ignored());
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 查询画廊失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::ignored());
                    }
                };

                let self_id = event_self_id(&event_raw);
                let (images, source_label) = match message::extract_images_from_event(
                    &mut host,
                    &event_raw,
                    self_id,
                    trace_id,
                )
                .await
                {
                    Ok(v) => v,
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 提取图片失败：{}", redact_secrets(&err)),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::ignored());
                    }
                };
                if images.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        "没有找到可上传的图片（可在当前消息或引用消息中带图）",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult::ignored());
                }
                mark_command_effective(&mut host, trace_id).await;
                let source_label = if source_label.is_empty() {
                    "当前消息"
                } else {
                    source_label.as_str()
                };

                let mut outcomes: Vec<UploadOutcome> = Vec::new();
                for (idx, image) in images.iter().enumerate() {
                    let index = idx + 1;
                    let url = &image.source_url;
                    let (mut filename, data) = match download_bytes(url).await {
                        Ok(v) => v,
                        Err(err) => {
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: None,
                                duplicate: None,
                                failure: Some(redact_secrets(&err)),
                            });
                            continue;
                        }
                    };
                    if !image.name.is_empty() {
                        filename = image.name.clone();
                    }
                    match client
                        .upload_image(gallery.id, &filename, data, false)
                        .await
                    {
                        Ok(uploaded) => {
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: Some(uploaded.id),
                                duplicate: None,
                                failure: None,
                            });
                        }
                        Err(err) if err.status_code == 409 && err.duplicate_image_id.is_some() => {
                            let dup_id = err.duplicate_image_id.unwrap();
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: None,
                                duplicate: Some(dup_id),
                                failure: None,
                            });
                            // Optional pages compare card; ignore path/API drift failures.
                            if !cfg.amiabot_pages.is_empty() {
                                let mut q = BTreeMap::new();
                                q.insert("current_image_url".into(), url.clone());
                                q.insert("duplicate_id".into(), dup_id.to_string());
                                q.insert("gallery".into(), gallery.name.clone());
                                let page =
                                    build_pages_url(&cfg.amiabot_pages, "/gallery/duplicate", &q);
                                match screenshot_and_upload(
                                    &mut host,
                                    &page,
                                    &format!("gallery-dup-{dup_id}"),
                                    json!({}),
                                )
                                .await
                                {
                                    Ok(img) => {
                                        let _ =
                                            send_image(&mut host, &event_raw, &img, trace_id).await;
                                    }
                                    Err(e) => {
                                        warn!(error=%e, "duplicate compare card failed");
                                        let _ = send_text(
                                            &mut host,
                                            &event_raw,
                                            &format!(
                                                "第 {index} 张图片与图库中已有图片相似，对应 ID：{dup_id}"
                                            ),
                                            trace_id,
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                let _ = send_text(
                                    &mut host,
                                    &event_raw,
                                    &format!(
                                        "第 {index} 张图片与图库中已有图片相似，对应 ID：{dup_id}"
                                    ),
                                    trace_id,
                                )
                                .await;
                            }
                        }
                        Err(err) if err.status_code == 409 => {
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: None,
                                duplicate: None,
                                failure: Some(format!(
                                    "冲突：{}",
                                    redact_secrets(&err.to_string())
                                )),
                            });
                        }
                        Err(err) => {
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: None,
                                duplicate: None,
                                failure: Some(redact_secrets(&err.to_string())),
                            });
                        }
                    }
                }
                let _ = send_text(
                    &mut host,
                    &event_raw,
                    &build_upload_summary(source_label, &gallery.name, &outcomes),
                    trace_id,
                )
                .await;
                return Ok(HandleResult::handled());
            }

            "cmd.gallery-view" => {
                let input = match_group(&match_data, 0);
                let num_raw = match_group(&match_data, 1);
                if input.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        "请输入图片 UUID 或画廊名称",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult::ignored());
                }

                // UUID path: ignore *N.
                if let Some(id) = parse_image_uuid(&input) {
                    match client.get_image(id).await {
                        Ok(image) => {
                            mark_command_effective(&mut host, trace_id).await;
                            if let Err(err) =
                                send_gallery_image(&mut host, &event_raw, &client, &image, trace_id)
                                    .await
                            {
                                let _ = send_text(
                                    &mut host,
                                    &event_raw,
                                    &format!("❌ 发送图片失败：{}", redact_secrets(&err)),
                                    trace_id,
                                )
                                .await;
                                return Ok(HandleResult::handled());
                            }
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &build_image_meta_text(&image),
                                trace_id,
                            )
                            .await;
                            return Ok(HandleResult::handled());
                        }
                        Err(err) if err.status_code == 404 => {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("找不到图片：{id}"),
                                trace_id,
                            )
                            .await;
                            return Ok(HandleResult::ignored());
                        }
                        Err(err) => {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("❌ 查询图片失败：{}", redact_secrets(&err.to_string())),
                                trace_id,
                            )
                            .await;
                            return Ok(HandleResult::ignored());
                        }
                    }
                }

                // Gallery-name path: honor *N (clamped).
                let count = parse_view_count(&num_raw);
                let gallery = match client.resolve_gallery(&input).await {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("找不到画廊或图片：{input}"),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::ignored());
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 查询画廊失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::ignored());
                    }
                };

                let images = match client.list_gallery_images(gallery.id).await {
                    Ok(v) => v,
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 获取画廊图片失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult::ignored());
                    }
                };
                if images.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        &format!("画廊「{}」还没有图片", gallery.name),
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult::ignored());
                }

                mark_command_effective(&mut host, trace_id).await;
                let picked = sample_random(&images, count as usize);
                for image in &picked {
                    if let Err(err) =
                        send_gallery_image(&mut host, &event_raw, &client, image, trace_id).await
                    {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 发送图片失败：{}", redact_secrets(&err)),
                            trace_id,
                        )
                        .await;
                        continue;
                    }
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        &build_image_meta_text(image),
                        trace_id,
                    )
                    .await;
                }
                return Ok(HandleResult::handled());
            }

            _ => return Ok(HandleResult::ignored()),
        }
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
            cfg: RwLock::new(GalleryConfig::default()),
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
    use regex::Regex;

    #[test]
    fn open_pattern_captures_name() {
        let re = Regex::new(OPEN_PATTERN).unwrap();
        let caps = re.captures("gallery open cats").unwrap();
        assert_eq!(caps.name("gallery_name").unwrap().as_str(), "cats");
        assert!(re.captures("gallery open").is_none());
    }

    #[test]
    fn upload_pattern_accepts_aliases() {
        let re = Regex::new(UPLOAD_PATTERN).unwrap();
        assert_eq!(
            re.captures("上传 cats")
                .unwrap()
                .name("gallery_name")
                .unwrap()
                .as_str(),
            "cats"
        );
        assert_eq!(
            re.captures("gallery upload cats")
                .unwrap()
                .name("gallery_name")
                .unwrap()
                .as_str(),
            "cats"
        );
        // Comma is still one \S+ token; multi-gallery is intentionally unsupported at command level.
        assert_eq!(
            re.captures("上传 a,b")
                .unwrap()
                .name("gallery_name")
                .unwrap()
                .as_str(),
            "a,b"
        );
    }

    #[test]
    fn view_pattern_and_count() {
        let re = Regex::new(VIEW_PATTERN).unwrap();
        let caps = re.captures("看 cats").unwrap();
        assert_eq!(caps.name("input").unwrap().as_str(), "cats");
        assert!(caps.name("num").is_none());

        let caps = re.captures("看 cats*3").unwrap();
        assert_eq!(caps.name("input").unwrap().as_str(), "cats");
        assert_eq!(caps.name("num").unwrap().as_str(), "3");

        let caps = re.captures("看 cats * 2").unwrap();
        assert_eq!(caps.name("input").unwrap().as_str(), "cats");
        assert_eq!(caps.name("num").unwrap().as_str(), "2");
    }

    #[test]
    fn view_count_clamp() {
        assert_eq!(parse_view_count(""), 1);
        assert_eq!(parse_view_count("0"), 1);
        assert_eq!(parse_view_count("3"), 3);
        assert_eq!(parse_view_count("5"), 5);
        assert_eq!(parse_view_count("9"), 5);
    }

    #[test]
    fn uuid_detect() {
        assert!(parse_image_uuid("11111111-1111-1111-1111-111111111111").is_some());
        assert!(parse_image_uuid("cats").is_none());
        assert!(parse_image_uuid("42").is_none());
    }

    #[test]
    fn sample_random_respects_n() {
        let items = vec![1, 2, 3, 4, 5];
        let got = sample_random(&items, 3);
        assert_eq!(got.len(), 3);
        for x in &got {
            assert!(items.contains(x));
        }
        let all = sample_random(&items, 10);
        assert_eq!(all.len(), 5);
        assert!(sample_random::<i32>(&[], 2).is_empty());
    }
}
