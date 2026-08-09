mod message;

mod client;

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use client::{GalleryClient, GalleryConfig, GalleryImageWithTags, download_bytes};
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, first_match_group, redact_secrets, screenshot_and_upload,
    send_image, send_text,
};
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::warn;
use tracing_subscriber::EnvFilter;

const CREATE_TAG_PATTERN: &str = r"(?i)^新建tag(?P<tag>.+)$";
const UPLOAD_PATTERN: &str = r"(?i)^上传(?P<tag>.+)$";
const VIEW_PATTERN: &str = r"(?i)^看(?P<input>.+)$";

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
        description: "画廊插件，支持创建标签、上传图片，以及按图片 ID 或标签查看图片".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Amiabot Gallery 插件配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "gallery_server":{"type":"string","description":"Gallery Server 的 API / Render 地址，默认 http://127.0.0.1:25006"},
                    "gallery_read_token":{"type":"string","description":"Gallery Server 读令牌"},
                    "gallery_write_token":{"type":"string","description":"Gallery Server 写令牌"},
                    "amiabot_pages":{"type":"string","description":"Amiabot Pages 地址，用于生成重复图片对比卡"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({
                "gallery_server":"http://127.0.0.1:25006",
                "gallery_read_token":"",
                "gallery_write_token":"",
                "amiabot_pages":""
            })),
        }),
        commands: vec![
            CommandListener {
                name: "gallery-create-tag".into(),
                id: "cmd.gallery-create-tag".into(),
                description: "新建画廊标签".into(),
                pattern: CREATE_TAG_PATTERN.into(),
                match_raw: false,
                handler: "HandleCreateTag".into(),
            },
            CommandListener {
                name: "gallery-upload".into(),
                id: "cmd.gallery-upload".into(),
                description: "将当前消息或引用消息中的图片上传到画廊".into(),
                pattern: UPLOAD_PATTERN.into(),
                match_raw: false,
                handler: "HandleUpload".into(),
            },
            CommandListener {
                name: "gallery-view".into(),
                id: "cmd.gallery-view".into(),
                description: "按图片 ID 或标签查看画廊中的图片".into(),
                pattern: VIEW_PATTERN.into(),
                match_raw: false,
                handler: "HandleView".into(),
            },
        ],
        ..Default::default()
    }
}

