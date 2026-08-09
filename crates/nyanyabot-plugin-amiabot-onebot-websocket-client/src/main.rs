use std::sync::Arc;

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use nyanyabot_proto::{
    CommandMatch, ConfigSpec, Descriptor, EventListener, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::jsonata;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock as AsyncRwLock, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone, Debug)]
struct Rule {
    expression: String,
    action: String,
    reply_json: String,
}

struct Plug {
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
    upstream_rules: RwLock<Vec<Rule>>,
    downstream_rules: RwLock<Vec<Rule>>,
    outbound: Mutex<Option<tokio::task::JoinHandle<()>>>,
    stop: Arc<tokio::sync::Notify>,
    send_tx: Mutex<Option<mpsc::Sender<String>>>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "OneBot WebSocket Client".into(),
        plugin_id: "external.amiabot-onebot-websocket-client".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "OneBot WebSocket 客户端插件，实现上游 NyaNyaBot 事件与下游 WebSocket 消息的双向过滤与透传。".into(),
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("OneBot WebSocket Client 插件配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "ws_url":{"type":"string","description":"下游 WebSocket 服务端 URL，例如 ws://127.0.0.1:8080/onebot"},
                    "url":{"type":"string","description":"下游 OneBot WS URL（兼容字段）"},
                    "ws_token":{"type":"string","description":"下游 WebSocket 连接所需的 Access Token / Authorization Bearer"},
                    "access_token":{"type":"string","description":"兼容旧字段"},
                    "upstream_rules":{
                        "type":"array",
                        "description":"上游消息（NyaNyaBot 事件 -> 下游 WS）的过滤规则列表。按顺序匹配，首个匹配的规则生效。若无规则匹配，默认透传。",
                        "items":{
                            "type":"object",
                            "properties":{
                                "expression":{"type":"string"},
                                "action":{"type":"string","enum":["passthrough","reject"]}
                            },
                            "required":["expression","action"]
                        }
                    },
                    "downstream_rules":{
                        "type":"array",
                        "description":"下游消息（下游 WS 动作请求 -> NyaNyaBot）的过滤规则列表。按顺序匹配，首个匹配的规则生效。若无规则匹配，默认透传。",
                        "items":{
                            "type":"object",
                            "properties":{
                                "expression":{"type":"string"},
                                "action":{"type":"string","enum":["passthrough","reject","match"]},
                                "reply_json":{"type":"string"}
                            },
                            "required":["expression","action"]
                        }
                    },
                    "event_filter":{"type":"string","description":"兼容旧版单过滤器；若 upstream_rules 为空则作为透传表达式"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({
                "ws_url":"",
                "ws_token":"",
                "upstream_rules":[],
                "downstream_rules":[]
            })),
        }),
        events: vec![
            EventListener {
                name: "ws-client-message".into(),
                id: "evt.onebot-ws.message".into(),
                description: "捕获所有上游 message 消息事件".into(),
                event: "message".into(),
                handler: "Handle".into(),
            },
            EventListener {
                name: "ws-client-notice".into(),
                id: "evt.onebot-ws.notice".into(),
                description: "捕获所有上游 notice 通知事件".into(),
                event: "notice".into(),
                handler: "Handle".into(),
            },
            EventListener {
                name: "ws-client-request".into(),
                id: "evt.onebot-ws.request".into(),
                description: "捕获所有上游 request 请求事件".into(),
                event: "request".into(),
                handler: "Handle".into(),
            },
            EventListener {
                name: "ws-client-meta".into(),
                id: "evt.onebot-ws.meta_event".into(),
                description: "捕获所有上游 meta_event 元事件".into(),
                event: "meta_event".into(),
                handler: "Handle".into(),
            },
        ],
        ..Default::default()
    }
}

