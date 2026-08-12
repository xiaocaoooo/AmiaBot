use nyanyabot_proto::HostClient;
use serde_json::{Value, json};

const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone)]
pub struct ExtractedImage {
    pub source_url: String,
    pub name: String,
}

/// Extract images from current message, or via reply -> get_msg (Go parity).
pub async fn extract_images_from_event(
    host: &mut HostClient,
    event: &Value,
    self_id: i64,
    trace_id: &str,
) -> Result<(Vec<ExtractedImage>, String), String> {
    let message = event.get("message");
    let images = collect_images_from_value(host, message, 0, self_id, trace_id).await?;
    if !images.is_empty() {
        return Ok((dedupe(images), "当前消息".into()));
    }
    let reply_id = find_reply_id(message);
    if reply_id.is_empty() {
        return Ok((Vec::new(), String::new()));
    }
    let resp = host
        .call_onebot(
            "get_msg",
            &json!({"message_id": reply_id}),
            self_id,
            trace_id,
        )
        .await
        .map_err(|e| e.to_string())?;
    let data = resp.get("data").cloned().unwrap_or(resp);
    let images = collect_images_from_value(host, data.get("message"), 0, self_id, trace_id).await?;
    Ok((dedupe(images), "引用消息".into()))
}

fn find_reply_id(message: Option<&Value>) -> String {
    let Some(Value::Array(segs)) = message else {
        return String::new();
    };
    for seg in segs {
        if seg.get("type").and_then(|v| v.as_str()) != Some("reply") {
            continue;
        }
        if let Some(data) = seg.get("data") {
            for key in ["id", "message_id"] {
                if let Some(id) = data.get(key).and_then(|v| v.as_str()).map(str::trim)
                    && !id.is_empty()
                {
                    return id.to_string();
                }
                if let Some(id) = data.get(key).and_then(|v| v.as_i64()) {
                    return id.to_string();
                }
            }
        }
    }
    String::new()
}

async fn collect_images_from_value(
    host: &mut HostClient,
    value: Option<&Value>,
    depth: usize,
    self_id: i64,
    trace_id: &str,
) -> Result<Vec<ExtractedImage>, String> {
    if depth > MAX_DEPTH {
        return Err("消息嵌套层级过深".into());
    }
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    match value {
        Value::Array(arr) => {
            collect_images_from_segments(host, arr, depth, self_id, trace_id).await
        }
        Value::Object(map) => {
            if let Some(message) = map.get("message") {
                return Box::pin(collect_images_from_value(
                    host,
                    Some(message),
                    depth + 1,
                    self_id,
                    trace_id,
                ))
                .await;
            }
            if let Some(content) = map.get("content") {
                return Box::pin(collect_images_from_value(
                    host,
                    Some(content),
                    depth + 1,
                    self_id,
                    trace_id,
                ))
                .await;
            }
            if let Some(data) = map.get("data") {
                if let Some(content) = data.get("content") {
                    return Box::pin(collect_images_from_value(
                        host,
                        Some(content),
                        depth + 1,
                        self_id,
                        trace_id,
                    ))
                    .await;
                }
                if map.get("type").and_then(|v| v.as_str()) == Some("forward") {
                    let fid = first_non_empty(&[
                        data.get("id").and_then(value_to_string),
                        data.get("forward_id").and_then(value_to_string),
                        map.get("id").and_then(value_to_string),
                    ]);
                    if fid.is_empty() {
                        return Ok(Vec::new());
                    }
                    return Box::pin(collect_images_from_forward_id(
                        host,
                        &fid,
                        depth + 1,
                        self_id,
                        trace_id,
                    ))
                    .await;
                }
            }
            Ok(Vec::new())
        }
        Value::String(s) => Ok(collect_images_from_cq(s)),
        _ => Ok(Vec::new()),
    }
}

async fn collect_images_from_segments(
    host: &mut HostClient,
    segments: &[Value],
    depth: usize,
    self_id: i64,
    trace_id: &str,
) -> Result<Vec<ExtractedImage>, String> {
    let mut images = Vec::new();
    for seg in segments {
        let ty = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let data = seg.get("data");
        match ty {
            "image" => {
                if let Some(img) = parse_image_segment(data) {
                    images.push(img);
                }
            }
            "forward" => {
                let fid = first_non_empty(&[
                    data.and_then(|d| d.get("id")).and_then(value_to_string),
                    data.and_then(|d| d.get("forward_id"))
                        .and_then(value_to_string),
                    seg.get("id").and_then(value_to_string),
                ]);
                if !fid.is_empty() {
                    images.extend(
                        collect_images_from_forward_id(host, &fid, depth + 1, self_id, trace_id)
                            .await?,
                    );
                }
            }
            "node" => {
                if let Some(content) = data.and_then(|d| d.get("content")) {
                    images.extend(
                        Box::pin(collect_images_from_value(
                            host,
                            Some(content),
                            depth + 1,
                            self_id,
                            trace_id,
                        ))
                        .await?,
                    );
                }
            }
            _ => {}
        }
    }
    Ok(images)
}

