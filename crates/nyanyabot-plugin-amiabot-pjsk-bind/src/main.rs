use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

struct Plug {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
}
fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot PJSK Bind".into(),
        plugin_id: "external.amiabot-pjsk-bind".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "PJSK 账户绑定".into(),
        dependencies: vec!["external.amiabot-pjsk-account".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(json!({"amiabot_pages":""})),
        }),
        commands: vec![
            CommandListener {
                name: "bind".into(),
                id: "cmd.profile-bind".into(),
                description: "绑定".into(),
                pattern: r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?绑定(?P<id>\d+)$".into(),
                match_raw: true,
                handler: "Handle".into(),
            },
            CommandListener {
                name: "id".into(),
                id: "cmd.profile-id".into(),
                description: "查询id".into(),
                pattern: r"^(?:(?P<server>cn|jp|tw|en|kr)|(?:烤))id$".into(),
                match_raw: true,
                handler: "Handle".into(),
            },
            CommandListener {
                name: "default-server".into(),
                id: "cmd.set-default-server".into(),
                description: "默认区服".into(),
                pattern: r"^(?:默认服务器|server)(?P<server>cn|jp|tw|en|kr)$".into(),
                match_raw: true,
                handler: "Handle".into(),
            },
        ],
        ..Default::default()
    }
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        *self.cfg.write() = config;
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
        let qq = plugin_common::event_user_id(&event_raw);
        let groups = match_data.map(|m| m.groups).unwrap_or_default();
        match listener_id {
            "cmd.profile-bind" => {
                let server = groups
                    .first()
                    .map(|s| s.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("jp");
                let id = groups.get(1).cloned().unwrap_or_default();
                match host
                    .call_dependency(
                        "external.amiabot-pjsk-account",
                        "account.add",
                        &json!({"qq_id": qq, "game_server": server, "game_id": id}),
                    )
                    .await
                {
                    Ok(_) => {
                        let _ = plugin_common::send_text(
                            &mut host,
                            &event_raw,
                            &format!("绑定成功！\n[{}] {id}", server.to_uppercase()),
                            trace_id,
                        )
                        .await;
                    }
                    Err(err) => {
                        let _ = plugin_common::send_text(
                            &mut host,
                            &event_raw,
                            &format!("绑定失败: {err}"),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            "cmd.profile-id" => {
                let server = groups
                    .first()
                    .map(|s| s.as_str())
                    .filter(|s| !s.is_empty() && *s != "烤");
                if let Some(server) = server {
                    match host
                        .call_dependency(
                            "external.amiabot-pjsk-account",
                            "account.get",
                            &json!({"qq_id": qq, "game_server": server}),
                        )
                        .await
                    {
                        Ok(v) => {
                            let gid = v.get("game_id").and_then(|x| x.as_str()).unwrap_or("?");
                            let _ = plugin_common::send_text(
                                &mut host,
                                &event_raw,
                                &format!("[{}] {gid}", server.to_uppercase()),
                                trace_id,
                            )
                            .await;
                        }
                        Err(err) => {
                            let _ = plugin_common::send_text(
                                &mut host,
                                &event_raw,
                                &format!("查询失败: {err}"),
                                trace_id,
                            )
                            .await;
                        }
                    }
                } else {
                    match host
                        .call_dependency(
                            "external.amiabot-pjsk-account",
                            "account.list",
                            &json!({"qq_id": qq}),
                        )
                        .await
                    {
                        Ok(v) => {
                            let mut lines = vec![];
                            if let Some(arr) = v.get("accounts").and_then(|a| a.as_array()) {
                                for a in arr {
                                    if a.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false) {
                                        lines.push(format!(
                                            "[{}] {}",
                                            a.get("game_server")
                                                .and_then(|s| s.as_str())
                                                .unwrap_or("?")
                                                .to_uppercase(),
                                            a.get("game_id")
                                                .and_then(|s| s.as_str())
                                                .unwrap_or("?")
                                        ));
                                    }
                                }
                            }
                            let msg = if lines.is_empty() {
                                "未绑定账户".into()
                            } else {
                                lines.join("\n")
                            };
                            let _ = plugin_common::send_text(&mut host, &event_raw, &msg, trace_id)
                                .await;
                        }
                        Err(err) => {
                            let _ = plugin_common::send_text(
                                &mut host,
                                &event_raw,
                                &format!("查询失败: {err}"),
                                trace_id,
                            )
                            .await;
                        }
                    }
                }
            }
            "cmd.set-default-server" => {
                let server = groups.first().cloned().unwrap_or_else(|| "jp".into());
                let _ = plugin_common::send_text(
                    &mut host,
                    &event_raw,
                    &format!("默认区服已设置为 {server}"),
                    trace_id,
                )
                .await;
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
            cfg: RwLock::new(json!({})),
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
