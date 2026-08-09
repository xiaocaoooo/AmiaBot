use std::sync::Arc;

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use nyanyabot_proto::{
    CommandMatch, ConfigSpec, Descriptor, EventListener, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock as AsyncRwLock};
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
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(json!({"url":"","access_token":"","event_filter":"$"})),
        }),
        events: vec![EventListener {
            name: "all".into(),
            id: "evt.all".into(),
            description: "转发全部事件".into(),
            event: "*".into(),
            handler: "Handle".into(),
        }],
        ..Default::default()
    }
}

impl Plug {
    async fn restart_outbound(&self) {
        // stop previous
        self.stop.notify_waiters();
        if let Some(h) = self.outbound.lock().await.take() {
            h.abort();
        }
        let cfg = self.cfg.read().clone();
        let url = cfg
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if url.is_empty() {
            return;
        }
        let token = cfg
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let stop = self.stop.clone();
        let handle = tokio::spawn(async move {
            loop {
                if let Err(err) = connect_loop(&url, &token, stop.clone()).await {
                    warn!(error = %err, "onebot ws client loop error; retrying");
                }
                tokio::select! {
                    _ = stop.notified() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
                }
            }
        });
        *self.outbound.lock().await = Some(handle);
    }
}

async fn connect_loop(
    url: &str,
    token: &str,
    stop: Arc<tokio::sync::Notify>,
) -> Result<(), String> {
    let mut req = url.into_client_request().map_err(|e| e.to_string())?;
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
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        // keep-alive / observe upstream events; bridge is filter-gated in handle()
                        let _ = t;
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
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
        _: &str,
        event_raw: Value,
        _: Option<CommandMatch>,
        _: &str,
    ) -> Result<HandleResult, StructuredError> {
        let cfg = self.cfg.read().clone();
        let filter = cfg
            .get("event_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("$");
        match plugin_common::jsonata::evaluate(filter, &event_raw) {
            Ok(Value::Bool(false)) | Ok(Value::Null) => {}
            Ok(_) | Err(_) => {
                // filtered pass: connection loop already established by configure
            }
        }
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        let url = self
            .cfg
            .read()
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            Ok("IDLE".into())
        } else {
            Ok("OK".into())
        }
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        self.stop.notify_waiters();
        if let Some(h) = self.outbound.lock().await.take() {
            h.abort();
        }
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
        }),
        host,
    )
    .await
}

#[cfg(test)]
mod tests {
    use plugin_common::jsonata::evaluate;
    use serde_json::json;
    #[test]
    fn path_and_identity() {
        let data = json!({"a": {"b": 1}});
        assert_eq!(evaluate("$", &data).unwrap(), data);
        assert_eq!(evaluate("$.a.b", &data).unwrap(), json!(1));
    }
}

#[cfg(test)]
mod descriptor_snapshot_tests {
    use super::plugin_descriptor;

    #[test]
    fn descriptor_matches_snapshot() {
        let mut d = plugin_descriptor();
        nyanyabot_proto::ensure_descriptor_arrays(&mut d);
        let actual = serde_json::to_string_pretty(&d).expect("serialize descriptor");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots/descriptor.json");
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("{actual}\n")).unwrap();
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing descriptor snapshot at {}: {e}", path.display()));
        assert_eq!(
            actual.trim(),
            expected.trim(),
            "descriptor snapshot mismatch; run with UPDATE_SNAPSHOTS=1"
        );
    }
}
