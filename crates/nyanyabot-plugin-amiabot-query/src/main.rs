use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, event_group_id, event_self_id, event_user_id, redact_secrets,
    screenshot_and_upload, send_image, send_text,
};
use regex::Regex;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

const USER_PATTERN: &str = r"^(?:query|资料卡|用户资料)(?:\s+.*|\[CQ:at,[^\]]+\].*)?$";
const GROUP_PATTERN: &str = r"^(?:group|群资料卡|群信息)$";

#[derive(Clone, Default)]
struct Cfg {
    amiabot_pages: String,
}

impl Cfg {
    fn from_value(v: &Value) -> Self {
        Self {
            amiabot_pages: plugin_common::normalize_http_base(
                v.get("amiabot_pages")
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
        name: "Amiabot Query".into(),
        plugin_id: "external.amiabot-query".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "用户/群资料卡查询".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Query 插件配置".into()),
            schema: Some(
                json!({"type":"object","properties":{"amiabot_pages":{"type":"string"}},"additionalProperties":true}),
            ),
            default: Some(json!({"amiabot_pages":""})),
        }),
        commands: vec![
            CommandListener {
                name: "query-user".into(),
                id: "cmd.query-user".into(),
                description: "查询用户资料卡".into(),
                pattern: USER_PATTERN.into(),
                match_raw: false,
                handler: "HandleUser".into(),
            },
            CommandListener {
                name: "query-group".into(),
                id: "cmd.query-group".into(),
                description: "查询群资料卡".into(),
                pattern: GROUP_PATTERN.into(),
                match_raw: false,
                handler: "HandleGroup".into(),
            },
        ],
        ..Default::default()
    }
}

fn extract_target_user_id(content: &str, event: &Value) -> i64 {
    if let Some(caps) = Regex::new(r"\[CQ:at,qq=(\d+)\]")
        .ok()
        .and_then(|re| re.captures(content))
        && let Ok(id) = caps[1].parse::<i64>()
    {
        return id;
    }
    if let Some(caps) = Regex::new(r"(\d{5,})")
        .ok()
        .and_then(|re| re.captures(content))
        && let Ok(id) = caps[1].parse::<i64>()
    {
        return id;
    }
    if let Some(id) = event
        .get("reply")
        .and_then(|r| plugin_common::json_i64(r.get("user_id")))
    {
        return id;
    }
    event_user_id(event)
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
        _match_data: Option<CommandMatch>,
        trace_id: &str,
    ) -> Result<HandleResult, StructuredError> {
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let cfg = self.cfg.read().clone();
        let self_id = event_self_id(&event_raw);
        match listener_id {
            "cmd.query-user" => {
                let content = event_content(&event_raw);
                let target = extract_target_user_id(&content, &event_raw);
                if target <= 0 {
                    let _ = send_text(&mut host, &event_raw, "无法解析目标用户", trace_id).await;
                    return Ok(HandleResult {});
                }
                let info = host
                    .call_onebot(
                        "get_stranger_info",
                        &json!({"user_id": target}),
                        self_id,
                        trace_id,
                    )
                    .await;
                let data = info
                    .ok()
                    .and_then(|v| v.get("data").cloned())
                    .unwrap_or(json!({}));
                if cfg.amiabot_pages.is_empty() {
                    let nick = data
                        .get("nickname")
                        .or_else(|| data.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        &format!("用户 {target}\n昵称：{nick}"),
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
                let mut q = BTreeMap::new();
                q.insert("user_id".into(), target.to_string());
                if let Some(n) = data.get("nickname").and_then(|v| v.as_str()) {
                    q.insert("nickname".into(), n.to_string());
                }
                let page = build_pages_url(&cfg.amiabot_pages, "/query/user", &q);
                let blob = format!(
                    "query-user-{}-{}",
                    target,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                );
                match screenshot_and_upload(&mut host, &page, &blob, json!({})).await {
                    Ok(url) => {
                        let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("查询失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            "cmd.query-group" => {
                let group_id = event_group_id(&event_raw);
                if group_id <= 0 {
                    let _ = send_text(&mut host, &event_raw, "请在群内使用群资料卡命令", trace_id)
                        .await;
                    return Ok(HandleResult {});
                }
                let info = host
                    .call_onebot(
                        "get_group_info",
                        &json!({"group_id": group_id}),
                        self_id,
                        trace_id,
                    )
                    .await;
                let data = info
                    .ok()
                    .and_then(|v| v.get("data").cloned())
                    .unwrap_or(json!({}));
                if cfg.amiabot_pages.is_empty() {
                    let name = data
                        .get("group_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        &format!("群 {group_id}\n群名：{name}"),
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
                let mut q = BTreeMap::new();
                q.insert("group_id".into(), group_id.to_string());
                if let Some(n) = data.get("group_name").and_then(|v| v.as_str()) {
                    q.insert("group_name".into(), n.to_string());
                }
                let page = build_pages_url(&cfg.amiabot_pages, "/query/group", &q);
                let blob = format!(
                    "query-group-{}-{}",
                    group_id,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                );
                match screenshot_and_upload(&mut host, &page, &blob, json!({})).await {
                    Ok(url) => {
                        let _ = send_image(&mut host, &event_raw, &url, trace_id).await;
                    }
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("查询失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            _ => {}
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
    use serde_json::json;
    #[test]
    fn extract_at() {
        let event = json!({"user_id": 1});
        assert_eq!(
            extract_target_user_id("资料卡 [CQ:at,qq=12345]", &event),
            12345
        );
    }
}