async fn collect_images_from_forward_id(
    host: &mut HostClient,
    forward_id: &str,
    depth: usize,
    self_id: i64,
    trace_id: &str,
) -> Result<Vec<ExtractedImage>, String> {
    let mut resp = host
        .call_onebot(
            "get_forward_msg",
            &json!({"message_id": forward_id}),
            self_id,
            trace_id,
        )
        .await;
    if resp.is_err() {
        resp = host
            .call_onebot(
                "get_forward_msg",
                &json!({"id": forward_id}),
                self_id,
                trace_id,
            )
            .await;
    }
    let resp = resp.map_err(|e| e.to_string())?;
    let data = resp.get("data").cloned().unwrap_or(resp);
    let messages = data
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut images = Vec::new();
    for node in messages {
        images.extend(
            Box::pin(collect_images_from_value(
                host,
                Some(&node),
                depth + 1,
                self_id,
                trace_id,
            ))
            .await?,
        );
    }
    Ok(images)
}

fn parse_image_segment(data: Option<&Value>) -> Option<ExtractedImage> {
    let data = data?;
    let mut source = data
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if source.is_empty() {
        for key in ["file", "path"] {
            let c = data.get(key).and_then(|v| v.as_str()).unwrap_or("").trim();
            if looks_downloadable(c) {
                source = c.to_string();
                break;
            }
        }
    }
    if source.is_empty() {
        return None;
    }
    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| filename_from_url(&source));
    Some(ExtractedImage {
        source_url: source,
        name: if name.is_empty() {
            "image".into()
        } else {
            name
        },
    })
}

fn collect_images_from_cq(raw: &str) -> Vec<ExtractedImage> {
    let mut out = Vec::new();
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i + 10 < bytes.len() {
        if raw[i..].starts_with("[CQ:image")
            && let Some(end) = raw[i..].find(']')
        {
            let body = &raw[i + 1..i + end];
            let mut url = String::new();
            let mut file = String::new();
            let mut name = String::new();
            for part in body.split(',') {
                if let Some(v) = part.strip_prefix("url=") {
                    url = v.trim().to_string();
                } else if let Some(v) = part.strip_prefix("file=") {
                    file = v.trim().to_string();
                } else if let Some(v) = part.strip_prefix("name=") {
                    name = v.trim().to_string();
                }
            }
            let source = if !url.is_empty() {
                url
            } else if looks_downloadable(&file) {
                file
            } else {
                String::new()
            };
            if !source.is_empty() {
                out.push(ExtractedImage {
                    source_url: source.clone(),
                    name: if name.is_empty() {
                        filename_from_url(&source)
                    } else {
                        name
                    },
                });
            }
            i += end + 1;
            continue;
        }
        i += 1;
    }
    out
}

fn looks_downloadable(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("file://")
        || s.starts_with("base64://")
}

fn filename_from_url(url: &str) -> String {
    url.split('?')
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .unwrap_or("image")
        .to_string()
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn first_non_empty(vals: &[Option<String>]) -> String {
    for s in vals.iter().flatten() {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    String::new()
}

fn dedupe(images: Vec<ExtractedImage>) -> Vec<ExtractedImage> {
    let mut out = Vec::new();
    for img in images {
        if out
            .iter()
            .any(|x: &ExtractedImage| x.source_url == img.source_url)
        {
            continue;
        }
        out.push(img);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_reply_id() {
        let msg = json!([
            {"type":"reply","data":{"id":"12345"}},
            {"type":"image","data":{"url":"https://a/b.jpg"}}
        ]);
        assert_eq!(find_reply_id(Some(&msg)), "12345");
    }

    #[test]
    fn parses_image_segment() {
        let data = json!({"url":"https://x/y.png","name":"y.png"});
        let img = parse_image_segment(Some(&data)).unwrap();
        assert_eq!(img.source_url, "https://x/y.png");
        assert_eq!(img.name, "y.png");
    }

    #[test]
    fn cq_image_extract() {
        let imgs = collect_images_from_cq("[CQ:image,file=1,url=https://c/d.jpg]");
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].source_url, "https://c/d.jpg");
    }
}
