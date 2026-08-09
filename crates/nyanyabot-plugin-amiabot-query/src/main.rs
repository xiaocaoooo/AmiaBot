use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::Datelike;
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
                description: "查询用户资料卡（支持 query/资料卡/用户资料，可 @ 或指定对象）".into(),
                pattern: USER_PATTERN.into(),
                match_raw: false,
                handler: "HandleQueryUser".into(),
            },
            CommandListener {
                name: "query-group".into(),
                id: "cmd.query-group".into(),
                description: "查询当前群资料卡（如 group、群资料卡、群信息）".into(),
                pattern: GROUP_PATTERN.into(),
                match_raw: false,
                handler: "HandleQueryGroup".into(),
            },
        ],
        ..Default::default()
    }
}

fn extract_target_user_id(content: &str, event: &Value) -> i64 {
    // Go order: message segments first, then raw CQ:at in content. No bare digits / reply.
    if let Some(arr) = event.get("message").and_then(|v| v.as_array()) {
        for seg in arr {
            if let Some(id) = extract_at_from_segment(seg) {
                return id;
            }
        }
    }
    extract_at_from_content(content)
}

fn extract_at_from_segment(seg: &Value) -> Option<i64> {
    if !seg
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t.eq_ignore_ascii_case("at"))
    {
        return None;
    }
    let qq = seg
        .get("data")
        .and_then(|d| d.get("qq"))
        .map(|v| match v {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default();
    if qq.is_empty() || qq.eq_ignore_ascii_case("all") {
        return None;
    }
    qq.parse::<i64>().ok().filter(|id| *id > 0)
}

fn extract_at_from_content(content: &str) -> i64 {
    Regex::new(r"\[CQ:at,qq=(\d+)\]")
        .ok()
        .and_then(|re| re.captures(content))
        .and_then(|caps| caps[1].parse::<i64>().ok())
        .filter(|id| *id > 0)
        .unwrap_or(0)
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

/// Go setStringParam: skip empty only (keeps "false" / non-empty zeros-as-text).
fn put_str(q: &mut BTreeMap<String, String>, k: &str, v: impl ToString) {
    let s = v.to_string();
    let t = s.trim();
    if !t.is_empty() {
        q.insert(k.to_string(), t.to_string());
    }
}

/// Go setInt/setInt64Param: skip <= 0.
fn put_int(q: &mut BTreeMap<String, String>, k: &str, v: i64) {
    if v > 0 {
        q.insert(k.to_string(), v.to_string());
    }
}

fn put_bool(q: &mut BTreeMap<String, String>, k: &str, v: bool) {
    q.insert(k.to_string(), if v { "true" } else { "false" }.to_string());
}

fn put_always(q: &mut BTreeMap<String, String>, k: &str, v: impl ToString) {
    q.insert(k.to_string(), v.to_string());
}

// Back-compat wrapper used by group fetch for mixed values.
fn put(q: &mut BTreeMap<String, String>, k: &str, v: impl ToString) {
    let s = v.to_string();
    let t = s.trim();
    if t.is_empty() {
        return;
    }
    // Numeric zero skip (Go setInt*); keep "false".
    if t == "0" {
        return;
    }
    q.insert(k.to_string(), t.to_string());
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

fn format_timestamp(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    use chrono::{Local, TimeZone};
    // Go accepts unix seconds or milliseconds.
    let (secs, nsecs) = if ts > 1_000_000_000_000 {
        let ms = ts;
        (ms / 1000, ((ms % 1000) * 1_000_000) as u32)
    } else {
        (ts, 0)
    };
    match Local.timestamp_opt(secs, nsecs) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => ts.to_string(),
    }
}

fn normalize_reg_year(reg_year: i64, reg_time: i64) -> i64 {
    if reg_year > 0 {
        return reg_year;
    }
    if reg_time <= 0 {
        return 0;
    }
    use chrono::{Local, TimeZone};
    let secs = if reg_time > 1_000_000_000_000 {
        reg_time / 1000
    } else {
        reg_time
    };
    match Local.timestamp_opt(secs, 0) {
        chrono::LocalResult::Single(dt) => i64::from(dt.year()),
        _ => 0,
    }
}

fn normalize_qage(v: &Value) -> String {
    let text = match v {
        Value::Null => String::new(),
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    };
    if text.is_empty() || text == "0" {
        return String::new();
    }
    if text.contains('年') {
        text
    } else {
        format!("{text} 年")
    }
}

fn render_honor_member(v: &Value) -> String {
    if v.is_null() || !v.is_object() {
        return String::new();
    }
    let nick = s(v, &["nickname", "nick"]);
    let uid = n_i64(v, &["user_id", "userId"]);
    let desc = s(v, &["description", "desc"]);
    if nick.is_empty() && uid == 0 {
        return String::new();
    }
    let mut out = if !nick.is_empty() {
        nick
    } else {
        uid.to_string()
    };
    if uid > 0 {
        out = format!("{out}({uid})");
    }
    if !desc.is_empty() {
        out = format!("{out} {desc}");
    }
    out
}

fn first_honor_member(list: Option<&Value>) -> Value {
    list.and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(Value::Null)
}

fn bool_from_value(v: Option<&Value>) -> Option<bool> {
    match v? {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => Some(n.as_i64().unwrap_or(0) != 0),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

async fn fetch_group_page_params(
    host: &mut HostClient,
    group_id: i64,
    self_id: i64,
    trace_id: &str,
) -> Result<BTreeMap<String, String>, String> {
    let mut q = BTreeMap::new();
    put_always(&mut q, "id", group_id);

    let mut name = String::new();
    let mut remark = String::new();
    let mut level = 0i64;
    let mut create_time = 0i64;
    let mut member_count = 0i64;
    let mut max_member_count = 0i64;
    let mut active_members = 0i64;
    let mut owner_id = 0i64;
    let mut rules = String::new();
    let mut join_question = String::new();
    let mut description = String::new();
    let mut is_muted_all = false;
    let mut detail_ok = false;
    let mut base_ok = false;

    // get_group_detail_info (NapCat / extended)
    if let Ok(resp) = onebot(
        host,
        "get_group_detail_info",
        json!({"group_id": group_id}),
        self_id,
        trace_id,
    )
    .await
    {
        let d = api_data(&resp);
        detail_ok = true;
        name = s(&d, &["group_name", "groupName", "name"]);
        level = n_i64(&d, &["group_grade", "groupGrade", "level"]);
        create_time = n_i64(&d, &["group_create_time", "groupCreateTime", "create_time"]);
        member_count = first_positive(&[
            n_i64(&d, &["member_num", "memberNum"]),
            n_i64(&d, &["member_count", "memberCount"]),
        ]);
        max_member_count = first_positive(&[
            n_i64(&d, &["max_member_num", "maxMemberNum"]),
            n_i64(&d, &["max_member_count", "maxMemberCount"]),
        ]);
        active_members = n_i64(&d, &["active_member_num", "activeMemberNum"]);
        owner_id = first_positive(&[
            n_i64(&d, &["owner_uin", "ownerUin"]),
            n_i64(&d, &["owner_id", "ownerId"]),
        ]);
        rules = truncate_text(&s(&d, &["finger_memo", "fingerMemo"]), 120);
        join_question = truncate_text(&s(&d, &["group_question", "groupQuestion"]), 120);
        description = truncate_text(&s(&d, &["group_memo", "groupMemo", "description"]), 120);
        is_muted_all = n_i64(&d, &["shut_up_all_timestamp", "shutUpAllTimestamp"]) > 0;
    }

    // get_group_info base
    match onebot(
        host,
        "get_group_info",
        json!({"group_id": group_id, "no_cache": false}),
        self_id,
        trace_id,
    )
    .await
    {
        Ok(resp) => {
            let d = api_data(&resp);
            base_ok = true;
            if name.is_empty() {
                name = s(&d, &["group_name", "groupName", "name"]);
            }
            if member_count <= 0 {
                member_count = n_i64(&d, &["member_count", "memberCount"]);
            }
            if max_member_count <= 0 {
                max_member_count = n_i64(&d, &["max_member_count", "maxMemberCount"]);
            }
            remark = truncate_text(&s(&d, &["group_remark", "groupRemark", "remark"]), 64);
            if !detail_ok {
                is_muted_all = n_i64(&d, &["group_all_shut", "groupAllShut"]) > 0;
            }
            if owner_id <= 0 {
                owner_id = n_i64(&d, &["owner_id", "ownerId"]);
            }
        }
        Err(err) if !detail_ok => {
            return Err(format!("get_group_info 失败: {err}"));
        }
        Err(_) => {}
    }

    if name.is_empty() {
        name = format!("群聊 {group_id}");
    }
    if !detail_ok && !base_ok {
        return Err("获取群资料失败".into());
    }

    put(&mut q, "name", &name);
    put(&mut q, "remark", &remark);
    put(&mut q, "level", level);
    put(&mut q, "create_time", format_timestamp(create_time));
    put(&mut q, "member_count", member_count);
    put(&mut q, "max_member_count", max_member_count);
    put(&mut q, "active_member_count", active_members);
    put(&mut q, "owner_id", owner_id);
    put(&mut q, "rules", &rules);
    put(&mut q, "join_question", &join_question);
    put(&mut q, "description", &description);
    put_bool(&mut q, "is_muted_all", is_muted_all);

    // member list stats
    let mut admin_count = 0i64;
    let mut robot_count = 0i64;
    let mut muted_count = 0i64;
    let mut card_count = 0i64;
    let mut title_count = 0i64;
    let mut unfriendly_count = 0i64;
    let mut male_count = 0i64;
    let mut female_count = 0i64;
    let mut unknown_sex_count = 0i64;
    let mut derived_active = 0i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if let Ok(resp) = onebot(
        host,
        "get_group_member_list",
        json!({"group_id": group_id, "no_cache": false}),
        self_id,
        trace_id,
    )
    .await
    {
        let data = api_data(&resp);
        let members = if let Some(arr) = data.as_array() {
            arr.clone()
        } else if let Some(arr) = data.get("members").and_then(|v| v.as_array()) {
            arr.clone()
        } else {
            Vec::new()
        };
        for m in members {
            let role = normalize_enum(&s(&m, &["role"]));
            match role.as_str() {
                "owner" => {
                    let uid = n_i64(&m, &["user_id", "userId"]);
                    if owner_id <= 0 && uid > 0 {
                        owner_id = uid;
                    }
                }
                "admin" => admin_count += 1,
                _ => {}
            }
            if bool_from_value(m.get("is_robot").or_else(|| m.get("isRobot"))).unwrap_or(false) {
                robot_count += 1;
            }
            if bool_from_value(m.get("unfriendly")).unwrap_or(false) {
                unfriendly_count += 1;
            }
            let shut = n_i64(&m, &["shut_up_timestamp", "shutUpTimestamp"]);
            if shut > now {
                muted_count += 1;
            }
            if !s(&m, &["card"]).is_empty() {
                card_count += 1;
            }
            if !s(&m, &["title"]).is_empty() {
                title_count += 1;
            }
            match normalize_enum(&s(&m, &["sex"])).as_str() {
                "male" | "man" | "1" => male_count += 1,
                "female" | "woman" | "2" => female_count += 1,
                _ => unknown_sex_count += 1,
            }
            let last = n_i64(&m, &["last_sent_time", "lastSentTime"]);
            if last > now - 7 * 24 * 3600 {
                derived_active += 1;
            }
        }
        put(&mut q, "owner_id", owner_id);
        put(&mut q, "admin_count", admin_count);
        put(&mut q, "robot_count", robot_count);
        put(&mut q, "muted_count", muted_count);
        put(&mut q, "card_count", card_count);
        put(&mut q, "title_count", title_count);
        put(&mut q, "unfriendly_count", unfriendly_count);
        put(&mut q, "male_count", male_count);
        put(&mut q, "female_count", female_count);
        put(&mut q, "unknown_sex_count", unknown_sex_count);
        put(&mut q, "derived_active_member_count", derived_active);
    }

    // honors
    if let Ok(resp) = onebot(
        host,
        "get_group_honor_info",
        json!({"group_id": group_id, "type": "all"}),
        self_id,
        trace_id,
    )
    .await
    {
        let d = api_data(&resp);
        put(
            &mut q,
            "current_talkative",
            render_honor_member(d.get("current_talkative").unwrap_or(&Value::Null)),
        );
        put(
            &mut q,
            "talkative_top",
            render_honor_member(&first_honor_member(d.get("talkative_list"))),
        );
        put(
            &mut q,
            "performer_top",
            render_honor_member(&first_honor_member(d.get("performer_list"))),
        );
        put(
            &mut q,
            "legend_top",
            render_honor_member(&first_honor_member(d.get("legend_list"))),
        );
        put(
            &mut q,
            "emotion_top",
            render_honor_member(&first_honor_member(d.get("emotion_list"))),
        );
        put(
            &mut q,
            "strong_newbie_top",
            render_honor_member(&first_honor_member(d.get("strong_newbie_list"))),
        );
    }

    // notices
    if let Ok(resp) = onebot(
        host,
        "_get_group_notice",
        json!({"group_id": group_id}),
        self_id,
        trace_id,
    )
    .await
    {
        let data = api_data(&resp);
        let notices = if let Some(arr) = data.as_array() {
            arr.clone()
        } else if let Some(arr) = data.get("notices").and_then(|v| v.as_array()) {
            arr.clone()
        } else {
            Vec::new()
        };
        if let Some(latest) = notices.first() {
            let msg = latest.get("message").cloned().unwrap_or(Value::Null);
            let text = {
                let t = s(&msg, &["text"]);
                if t.is_empty() {
                    s(latest, &["text"])
                } else {
                    t
                }
            };
            put(&mut q, "latest_notice_text", truncate_text(&text, 160));
            let pt = n_i64(latest, &["publish_time", "publishTime"]);
            put(&mut q, "latest_notice_time", format_timestamp(pt));
            put(
                &mut q,
                "latest_notice_sender_id",
                n_i64(latest, &["sender_id", "senderId"]),
            );
            put(
                &mut q,
                "latest_notice_read_num",
                n_i64(latest, &["read_num", "readNum"]),
            );
        }
    }

    // extended info
    if let Ok(resp) = onebot(
        host,
        "get_group_info_ex",
        json!({"group_id": group_id}),
        self_id,
        trace_id,
    )
    .await
    {
        let d = api_data(&resp);
        let ext = d
            .get("extInfo")
            .or_else(|| d.get("ext_info"))
            .cloned()
            .unwrap_or(d);
        put(
            &mut q,
            "lucky_word",
            truncate_text(&s(&ext, &["luckyWord", "lucky_word"]), 32),
        );
        if owner_id <= 0 {
            let owner = ext
                .pointer("/groupOwnerID/memberUin")
                .or_else(|| ext.pointer("/group_owner_id/member_uin"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            if owner > 0 {
                put(&mut q, "owner_id", owner);
            }
        }
    }

    Ok(q)
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
                let mut nickname = {
                    let n = s(&stranger, &["nickname", "nick"]);
                    if n.is_empty() { target.to_string() } else { n }
                };
                put_str(&mut q, "nickname", &nickname);
                put_str(&mut q, "remark", s(&stranger, &["remark"]));
                put_str(&mut q, "qid", s(&stranger, &["qid", "q_id"]));
                put_str(
                    &mut q,
                    "long_nick",
                    truncate_text(&s(&stranger, &["long_nick", "longNick", "signature"]), 120),
                );
                let mut sex = normalize_enum(&s(&stranger, &["sex", "gender"]));
                put_str(&mut q, "sex", &sex);
                let mut age = n_i64(&stranger, &["age"]);
                put_int(&mut q, "age", age);
                let reg_year = normalize_reg_year(
                    n_i64(&stranger, &["reg_year", "regYear"]),
                    first_positive(&[
                        n_i64(&stranger, &["reg_time", "regTime"]),
                        n_i64(&stranger, &["regTime"]),
                    ]),
                );
                put_int(&mut q, "reg_year", reg_year);
                let mut qq_level = first_positive(&[
                    n_i64(&stranger, &["qq_level", "qqLevel"]),
                    n_i64(&stranger, &["level"]),
                ]);
                put_str(
                    &mut q,
                    "birthday",
                    format_birthday(
                        n_i64(&stranger, &["birthday_year", "birthdayYear"]),
                        n_i64(&stranger, &["birthday_month", "birthdayMonth"]),
                        n_i64(&stranger, &["birthday_day", "birthdayDay"]),
                    ),
                );
                put_str(
                    &mut q,
                    "phone_num",
                    s(&stranger, &["phone_num", "phoneNum"]),
                );
                put_str(&mut q, "email", s(&stranger, &["email"]));
                put_str(
                    &mut q,
                    "category_name",
                    s(&stranger, &["category_name", "categoryName"]),
                );
                let category_id =
                    first_positive(&[n_i64(&stranger, &["category_id", "categoryId"])]);
                if category_id > 0 {
                    put_str(&mut q, "category_id", category_id.to_string());
                }
                if let Some(b) =
                    bool_from_value(stranger.get("is_vip").or_else(|| stranger.get("isVip")))
                {
                    put_bool(&mut q, "is_vip", b);
                }
                if let Some(b) = bool_from_value(
                    stranger
                        .get("is_years_vip")
                        .or_else(|| stranger.get("isYearsVip")),
                ) {
                    put_bool(&mut q, "is_years_vip", b);
                }
                put_int(
                    &mut q,
                    "vip_level",
                    n_i64(&stranger, &["vip_level", "vipLevel"]),
                );
                put_int(
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
                                put_str(&mut q, "card", s(&m, &["card"]));
                                put_str(&mut q, "role", normalize_enum(&s(&m, &["role"])));
                                put_str(&mut q, "group_level", s(&m, &["level"]));
                                put_str(&mut q, "title", truncate_text(&s(&m, &["title"]), 48));
                                put_str(
                                    &mut q,
                                    "join_time",
                                    format_timestamp(n_i64(&m, &["join_time", "joinTime"])),
                                );
                                put_str(
                                    &mut q,
                                    "last_sent_time",
                                    format_timestamp(n_i64(
                                        &m,
                                        &["last_sent_time", "lastSentTime"],
                                    )),
                                );
                                put_str(&mut q, "area", s(&m, &["area"]));
                                let qage_src = m
                                    .get("qage")
                                    .or_else(|| m.get("q_age"))
                                    .or_else(|| m.get("qAge"))
                                    .cloned()
                                    .unwrap_or(Value::Null);
                                put_str(&mut q, "qage", normalize_qage(&qage_src));
                                put_str(
                                    &mut q,
                                    "mute_until",
                                    format_timestamp(n_i64(
                                        &m,
                                        &["shut_up_timestamp", "shutUpTimestamp"],
                                    )),
                                );
                                put_str(
                                    &mut q,
                                    "title_expire_time",
                                    format_timestamp(n_i64(
                                        &m,
                                        &["title_expire_time", "titleExpireTime"],
                                    )),
                                );
                                if let Some(b) = bool_from_value(m.get("unfriendly")) {
                                    put_bool(&mut q, "unfriendly", b);
                                }
                                if let Some(b) =
                                    bool_from_value(m.get("is_robot").or_else(|| m.get("isRobot")))
                                {
                                    put_bool(&mut q, "is_robot", b);
                                }
                                qq_level = first_positive(&[
                                    qq_level,
                                    n_i64(&m, &["qq_level", "qqLevel"]),
                                ]);
                                if nickname == target.to_string() {
                                    let alt = s(&m, &["card", "nickname"]);
                                    if !alt.is_empty() {
                                        nickname = alt;
                                        put_str(&mut q, "nickname", &nickname);
                                    }
                                }
                                if sex.is_empty() {
                                    sex = normalize_enum(&s(&m, &["sex"]));
                                    put_str(&mut q, "sex", &sex);
                                }
                                if age <= 0 {
                                    age = n_i64(&m, &["age"]);
                                    put_int(&mut q, "age", age);
                                }
                            }
                            Err(err) => {
                                warn!(error=%err, "get_group_member_info failed; continue");
                            }
                        }
                    }
                }
                put_int(&mut q, "qq_level", qq_level);

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
                    put_int(&mut q, "online_status", n_i64(&d, &["status"]));
                    put_int(
                        &mut q,
                        "online_ext_status",
                        n_i64(&d, &["ext_status", "extStatus"]),
                    );
                } else {
                    warn!(target_user_id = target, "nc_get_user_status failed; ignore");
                }

                if cfg.amiabot_pages.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "❌ 服务未配置", trace_id).await;
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
                let q = match fetch_group_page_params(&mut host, group_id, self_id, trace_id).await
                {
                    Ok(q) => q,
                    Err(err) => {
                        let _ = send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 获取群资料失败：{}", redact_secrets(&err)),
                            trace_id,
                        )
                        .await;
                        return Ok(HandleResult {});
                    }
                };
                if cfg.amiabot_pages.is_empty() {
                    let _ = send_text(&mut host, &event_raw, "❌ 服务未配置", trace_id).await;
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
    fn extract_at_segment_preferred_over_content() {
        let event = json!({
            "user_id": 1,
            "message": [{"type":"at","data":{"qq":"998877"}}]
        });
        assert_eq!(
            extract_target_user_id("资料卡 [CQ:at,qq=12345]", &event),
            998877
        );
    }

    #[test]
    fn extract_ignores_bare_digits_and_all() {
        let event = json!({
            "user_id": 1,
            "message": [{"type":"at","data":{"qq":"all"}}]
        });
        assert_eq!(extract_target_user_id("资料卡 123456789", &event), 0);
    }

    #[test]
    fn birthday_format() {
        assert_eq!(format_birthday(2000, 1, 2), "2000-01-02");
        assert_eq!(format_birthday(0, 1, 2), "");
    }

    #[test]
    fn honor_render_and_timestamp() {
        let v = serde_json::json!({"nickname":"Alice","user_id":123,"description":"day1"});
        let s = super::render_honor_member(&v);
        assert!(s.contains("Alice(123)"));
        assert_eq!(super::format_timestamp(0), "");
        assert!(!super::format_timestamp(1_700_000_000).is_empty());
        assert!(super::format_timestamp(1_700_000_000_000).contains('-'));
    }

    #[test]
    fn normalize_qage_appends_year() {
        assert_eq!(normalize_qage(&json!(8)), "8 年");
        assert_eq!(normalize_qage(&json!("8 年")), "8 年");
        assert_eq!(normalize_qage(&json!(0)), "");
        assert_eq!(normalize_qage(&json!("")), "");
    }

    #[test]
    fn put_helpers_match_go_semantics() {
        let mut q = BTreeMap::new();
        put_str(&mut q, "unfriendly", "false");
        put_int(&mut q, "age", 0);
        put_int(&mut q, "vip_level", 7);
        put_bool(&mut q, "is_robot", false);
        assert_eq!(q.get("unfriendly").map(String::as_str), Some("false"));
        assert!(!q.contains_key("age"));
        assert_eq!(q.get("vip_level").map(String::as_str), Some("7"));
        assert_eq!(q.get("is_robot").map(String::as_str), Some("false"));
    }

    #[test]
    fn user_page_omits_removed_keys_and_uses_qage() {
        let mut q = BTreeMap::new();
        put_always(&mut q, "id", 12345);
        put_str(&mut q, "qage", normalize_qage(&json!("8")));
        put_str(&mut q, "join_time", format_timestamp(1_700_000_000));
        put_bool(&mut q, "unfriendly", false);
        assert!(!q.contains_key("uid"));
        assert!(!q.contains_key("login_days"));
        assert!(!q.contains_key("card_changeable"));
        assert_eq!(q.get("qage").map(String::as_str), Some("8 年"));
        assert!(q["join_time"].contains('-'));
        assert_eq!(q.get("unfriendly").map(String::as_str), Some("false"));
    }
}