fn parse_tags(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for part in input.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn looks_like_image_id(input: &str) -> bool {
    input.trim().parse::<i64>().is_ok()
}

fn tag_names(img: &GalleryImageWithTags) -> Vec<String> {
    img.tags
        .iter()
        .map(|t| t.name.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn human_bytes(size: i64) -> String {
    const UNIT: f64 = 1024.0;
    let size = size as f64;
    if size < UNIT {
        return format!("{size} B");
    }
    let mut div = UNIT;
    let mut exp = 0;
    let mut n = size / UNIT;
    while n >= UNIT && exp < 5 {
        div *= UNIT;
        exp += 1;
        n = size / div;
    }
    let label = ['K', 'M', 'G', 'T', 'P', 'E'][exp];
    format!("{:.1} {label}iB", size / div)
}

fn build_image_meta_text(image: &GalleryImageWithTags) -> String {
    let mut lines = vec![format!("图片 #{}", image.image.id)];
    let names = tag_names(image);
    if !names.is_empty() {
        lines.push(format!("标签：{}", names.join("、")));
    }
    if image.image.width > 0 && image.image.height > 0 {
        lines.push(format!(
            "尺寸：{}×{}",
            image.image.width, image.image.height
        ));
    }
    if image.image.file_size > 0 {
        lines.push(format!("大小：{}", human_bytes(image.image.file_size)));
    }
    if let Some(ts) = image.image.created_at {
        lines.push(format!(
            "收录时间：{}",
            ts.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")
        ));
    }
    lines.join("\n")
}

struct UploadOutcome {
    index: usize,
    uploaded: Option<i64>,
    duplicate: Option<i64>,
    failure: Option<String>,
}

fn build_upload_summary(source_label: &str, tags: &[String], outcomes: &[UploadOutcome]) -> String {
    let mut success = Vec::new();
    let mut dup = Vec::new();
    let mut fail = Vec::new();
    for outcome in outcomes {
        if let Some(id) = outcome.uploaded {
            success.push(format!("#{id}"));
        } else if let Some(id) = outcome.duplicate {
            dup.push(format!("第 {} 张→#{id}", outcome.index));
        } else if let Some(msg) = &outcome.failure {
            fail.push(format!("第 {} 张：{msg}", outcome.index));
        }
    }
    let mut lines = vec![format!(
        "画廊上传结果（来源：{}）",
        if source_label.trim().is_empty() {
            "未知来源"
        } else {
            source_label
        }
    )];
    if !tags.is_empty() {
        lines.push(format!("目标标签：{}", tags.join("、")));
    }
    lines.push(format!(
        "成功 {} 张 / 重复 {} 张 / 失败 {} 张",
        success.len(),
        dup.len(),
        fail.len()
    ));
    if !success.is_empty() {
        lines.push(format!("新上传：{}", success.join("、")));
    }
    if !dup.is_empty() {
        lines.push(format!("重复图片：{}", dup.join("、")));
    }
    if !fail.is_empty() {
        lines.push(format!("失败详情：{}", fail.join("；")));
    }
    lines.join("\n")
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
            return Ok(HandleResult {});
        };
        let cfg = self.cfg.read().clone();
        let client = GalleryClient::new(&cfg);

        match listener_id {
            "cmd.gallery-create-tag" => {
                let tag_name = first_match_group(&match_data);
                if tag_name.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "标签名不能为空", trace_id).await;
                    return Ok(HandleResult {});
                }
                match client.create_tag(&tag_name).await {
                    Ok(tag) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("标签创建成功：#{}（ID：{}）", tag.name, tag.id),
                            trace_id,
                        )
                        .await;
                    }
                    Err(err) if err.status_code == 409 => {
                        match client.find_exact_tag(&tag_name).await {
                            Ok(Some(existing)) => {
                                let _ = send_text(
                                    &mut host,
                                    &event_raw,
                                    &format!(
                                        "标签已存在：#{}（ID：{}）",
                                        existing.name, existing.id
                                    ),
                                    trace_id,
                                )
                                .await;
                            }
                            _ => {
                                let _ = send_text(
                                    &mut host,
                                    &event_raw,
                                    &format!("创建标签失败：{}", redact_secrets(&err.to_string())),
                                    trace_id,
                                )
                                .await;
                            }
                        }
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("创建标签失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            "cmd.gallery-upload" => {
                let tags = parse_tags(&first_match_group(&match_data));
                if tags.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "请至少提供一个标签", trace_id).await;
                    return Ok(HandleResult {});
                }
                match client.find_missing_tags(&tags).await {
                    Ok(missing) if !missing.is_empty() => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("以下标签尚未创建，请先创建后再上传：{}", missing.join("、")),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult {});
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("标签校验失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult {});
                    }
                    _ => {}
                }

                let (images, source_label) =
                    match message::extract_images_from_event(&mut host, &event_raw).await {
                        Ok(v) => v,
                        Err(err) => {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("提取图片失败：{}", redact_secrets(&err)),
                                trace_id,
                            )
                            .await;
                            return Ok(HandleResult {});
                        }
                    };
                if images.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        "当前消息和引用消息中都没有找到图片",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
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
                    match client.upload_image(&filename, data, &tags, false).await {
                        Ok(uploaded) => {
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: Some(uploaded.image.id),
                                duplicate: None,
                                failure: None,
                            });
                        }
                        Err(err) if err.status_code == 409 && err.duplicate_image_id > 0 => {
                            outcomes.push(UploadOutcome {
                                index,
                                uploaded: None,
                                duplicate: Some(err.duplicate_image_id),
                                failure: None,
                            });
                            // duplicate compare card via pages if configured
                            if !cfg.amiabot_pages.is_empty() {
                                let mut q = BTreeMap::new();
                                q.insert("current_image_url".into(), url.clone());
                                q.insert("duplicate_id".into(), err.duplicate_image_id.to_string());
                                q.insert("current_tags".into(), tags.join(", "));
                                let page =
                                    build_pages_url(&cfg.amiabot_pages, "/gallery/duplicate", &q);
                                match screenshot_and_upload(
                                    &mut host,
                                    &page,
                                    &format!("gallery-dup-{}", err.duplicate_image_id),
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
                                                "第 {index} 张图片已存在于图库中，对应图片 ID：#{}",
                                                err.duplicate_image_id
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
                                        "第 {index} 张图片已存在于图库中，对应图片 ID：#{}",
                                        err.duplicate_image_id
                                    ),
                                    trace_id,
                                )
                                .await;
                            }
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
                    &build_upload_summary(source_label, &tags, &outcomes),
                    trace_id,
                )
                .await;
            }
            "cmd.gallery-view" => {
                let input = first_match_group(&match_data);
                if input.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        "请输入图片 ID 或至少一个标签",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
                // special pages modes from Go parseGalleryPagesViewInput
                let lower = input.to_lowercase();
                if lower == "alltags" || lower == "全部标签" || lower == "所有标签" {
                    if cfg.amiabot_pages.is_empty() {
                        let _ = send_text(&mut host, &event_raw, "amiabot_pages 未配置", trace_id)
                            .await;
                        return Ok(HandleResult {});
                    }
                    let page =
                        build_pages_url(&cfg.amiabot_pages, "/gallery/tags", &BTreeMap::new());
                    match screenshot_and_upload(&mut host, &page, "gallery-all-tags", json!({}))
                        .await
                    {
                        Ok(url) => {
                            let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
                        }
                        Err(err) => {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("查看失败：{err}"),
                                trace_id,
                            )
                            .await;
                        }
                    }
                    return Ok(HandleResult {});
                }

                if looks_like_image_id(&input) {
                    let id: i64 = input.trim().parse().unwrap_or(0);
                    match client.get_image(id).await {
                        Ok(image) => {
                            let file_url = client.build_render_url(image.image.id);
                            let _ = send_image(&mut host, &event_raw, &file_url, trace_id).await;
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &build_image_meta_text(&image),
                                trace_id,
                            )
                            .await;
                        }
                        Err(err) => {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("查看失败：{}", redact_secrets(&err.to_string())),
                                trace_id,
                            )
                            .await;
                        }
                    }
                    return Ok(HandleResult {});
                }

                let tags = parse_tags(&input);
                if tags.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        "请输入图片 ID 或至少一个标签",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
                // Prefer pages card when available (Go behavior for multi-tag/list).
                if !cfg.amiabot_pages.is_empty() {
                    let mut q = BTreeMap::new();
                    q.insert("tags".into(), tags.join(","));
                    let page = build_pages_url(&cfg.amiabot_pages, "/gallery/view", &q);
                    match screenshot_and_upload(
                        &mut host,
                        &page,
                        &format!("gallery-view-{}", tags.join("-")),
                        json!({}),
                    )
                    .await
                    {
                        Ok(url) => {
                            let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
                            return Ok(HandleResult {});
                        }
                        Err(err) => {
                            warn!(error=%err, "gallery pages view failed; fallback list");
                        }
                    }
                }
                match client.list_images_by_tag(&tags[0], 1, 5).await {
                    Ok((items, total)) => {
                        if items.is_empty() {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!("标签 {} 下没有图片", tags[0]),
                                trace_id,
                            )
                            .await;
                        } else {
                            let _ = send_text(
                                &mut host,
                                &event_raw,
                                &format!(
                                    "标签 {} 共 {total} 张，展示前 {} 张",
                                    tags[0],
                                    items.len()
                                ),
                                trace_id,
                            )
                            .await;
                            for img in items.iter().take(5) {
                                let url = client.build_render_url(img.image.id);
                                let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
                            }
                        }
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("查看失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            _ => {
                let _ = event_content(&event_raw);
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
        let got = serde_json::to_value(&desc).expect("serialize descriptor");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots/descriptor.json");
        if std::env::var("UPDATE_SNAPSHOTS").ok().as_deref() == Some("1") {
            std::fs::create_dir_all(path.parent().unwrap()).ok();
            std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap()).unwrap();
            return;
        }
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing descriptor snapshot at {}: {e}", path.display()));
        let expected: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            got, expected,
            "descriptor snapshot mismatch; run with UPDATE_SNAPSHOTS=1"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn parse_tags_dedup() {
        assert_eq!(
            parse_tags("Foo, bar, FOO,  "),
            vec!["Foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn image_id_detect() {
        assert!(looks_like_image_id("42"));
        assert!(!looks_like_image_id("cat"));
    }
}
