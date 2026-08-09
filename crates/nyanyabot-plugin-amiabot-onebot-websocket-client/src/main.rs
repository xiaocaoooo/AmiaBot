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

struct Plug {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
    outbound: Mutex<Option<tokio::task::JoinHandle<()>>>,
    stop: Arc<tokio::sync::Notify>,
    send_tx: Mutex<Option<mpsc::Sender<String>>>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "OneBot WebSocket Client".into(),
        plugin_id: "external.amiabot-onebot-websocket-client".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "正向 OneBot WebSocket 客户端桥接".into(),
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("OneBot WS Client 配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "url":{"type":"string","description":"下游 OneBot WS URL（兼容字段）"},
                    "ws_url":{"type":"string","description":"下游 OneBot WS URL"},
                    "access_token":{"type":"string"},
                    "ws_token":{"type":"string"},
                    "event_filter":{"type":"string","description":"JSONata 子集过滤，默认 $ 全通"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({
                "url":"",
                "ws_url":"",
                "access_token":"",
                "ws_token":"",
                "event_filter":"$"
            })),
        }),
        events: vec![
            EventListener {
                name: "onebot-ws-message".into(),
                id: "evt.onebot-ws.message".into(),
                description: "转发 message 事件".into(),
                event: "message".into(),
                handler: "Handle".into(),
            },
            EventListener {
                name: "onebot-ws-notice".into(),
                id: "evt.onebot-ws.notice".into(),
                description: "转发 notice 事件".into(),
                event: "notice".into(),
                handler: "Handle".into(),
            },
            EventListener {
                name: "onebot-ws-request".into(),
                id: "evt.onebot-ws.request".into(),
                description: "转发 request 事件".into(),
                event: "request".into(),
                handler: "Handle".into(),
            },
            EventListener {
                name: "onebot-ws-meta".into(),
                id: "evt.onebot-ws.meta_event".into(),
                description: "转发 meta_event 事件".into(),
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

impl Plug {
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
        let (tx, mut rx) = mpsc::channel::<String>(1024);
        *self.send_tx.lock().await = Some(tx);
        let url2 = url;
        let token2 = token;
        let handle = tokio::spawn(async move {
            let mut delay = std::time::Duration::from_secs(1);
            loop {
                match connect_and_serve(&url2, &token2, stop.clone(), &mut rx).await {
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
            "Authorization",
            http::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| e.to_string())?,
        );
    }
    let (ws, _) = tokio_tungstenite::connect_async(req)
        .await
        .map_err(|e| e.to_string())?;
    info!(%url, "onebot forward ws connected");
    let (mut sink, mut stream) = ws.split();
    loop {
        tokio::select! {
            _ = stop.notified() => {
                let _ = sink.close().await;
                break;
            }
            maybe = rx.recv() => {
                match maybe {
                    Some(payload) => {
                        if sink.send(Message::Text(payload.into())).await.is_err() {
                            return Err("write failed".into());
                        }
                    }
                    None => break,
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(_))) => {
                        // downstream events ignored in simplified port (Go has downstream rules)
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
    Ok(())
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
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
        let filter = self
            .cfg
            .read()
            .get("event_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("$")
            .to_string();
        if !jsonata::is_truthy_filter(&filter, &event_raw) {
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
            outbound: Mutex::new(None),
            stop,
            send_tx: Mutex::new(None),
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
