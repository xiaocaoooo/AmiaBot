use std::collections::BTreeMap;

use nyanyabot_proto::HostClient;
use serde_json::{Value, json};
use url::Url;

pub fn normalize_http_base(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        s.trim_end_matches('/').to_string()
    } else {
        format!("http://{}", s.trim_end_matches('/'))
    }
}

pub fn join_url(base: &str, path: &str) -> String {
    let base = normalize_http_base(base);
    let path = path.trim_start_matches('/');
    if base.is_empty() {
        return format!("/{path}");
    }
    format!("{base}/{path}")
}

/// Build `base/path?k=v` with sorted query keys for stable URLs.
pub fn build_pages_url(base: &str, path: &str, query: &BTreeMap<String, String>) -> String {
    let mut url = join_url(base, path);
    if query.is_empty() {
        return url;
    }
    let mut first = true;
    for (k, v) in query {
        if k.is_empty() {
            continue;
        }
        let enc_k = urlencoding_encode(k);
        let enc_v = urlencoding_encode(v);
        if first {
            url.push('?');
            first = false;
        } else {
            url.push('&');
        }
        url.push_str(&enc_k);
        url.push('=');
        url.push_str(&enc_v);
    }
    url
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn redact_secrets(s: &str) -> String {
    let mut out = s.to_string();
    for key in [
        "token",
        "password",
        "authorization",
        "blob_token",
        "api_key",
        "gallery_read_token",
        "gallery_write_token",
    ] {
        let needle = format!("{key}=");
        if let Some(idx) = out.find(&needle) {
            let start = idx + needle.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
                .map(|i| start + i)
                .unwrap_or(out.len());
            out.replace_range(start..end, "***");
        }
    }
    out
}

pub fn event_self_id(event: &Value) -> i64 {
    json_i64(event.get("self_id")).unwrap_or(0)
}

pub fn event_user_id(event: &Value) -> i64 {
    json_i64(event.get("user_id")).unwrap_or(0)
}

pub fn event_group_id(event: &Value) -> i64 {
    json_i64(event.get("group_id")).unwrap_or(0)
}

pub fn event_message_type(event: &Value) -> &str {
    event
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn event_content(event: &Value) -> String {
    if let Some(s) = event.get("content").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(s) = event.get("raw_message").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    String::new()
}

pub fn json_i64(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    if let Some(n) = v.as_u64() {
        return i64::try_from(n).ok();
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse().ok();
    }
    None
}

pub fn first_match_group(match_data: &Option<nyanyabot_proto::CommandMatch>) -> String {
    match_data
        .as_ref()
        .and_then(|m| m.groups.first())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub async fn send_text(
    host: &mut HostClient,
    event: &Value,
    text: &str,
    trace_id: &str,
) -> Result<(), nyanyabot_proto::StructuredError> {
    let self_id = event_self_id(event);
    if event_message_type(event) == "group" {
        host.call_onebot(
            "send_group_msg",
            &json!({"group_id": event_group_id(event), "message": text}),
            self_id,
            trace_id,
        )
        .await?;
    } else {
        host.call_onebot(
            "send_private_msg",
            &json!({"user_id": event_user_id(event), "message": text}),
            self_id,
            trace_id,
        )
        .await?;
    }
    Ok(())
}

pub async fn send_image(
    host: &mut HostClient,
    event: &Value,
    image_url: &str,
    trace_id: &str,
) -> Result<(), nyanyabot_proto::StructuredError> {
    let segment = json!([{"type":"image","data":{"file": image_url}}]);
    let self_id = event_self_id(event);
    if event_message_type(event) == "group" {
        host.call_onebot(
            "send_group_msg",
            &json!({"group_id": event_group_id(event), "message": segment}),
            self_id,
            trace_id,
        )
        .await?;
    } else {
        host.call_onebot(
            "send_private_msg",
            &json!({"user_id": event_user_id(event), "message": segment}),
            self_id,
            trace_id,
        )
        .await?;
    }
    Ok(())
}

pub async fn send_forward(
    host: &mut HostClient,
    event: &Value,
    nodes: Value,
    trace_id: &str,
) -> Result<(), nyanyabot_proto::StructuredError> {
    let self_id = event_self_id(event);
    if event_message_type(event) == "group" {
        host.call_onebot(
            "send_group_forward_msg",
            &json!({"group_id": event_group_id(event), "messages": nodes}),
            self_id,
            trace_id,
        )
        .await?;
    } else {
        host.call_onebot(
            "send_private_forward_msg",
            &json!({"user_id": event_user_id(event), "messages": nodes}),
            self_id,
            trace_id,
        )
        .await?;
    }
    Ok(())
}

pub async fn screenshot_and_upload(
    host: &mut HostClient,
    page_url: &str,
    blob_id: &str,
    extra: Value,
) -> Result<String, nyanyabot_proto::StructuredError> {
    let mut params = extra;
    if !params.is_object() {
        params = json!({});
    }
    params
        .as_object_mut()
        .unwrap()
        .insert("page_url".into(), json!(page_url));
    let built = host
        .call_dependency("external.screenshot", "screenshot.build_url", &params)
        .await?;
    let shot_url = built
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| nyanyabot_proto::StructuredError::internal("screenshot url missing"))?
        .to_string();
    let uploaded = host
        .call_dependency(
            "external.blobserver",
            "blob.upload_remote",
            &json!({
                "download_url": shot_url,
                "blob_id": blob_id,
                "kind": "image"
            }),
        )
        .await?;
    uploaded
        .get("onebot_url")
        .or_else(|| uploaded.get("blob_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| nyanyabot_proto::StructuredError::internal("blob upload result missing url"))
}

pub async fn upload_remote_blob(
    host: &mut HostClient,
    download_url: &str,
    blob_id: &str,
    kind: &str,
) -> Result<String, nyanyabot_proto::StructuredError> {
    let uploaded = host
        .call_dependency(
            "external.blobserver",
            "blob.upload_remote",
            &json!({
                "download_url": download_url,
                "blob_id": blob_id,
                "kind": kind
            }),
        )
        .await?;
    uploaded
        .get("onebot_url")
        .or_else(|| uploaded.get("blob_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| nyanyabot_proto::StructuredError::internal("blob upload result missing url"))
}

/// Collect image file URLs from message segments / CQ codes, including reply chain if present.
pub fn extract_image_urls(event: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_images_from_value(event.get("message"), &mut out);
    if out.is_empty()
        && let Some(raw) = event.get("raw_message").and_then(|v| v.as_str())
    {
        collect_images_from_cq(raw, &mut out);
    }
    // reply / source message if nested
    if let Some(reply) = event.get("reply") {
        collect_images_from_value(reply.get("message"), &mut out);
        if let Some(raw) = reply.get("raw_message").and_then(|v| v.as_str()) {
            collect_images_from_cq(raw, &mut out);
        }
    }
    out
}

fn collect_images_from_value(message: Option<&Value>, out: &mut Vec<String>) {
    let Some(message) = message else { return };
    match message {
        Value::Array(arr) => {
            for seg in arr {
                let ty = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if ty == "image"
                    && let Some(file) = seg
                        .get("data")
                        .and_then(|d| d.get("url").or_else(|| d.get("file")))
                        .and_then(|v| v.as_str())
                {
                    let f = file.trim();
                    if !f.is_empty() && !out.iter().any(|x| x == f) {
                        out.push(f.to_string());
                    }
                }
            }
        }
        Value::String(s) => collect_images_from_cq(s, out),
        _ => {}
    }
}

fn collect_images_from_cq(raw: &str, out: &mut Vec<String>) {
    // [CQ:image,file=xxx,url=yyy]
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i + 10 < bytes.len() {
        if raw[i..].starts_with("[CQ:image")
            && let Some(end) = raw[i..].find(']')
        {
            let body = &raw[i + 1..i + end];
            for part in body.split(',') {
                if let Some(v) = part.strip_prefix("url=") {
                    let v = v.trim();
                    if !v.is_empty() && !out.iter().any(|x| x == v) {
                        out.push(v.to_string());
                    }
                } else if let Some(v) = part.strip_prefix("file=") {
                    let v = v.trim();
                    if (v.starts_with("http://") || v.starts_with("https://"))
                        && !out.iter().any(|x| x == v)
                    {
                        out.push(v.to_string());
                    }
                }
            }
            i += end + 1;
            continue;
        }
        i += 1;
    }
}

pub fn ensure_url(raw: &str) -> Result<Url, String> {
    Url::parse(raw).map_err(|e| e.to_string())
}

pub mod jsonata {
    use serde_json::{Value, json};

    /// Minimal JSONata-like subset used by AmiaBot filters/templates.
    pub fn evaluate(expr: &str, data: &Value) -> Result<Value, String> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Ok(data.clone());
        }
        if let Some((l, r)) = split_top(expr, " or ") {
            let lv = truthy(&evaluate(l, data)?);
            if lv {
                return Ok(Value::Bool(true));
            }
            return Ok(Value::Bool(truthy(&evaluate(r, data)?)));
        }
        if let Some((l, r)) = split_top(expr, " and ") {
            let lv = truthy(&evaluate(l, data)?);
            if !lv {
                return Ok(Value::Bool(false));
            }
            return Ok(Value::Bool(truthy(&evaluate(r, data)?)));
        }
        if let Some((l, r)) = split_top(expr, "!=") {
            return Ok(Value::Bool(evaluate(l, data)? != evaluate(r, data)?));
        }
        if let Some((l, r)) = split_top(expr, "=") {
            return Ok(Value::Bool(evaluate(l, data)? == evaluate(r, data)?));
        }
        eval_atom(expr, data)
    }

    pub fn is_truthy_filter(expr: &str, data: &Value) -> bool {
        match evaluate(expr, data) {
            Ok(v) => truthy(&v),
            Err(_) => false,
        }
    }

    fn eval_atom(expr: &str, data: &Value) -> Result<Value, String> {
        let expr = expr.trim();
        if expr == "$" {
            return Ok(data.clone());
        }
        if let Some(path) = expr.strip_prefix("$.") {
            let mut cur = data;
            for part in path.split('.') {
                if part.is_empty() {
                    continue;
                }
                if let Ok(idx) = part.parse::<usize>() {
                    cur = cur.get(idx).ok_or_else(|| format!("missing index {idx}"))?;
                } else {
                    cur = cur.get(part).ok_or_else(|| format!("missing {part}"))?;
                }
            }
            return Ok(cur.clone());
        }
        if expr == "true" {
            return Ok(Value::Bool(true));
        }
        if expr == "false" {
            return Ok(Value::Bool(false));
        }
        if expr == "null" {
            return Ok(Value::Null);
        }
        if let Ok(n) = expr.parse::<i64>() {
            return Ok(json!(n));
        }
        if let Ok(n) = expr.parse::<f64>() {
            return Ok(json!(n));
        }
        if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
            return Ok(Value::String(expr[1..expr.len() - 1].to_string()));
        }
        if expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 2 {
            return Ok(Value::String(expr[1..expr.len() - 1].to_string()));
        }
        Ok(Value::String(expr.to_string()))
    }

    fn truthy(v: &Value) -> bool {
        match v {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(false),
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
        }
    }

    fn split_top<'a>(expr: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
        let bytes = expr.as_bytes();
        let sep_b = sep.as_bytes();
        let mut i = 0;
        let mut in_sq = false;
        let mut in_dq = false;
        while i + sep_b.len() <= bytes.len() {
            let c = bytes[i] as char;
            if c == '\'' && !in_dq {
                in_sq = !in_sq;
            } else if c == '"' && !in_sq {
                in_dq = !in_dq;
            } else if !in_sq && !in_dq && bytes[i..].starts_with(sep_b) {
                let l = expr[..i].trim();
                let r = expr[i + sep.len()..].trim();
                if !l.is_empty() && !r.is_empty() {
                    return Some((l, r));
                }
            }
            i += 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn url_helpers() {
        assert_eq!(normalize_http_base("example.com/"), "http://example.com");
        assert_eq!(join_url("http://a", "/b"), "http://a/b");
        let mut q = BTreeMap::new();
        q.insert("pid".into(), "123".into());
        assert_eq!(
            build_pages_url("http://pages", "/pixiv/illust/info", &q),
            "http://pages/pixiv/illust/info?pid=123"
        );
    }

    #[test]
    fn redact() {
        let s = redact_secrets("token=abc password=xyz");
        assert!(s.contains("***"));
        assert!(!s.contains("abc"));
    }

    #[test]
    fn event_fields() {
        let event = json!({
            "self_id": 11,
            "user_id": "22",
            "group_id": 33,
            "message_type": "group",
            "content": "hi"
        });
        assert_eq!(event_self_id(&event), 11);
        assert_eq!(event_user_id(&event), 22);
        assert_eq!(event_group_id(&event), 33);
        assert_eq!(event_message_type(&event), "group");
        assert_eq!(event_content(&event), "hi");
    }

    #[test]
    fn extract_images() {
        let event = json!({
            "message": [
                {"type":"text","data":{"text":"x"}},
                {"type":"image","data":{"url":"https://a/b.jpg"}}
            ],
            "raw_message": "[CQ:image,file=https://c/d.png]"
        });
        let urls = extract_image_urls(&event);
        assert!(urls.iter().any(|u| u.contains("a/b.jpg")));
    }

    #[test]
    fn join_url_empty_base() {
        assert_eq!(join_url("", "x"), "/x");
        assert_eq!(normalize_http_base(""), "");
    }

    #[test]
    fn jsonata_subset() {
        let data = json!({"post_type":"message","user_id":1,"arr":[10,20],"nested":{"x":"y"}});
        assert_eq!(jsonata::evaluate("$", &data).unwrap(), data);
        assert_eq!(jsonata::evaluate("$.user_id", &data).unwrap(), json!(1));
        assert_eq!(jsonata::evaluate("$.arr.1", &data).unwrap(), json!(20));
        assert_eq!(
            jsonata::evaluate("$.post_type = \"message\"", &data).unwrap(),
            json!(true)
        );
        assert_eq!(
            jsonata::evaluate("$.user_id = 2 or $.user_id = 1", &data).unwrap(),
            json!(true)
        );
        assert_eq!(
            jsonata::evaluate("$.nested.x = 'y' and $.user_id = 1", &data).unwrap(),
            json!(true)
        );
        assert!(jsonata::is_truthy_filter(
            "$.post_type = \"message\"",
            &data
        ));
    }
}