fn cfg_url(cfg: &Value) -> String {
    cfg.get("ws_url")
        .or_else(|| cfg.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn cfg_token(cfg: &Value) -> String {
    cfg.get("ws_token")
        .or_else(|| cfg.get("access_token"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_rules(cfg: &Value, key: &str) -> Vec<Rule> {
    let mut out = Vec::new();
    if let Some(arr) = cfg.get(key).and_then(|v| v.as_array()) {
        for item in arr {
            let expression = item
                .get("expression")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let action = item
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if expression.is_empty() || action.is_empty() {
                continue;
            }
            let reply_json = item
                .get("reply_json")
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            out.push(Rule {
                expression,
                action,
                reply_json,
            });
        }
    }
    out
}

/// First matching rule decides; default passthrough when none match (Go parity).
fn decide_upstream(rules: &[Rule], event: &Value) -> bool {
    let mut passthrough = true;
    for rule in rules {
        if jsonata::is_truthy_filter(&rule.expression, event) {
            match rule.action.as_str() {
                "reject" => passthrough = false,
                "passthrough" => passthrough = true,
                _ => {}
            }
            break;
        }
    }
    passthrough
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DownstreamDecision {
    Passthrough,
    Reject,
    Match(String),
}

fn decide_downstream(rules: &[Rule], data: &Value) -> DownstreamDecision {
    let mut passthrough = true;
    let mut matched: Option<&Rule> = None;
    for rule in rules {
        if jsonata::is_truthy_filter(&rule.expression, data) {
            matched = Some(rule);
            match rule.action.as_str() {
                "reject" => passthrough = false,
                "passthrough" => passthrough = true,
                "match" => passthrough = false,
                _ => {}
            }
            break;
        }
    }
    if let Some(rule) = matched {
        if rule.action == "reject" {
            return DownstreamDecision::Reject;
        }
        if rule.action == "match" {
            return DownstreamDecision::Match(rule.reply_json.clone());
        }
    }
    if passthrough {
        DownstreamDecision::Passthrough
    } else {
        DownstreamDecision::Reject
    }
}

fn error_reply(echo: &str, message: &str) -> String {
    json!({
        "status": "failed",
        "retcode": 1403,
        "msg": message,
        "wording": message,
        "echo": echo,
        "data": null
    })
    .to_string()
}

fn mock_reply(echo: &str, reply_json: &str) -> String {
    if let Ok(mut m) = serde_json::from_str::<Value>(reply_json) {
        if let Some(obj) = m.as_object_mut()
            && !echo.is_empty()
        {
            obj.insert("echo".into(), Value::String(echo.to_string()));
        }
        return m.to_string();
    }
    reply_json.to_string()
}

impl Plug {
    fn load_rules_from_cfg(&self, cfg: &Value) {
        // event_filter remains a handle()-only legacy path when upstream_rules is empty.
        let ups = parse_rules(cfg, "upstream_rules");
        let downs = parse_rules(cfg, "downstream_rules");
        *self.upstream_rules.write() = ups;
        *self.downstream_rules.write() = downs;
    }

    async fn restart_outbound(&self) {
        self.stop.notify_waiters();
        if let Some(h) = self.outbound.lock().await.take() {
            h.abort();
        }
        *self.send_tx.lock().await = None;

        let cfg = self.cfg.read().clone();
        let url = cfg_url(&cfg);
        if url.is_empty() {
            return;
        }
        let token = cfg_token(&cfg);
        let stop = self.stop.clone();
        let host = self.host.clone();
        let downstream_rules = self.downstream_rules.read().clone();
        let (tx, mut rx) = mpsc::channel::<String>(1024);
        *self.send_tx.lock().await = Some(tx);
        let handle = tokio::spawn(async move {
            let mut delay = std::time::Duration::from_secs(1);
            loop {
                match connect_and_serve(
                    &url,
                    &token,
                    stop.clone(),
                    &mut rx,
                    host.clone(),
                    downstream_rules.clone(),
                )
                .await
                {
                    Ok(()) => {}
                    Err(err) => warn!(error=%err, "onebot ws client loop error; retrying"),
                }
                tokio::select! {
                    _ = stop.notified() => break,
                    _ = tokio::time::sleep(delay) => {}
                }
                delay = (delay * 2).min(std::time::Duration::from_secs(30));
            }
        });
        *self.outbound.lock().await = Some(handle);
    }
}

async fn connect_and_serve(
    url: &str,
    token: &str,
    stop: Arc<tokio::sync::Notify>,
    rx: &mut mpsc::Receiver<String>,
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    downstream_rules: Vec<Rule>,
) -> Result<(), String> {
    let mut req_url = url.to_string();
    if !token.is_empty() && !req_url.contains("access_token=") {
        if req_url.contains('?') {
            req_url.push_str(&format!("&access_token={token}"));
        } else {
            req_url.push_str(&format!("?access_token={token}"));
        }
    }
    let mut req = req_url.into_client_request().map_err(|e| e.to_string())?;
    if !token.is_empty() {
        req.headers_mut().insert(
            http::header::AUTHORIZATION,
            format!("Bearer {token}")
                .parse()
                .map_err(|e: http::header::InvalidHeaderValue| e.to_string())?,
        );
    }
    info!(%url, "connecting to downstream WS");
    let (ws, _) = tokio_tungstenite::connect_async(req)
        .await
        .map_err(|e| e.to_string())?;
    info!("successfully connected to downstream WS");
    let (mut sink, mut stream) = ws.split();
    // reply channel for downstream handling tasks
    let (reply_tx, mut reply_rx) = mpsc::channel::<String>(1024);

    loop {
        tokio::select! {
            _ = stop.notified() => {
                let _ = sink.send(Message::Close(None)).await;
                return Ok(());
            }
            msg = rx.recv() => {
                match msg {
                    Some(payload) => {
                        if sink.send(Message::Text(payload.into())).await.is_err() {
                            return Err("failed to write upstream event".into());
                        }
                    }
                    None => return Ok(()),
                }
            }
            msg = reply_rx.recv() => {
                if let Some(payload) = msg
                    && sink.send(Message::Text(payload.into())).await.is_err()
                {
                    return Err("failed to write downstream reply".into());
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let host = host.clone();
                        let rules = downstream_rules.clone();
                        let reply_tx = reply_tx.clone();
                        tokio::spawn(async move {
                            handle_downstream_message(&text, &rules, host, reply_tx).await;
                        });
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        if let Ok(text) = String::from_utf8(bin.to_vec()) {
                            let host = host.clone();
                            let rules = downstream_rules.clone();
                            let reply_tx = reply_tx.clone();
                            tokio::spawn(async move {
                                handle_downstream_message(&text, &rules, host, reply_tx).await;
                            });
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => return Err("connection closed".into()),
                    Some(Err(err)) => return Err(err.to_string()),
                    _ => {}
                }
            }
        }
    }
}

async fn handle_downstream_message(
    text: &str,
    rules: &[Rule],
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    reply_tx: mpsc::Sender<String>,
) {
    let data: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(err) => {
            warn!(error=%err, "failed to unmarshal downstream payload");
            return;
        }
    };
    let action = data
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let echo = data
        .get("echo")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    let params = data.get("params").cloned().unwrap_or(Value::Null);

    match decide_downstream(rules, &data) {
        DownstreamDecision::Reject => {
            let _ = reply_tx.send(error_reply(&echo, "rejected by rules")).await;
        }
        DownstreamDecision::Match(reply) => {
            let _ = reply_tx.send(mock_reply(&echo, &reply)).await;
        }
        DownstreamDecision::Passthrough => {
            if action.is_empty() {
                warn!("downstream payload missing action; dropping");
                return;
            }
            let host_client = host.read().await.clone();
            let Some(mut host_client) = host_client else {
                let _ = reply_tx
                    .send(error_reply(&echo, "host is unavailable"))
                    .await;
                return;
            };
            match host_client.call_onebot(&action, &params, 0, "").await {
                Ok(mut resp) => {
                    if let Some(obj) = resp.as_object_mut()
                        && !echo.is_empty()
                    {
                        obj.insert("echo".into(), Value::String(echo));
                    }
                    let _ = reply_tx.send(resp.to_string()).await;
                }
                Err(err) => {
                    let _ = reply_tx.send(error_reply(&echo, &err.to_string())).await;
                }
            }
        }
    }
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        self.load_rules_from_cfg(&config);
        *self.cfg.write() = config;
        self.restart_outbound().await;
        Ok(())
    }
    async fn invoke(&self, _: &str, _: Value, _: &str) -> Result<Value, StructuredError> {
        Err(StructuredError::not_found("method is not exported"))
    }
    async fn handle(
        &self,
        _listener_id: &str,
        event_raw: Value,
        _: Option<CommandMatch>,
        _: &str,
    ) -> Result<HandleResult, StructuredError> {
        let cfg = self.cfg.read().clone();
        let ups = self.upstream_rules.read().clone();
        let allow = if ups.is_empty() {
            // Legacy single event_filter: default "$"/empty = allow all.
            let filter = cfg
                .get("event_filter")
                .and_then(|v| v.as_str())
                .unwrap_or("$")
                .trim();
            if filter.is_empty() || filter == "$" {
                true
            } else {
                jsonata::is_truthy_filter(filter, &event_raw)
            }
        } else {
            decide_upstream(&ups, &event_raw)
        };
        if !allow {
            return Ok(HandleResult {});
        }
        let payload = serde_json::to_string(&event_raw).unwrap_or_else(|_| "{}".into());
        if let Some(tx) = self.send_tx.lock().await.as_ref()
            && tx.try_send(payload).is_err()
        {
            warn!("upstream event send queue full; dropping");
        }
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        let url = cfg_url(&self.cfg.read());
        if url.is_empty() {
            Ok("stopped".into())
        } else if self.send_tx.lock().await.is_some() {
            Ok("running".into())
        } else {
            Ok("stopped".into())
        }
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        self.stop.notify_waiters();
        if let Some(h) = self.outbound.lock().await.take() {
            h.abort();
        }
        *self.send_tx.lock().await = None;
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
    let stop = Arc::new(tokio::sync::Notify::new());
    run_plugin_with_host(
        Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(json!({})),
            upstream_rules: RwLock::new(Vec::new()),
            downstream_rules: RwLock::new(Vec::new()),
            outbound: Mutex::new(None),
            stop,
            send_tx: Mutex::new(None),
        }),
        host,
    )
    .await
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn upstream_default_passthrough() {
        assert!(decide_upstream(&[], &json!({"post_type":"message"})));
    }

    #[test]
    fn upstream_reject_rule() {
        let rules = vec![Rule {
            expression: "post_type = 'message'".into(),
            action: "reject".into(),
            reply_json: String::new(),
        }];
        assert!(!decide_upstream(&rules, &json!({"post_type":"message"})));
        assert!(decide_upstream(&rules, &json!({"post_type":"notice"})));
    }

    #[test]
    fn downstream_match_and_reject() {
        let rules = vec![
            Rule {
                expression: "action = 'send_msg'".into(),
                action: "match".into(),
                reply_json: r#"{"status":"ok","retcode":0}"#.into(),
            },
            Rule {
                expression: "action = 'delete_msg'".into(),
                action: "reject".into(),
                reply_json: String::new(),
            },
        ];
        match decide_downstream(&rules, &json!({"action":"send_msg"})) {
            DownstreamDecision::Match(s) => assert!(s.contains("ok")),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(
            decide_downstream(&rules, &json!({"action":"delete_msg"})),
            DownstreamDecision::Reject
        );
        assert_eq!(
            decide_downstream(&rules, &json!({"action":"get_status"})),
            DownstreamDecision::Passthrough
        );
    }

    #[test]
    fn parse_rules_from_config() {
        let cfg = json!({
            "upstream_rules":[{"expression":"post_type = 'message'","action":"passthrough"}],
            "downstream_rules":[{"expression":"action = 'x'","action":"reject","reply_json":"{}"}]
        });
        assert_eq!(parse_rules(&cfg, "upstream_rules").len(), 1);
        assert_eq!(parse_rules(&cfg, "downstream_rules")[0].action, "reject");
    }
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
