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
    build_pages_url, event_content, event_group_id, event_message_type, event_self_id,
    event_user_id, json_i64, redact_secrets, screenshot_and_upload, send_image, send_text,
};
use regex::Regex;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::warn;
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
            schema: Some(json!({
                "type":"object",
                "properties":{"amiabot_pages":{"type":"string"}},
                "additionalProperties": true
            })),
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
    if let Some(id) = event.get("reply").and_then(|r| json_i64(r.get("user_id"))) {
        return id;
    }
    // also message segments at
    if let Some(arr) = event.get("message").and_then(|v| v.as_array()) {
        for seg in arr {
            if seg.get("type").and_then(|v| v.as_str()) == Some("at")
                && let Some(id) = json_i64(seg.get("data").and_then(|d| d.get("qq")))
            {
                return id;
            }
        }
    }
    event_user_id(event)
}

fn s(v: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(x) = v.get(*k).and_then(|x| x.as_str()) {
            let t = x.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    String::new()
}

fn n_i64(v: &Value, keys: &[&str]) -> i64 {
    for k in keys {
        if let Some(x) = json_i64(v.get(*k)) {
            return x;
        }
    }
    0
}

fn n_i32(v: &Value, keys: &[&str]) -> i32 {
    n_i64(v, keys) as i32
}

fn first_positive(vals: &[i64]) -> i64 {
    vals.iter().copied().find(|v| *v > 0).unwrap_or(0)
}

fn normalize_enum(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn truncate_text(s: &str, limit: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= limit {
        return s.to_string();
    }
    s.chars().take(limit).collect::<String>() + "…"
}

fn format_birthday(y: i64, m: i64, d: i64) -> String {
    if y <= 0 || m <= 0 || d <= 0 {
        return String::new();
    }
    format!("{y:04}-{m:02}-{d:02}")
}

fn api_data(resp: &Value) -> Value {
    resp.get("data").cloned().unwrap_or(json!({}))
}

fn put(q: &mut BTreeMap<String, String>, k: &str, v: impl ToString) {
    let s = v.to_string();
    if !s.trim().is_empty() && s != "0" && s != "false" {
        q.insert(k.to_string(), s);
    }
}

fn put_always(q: &mut BTreeMap<String, String>, k: &str, v: impl ToString) {
    q.insert(k.to_string(), v.to_string());
}

async fn onebot(
    host: &mut HostClient,
    action: &str,
    params: Value,
    self_id: i64,
    trace_id: &str,
) -> Result<Value, StructuredError> {
    host.call_onebot(action, &params, self_id, trace_id).await
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
                let mut target = extract_target_user_id(&content, &event_raw);
                if target <= 0 {
                    target = event_user_id(&event_raw);
                }
                if target <= 0 {
                    let _ =
                        send_text(&mut host, &event_raw, "❌ 无法识别要查询的用户", trace_id).await;
                    return Ok(HandleResult {});
                }

                let stranger_resp = match onebot(
                    &mut host,
                    "get_stranger_info",
                    json!({"user_id": target, "no_cache": false}),
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
                            &format!("❌ 获取用户资料失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult {});
                    }
                };
                let stranger = api_data(&stranger_resp);

                let mut q = BTreeMap::new();
                put_always(&mut q, "id", target);
                let nickname = {
                    let n = s(&stranger, &["nickname", "nick"]);
                    if n.is_empty() { target.to_string() } else { n }
                };
                put(&mut q, "nickname", &nickname);
                put(&mut q, "remark", s(&stranger, &["remark"]));
                put(&mut q, "uid", s(&stranger, &["uid"]));
                put(&mut q, "qid", s(&stranger, &["qid", "q_id"]));
                put(
                    &mut q,
                    "long_nick",
                    truncate_text(&s(&stranger, &["long_nick", "longNick", "signature"]), 80),
                );
                put(
                    &mut q,
                    "sex",
                    normalize_enum(&s(&stranger, &["sex", "gender"])),
                );
                put(&mut q, "age", n_i32(&stranger, &["age"]));
                let reg_year = {
                    let y = n_i64(&stranger, &["reg_year", "regYear"]);
                    if y > 0 {
                        y
                    } else {
                        let ts = first_positive(&[
                            n_i64(&stranger, &["reg_time", "regTime"]),
                            n_i64(&stranger, &["regTime"]),
                        ]);
                        if ts > 10_000_000_000 {
                            // ms
                            ((ts / 1000) / 31_536_000) + 1970
                        } else if ts > 0 {
                            (ts / 31_536_000) + 1970
                        } else {
                            0
                        }
                    }
                };
                put(&mut q, "reg_year", reg_year);
                put(
                    &mut q,
                    "login_days",
                    n_i64(&stranger, &["login_days", "loginDays"]),
                );
                let mut qq_level = first_positive(&[
                    n_i64(&stranger, &["qq_level", "qqLevel"]),
                    n_i64(&stranger, &["level"]),
                ]);
                put(
                    &mut q,
                    "birthday",
                    format_birthday(
                        n_i64(&stranger, &["birthday_year", "birthdayYear"]),
                        n_i64(&stranger, &["birthday_month", "birthdayMonth"]),
                        n_i64(&stranger, &["birthday_day", "birthdayDay"]),
                    ),
                );
                put(
                    &mut q,
                    "phone_num",
                    s(&stranger, &["phone_num", "phoneNum"]),
                );
                put(&mut q, "email", s(&stranger, &["email"]));
                put(
                    &mut q,
                    "category_name",
                    s(&stranger, &["category_name", "categoryName"]),
                );
                put(
                    &mut q,
                    "category_id",
                    first_positive(&[
                        n_i64(&stranger, &["category_id", "categoryId"]),
                        n_i64(&stranger, &["categoryID"]),
                    ]),
                );
                put(
                    &mut q,
                    "is_vip",
                    stranger
                        .get("is_vip")
                        .or_else(|| stranger.get("isVip"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                );
                put(
                    &mut q,
                    "is_years_vip",
                    stranger
                        .get("is_years_vip")
                        .or_else(|| stranger.get("isYearsVip"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                );
                put(
                    &mut q,
                    "vip_level",
                    n_i64(&stranger, &["vip_level", "vipLevel"]),
                );
                put(
                    &mut q,
                    "online_status",
                    n_i64(&stranger, &["status", "online_status"]),
                );

                if event_message_type(&event_raw) == "group" {
                    let group_id = event_group_id(&event_raw);
                    if group_id > 0 {
                        match onebot(
                            &mut host,
                            "get_group_member_info",
                            json!({"group_id": group_id, "user_id": target, "no_cache": false}),
                            self_id,
                            trace_id,
                        )
                        .await
                        {
                            Ok(resp) => {
                                let m = api_data(&resp);
                                put(&mut q, "card", s(&m, &["card"]));
                                put(&mut q, "role", normalize_enum(&s(&m, &["role"])));
                                put(&mut q, "group_level", s(&m, &["level"]));
                                put(&mut q, "title", s(&m, &["title"]));
                                put(&mut q, "join_time", n_i64(&m, &["join_time", "joinTime"]));
                                put(
                                    &mut q,
                                    "last_sent_time",
                                    n_i64(&m, &["last_sent_time", "lastSentTime"]),
                                );
                                put(&mut q, "area", s(&m, &["area"]));
                                put(&mut q, "q_age", n_i64(&m, &["q_age", "qAge"]));
                                put(
                                    &mut q,
                                    "mute_until",
                                    n_i64(&m, &["shut_up_timestamp", "shutUpTimestamp"]),
                                );
                                put(
                                    &mut q,
                                    "title_expire_time",
                                    n_i64(&m, &["title_expire_time", "titleExpireTime"]),
                                );
                                if let Some(b) = m
                                    .get("card_changeable")
                                    .or_else(|| m.get("cardChangeable"))
                                    .and_then(|v| v.as_bool())
                                {
                                    put(&mut q, "card_changeable", b);
                                }
                                if let Some(b) = m.get("unfriendly").and_then(|v| v.as_bool()) {
                                    put(&mut q, "unfriendly", b);
                                }
                                if let Some(b) = m
                                    .get("is_robot")
                                    .or_else(|| m.get("isRobot"))
                                    .and_then(|v| v.as_bool())
                                {
                                    put(&mut q, "is_robot", b);
                                }
                                qq_level = first_positive(&[
                                    qq_level,
                                    n_i64(&m, &["qq_level", "qqLevel"]),
                                ]);
                            }
                            Err(err) => {
                                warn!(error=%err, "get_group_member_info failed; continue");
                            }
                        }
                    }
                }
                put(&mut q, "qq_level", qq_level);

                // optional napcat status
                if let Ok(st) = onebot(
                    &mut host,
                    "nc_get_user_status",
                    json!({"user_id": target}),
                    self_id,
                    trace_id,
                )
                .await
                {
                    let d = api_data(&st);
                    put(&mut q, "online_status", n_i64(&d, &["status"]));
                    put(
                        &mut q,
                        "online_ext_status",
                        n_i64(&d, &["ext_status", "extStatus"]),
                    );
                }

                if cfg.amiabot_pages.is_empty() {
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        &format!("用户 {target}\n昵称：{nickname}"),
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
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
                            &format!("❌ 资料卡生成失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            "cmd.query-group" => {
                if event_message_type(&event_raw) != "group" {
                    let _ =
                        send_text(&mut host, &event_raw, "该命令只能在群聊中使用", trace_id).await;
                    return Ok(HandleResult {});
                }
                let group_id = event_group_id(&event_raw);
                if group_id <= 0 {
                    let _ = send_text(&mut host, &event_raw, "❌ 无法识别当前群聊", trace_id).await;
                    return Ok(HandleResult {});
                }
                let info = match onebot(
                    &mut host,
                    "get_group_info",
                    json!({"group_id": group_id, "no_cache": false}),
                    self_id,
                    trace_id,
                )
                .await
                {
                    Ok(v) => api_data(&v),
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 获取群资料失败：{}", redact_secrets(&err.to_string())),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult {});
                    }
                };
                let mut q = BTreeMap::new();
                put_always(&mut q, "id", group_id);
                put(
                    &mut q,
                    "group_name",
                    s(&info, &["group_name", "groupName", "name"]),
                );
                put(
                    &mut q,
                    "member_count",
                    n_i64(&info, &["member_count", "memberCount"]),
                );
                put(
                    &mut q,
                    "max_member_count",
                    n_i64(&info, &["max_member_count", "maxMemberCount"]),
                );
                put(&mut q, "owner_id", n_i64(&info, &["owner_id", "ownerId"]));
                put(
                    &mut q,
                    "group_all_shut",
                    n_i64(&info, &["group_all_shut", "groupAllShut"]),
                );
                put(
                    &mut q,
                    "group_remark",
                    s(&info, &["group_remark", "groupRemark"]),
                );
                put(&mut q, "group_memo", s(&info, &["group_memo", "groupMemo"]));

                // optional extra detail APIs used by Go when available
                if let Ok(resp) = onebot(
                    &mut host,
                    "get_group_honor_info",
                    json!({"group_id": group_id, "type": "all"}),
                    self_id,
                    trace_id,
                )
                .await
                {
                    // pass condensed honor presence flag; full JSON may be too large for query
                    let d = api_data(&resp);
                    if d.get("current_talkative").is_some() || d.get("talkative_list").is_some() {
                        put(&mut q, "has_honor", true);
                    }
                }

                if cfg.amiabot_pages.is_empty() {
                    let name = q.get("group_name").cloned().unwrap_or_default();
                    let _ = send_text(
                        &mut host,
                        &event_raw,
                        &format!("群 {group_id}\n群名：{name}"),
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
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
                            &format!("❌ 群资料卡生成失败：{}", redact_secrets(&err.to_string())),
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

    #[test]
    fn extract_at_segment() {
        let event = json!({
            "user_id": 1,
            "message": [{"type":"at","data":{"qq":"998877"}}]
        });
        assert_eq!(extract_target_user_id("资料卡", &event), 998877);
    }

    #[test]
    fn birthday_format() {
        assert_eq!(format_birthday(2000, 1, 2), "2000-01-02");
        assert_eq!(format_birthday(0, 1, 2), "");
    }
}
