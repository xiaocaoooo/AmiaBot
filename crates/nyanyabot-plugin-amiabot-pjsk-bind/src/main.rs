use async_trait::async_trait;
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::build_pages_url;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

struct Plug {
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot PJSK Bind".into(),
        plugin_id: "external.amiabot-pjsk-bind".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "PJSK 账户绑定插件，提供绑定和查询 ID 的功能".into(),
        dependencies: vec!["external.amiabot-pjsk-account".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({
                "type":"object",
                "additionalProperties": false,
                "properties":{
                    "amiabot_pages":{
                        "type":"string",
                        "description":"Amiabot Pages 域名/地址；用于获取 profile 用户名"
                    }
                }
            })),
            default: Some(json!({"amiabot_pages":""})),
        }),
        commands: vec![
            CommandListener {
                name: "pjsk-bind".into(),
                id: "cmd.profile-bind".into(),
                description: "绑定 PJSK 游戏账号（如 绑定12345, jp绑定12345）".into(),
                pattern: r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?绑定(?P<id>\d+)$".into(),
                match_raw: false,
                handler: "Handle".into(),
            },
            CommandListener {
                name: "pjsk-id".into(),
                id: "cmd.profile-id".into(),
                description: "查询已绑定的游戏 ID（如 id, jpid, 烤id）".into(),
                pattern: r"^(?:(?P<server>cn|jp|tw|en|kr)|(?:烤))id$".into(),
                match_raw: false,
                handler: "Handle".into(),
            },
            CommandListener {
                name: "pjsk-set-default-server".into(),
                id: "cmd.set-default-server".into(),
                description: "设置默认服务器（如 serverjp, 默认服务器cn）".into(),
                pattern: r"^(?:默认服务器|server)(?P<server>cn|jp|tw|en|kr)$".into(),
                match_raw: false,
                handler: "Handle".into(),
            },
        ],
        ..Default::default()
    }
}

fn valid_server(s: &str) -> bool {
    matches!(s, "jp" | "cn" | "en" | "tw" | "kr")
}

async fn fetch_profile_name(
    pages_host: &str,
    server: &str,
    game_id: &str,
) -> Result<String, String> {
    let pages_host = pages_host.trim();
    if pages_host.is_empty() {
        return Err("pages 地址未配置".into());
    }
    let mut q = BTreeMap::new();
    q.insert("server".into(), server.to_string());
    q.insert("id".into(), game_id.to_string());
    let url = build_pages_url(pages_host, "/pjsk/profile/raw", &q);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建请求失败: {e}"))?;
    let resp = client
        .get(&url)
        .header("User-Agent", "amiabot-pjsk-bind/1.0")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("请求 pages 失败: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "pages 返回 {}: {}",
            status.as_u16(),
            shorten_error_body(&body)
        ));
    }
    let value: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 pages 返回失败: {e}"))?;
    let username = value
        .pointer("/user/name")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if username.is_empty() {
        return Err("未获取到用户名".into());
    }
    Ok(username)
}

fn shorten_error_body(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return "空响应".into();
    }
    if text.chars().count() > 240 {
        let clipped: String = text.chars().take(240).collect();
        format!("{clipped}...")
    } else {
        text
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
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "jp".into());
                let id = groups.get(1).cloned().unwrap_or_default();
                if !valid_server(&server) || id.trim().is_empty() {
                    let _ = plugin_common::send_text(
                        &mut host,
                        &event_raw,
                        "❌ 参数不正确，请发送 绑定+你的游戏ID，如 jp绑定12345",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
                match host
                    .call_dependency(
                        "external.amiabot-pjsk-account",
                        "account.add",
                        &json!({"qq_id": qq, "game_server": server, "game_id": id}),
                    )
                    .await
                {
                    Ok(resp) => {
                        let ok = resp
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        if !ok {
                            let msg = resp
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("绑定失败");
                            let _ = plugin_common::send_text(
                                &mut host,
                                &event_raw,
                                &format!("❌ {msg}"),
                                trace_id,
                            )
                            .await;
                            return Ok(HandleResult {});
                        }
                        // Auto-set preferred server if unset (Go parity).
                        if let Ok(pref) = host
                            .call_dependency(
                                "external.amiabot-pjsk-account",
                                "account.get_preferred_server",
                                &json!({"qq_id": qq}),
                            )
                            .await
                        {
                            let cur = pref
                                .get("server")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            if cur.is_empty() {
                                let _ = host
                                    .call_dependency(
                                        "external.amiabot-pjsk-account",
                                        "account.set_preferred_server",
                                        &json!({"qq_id": qq, "server": server}),
                                    )
                                    .await;
                            }
                        }
                        let pages = self
                            .cfg
                            .read()
                            .get("amiabot_pages")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let msg = match fetch_profile_name(&pages, &server, &id).await {
                            Ok(name) => {
                                format!("绑定成功！\n[{}] {}", server.to_uppercase(), name)
                            }
                            Err(err) => format!(
                                "绑定成功！\n[{}]\n\n获取用户名失败: {}",
                                server.to_uppercase(),
                                err
                            ),
                        };
                        let _ =
                            plugin_common::send_text(&mut host, &event_raw, &msg, trace_id).await;
                    }
                    Err(err) => {
                        let _ = plugin_common::send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 绑定失败: {err}"),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            "cmd.profile-id" => {
                let server = groups
                    .first()
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty() && s.as_str() != "烤");
                match host
                    .call_dependency(
                        "external.amiabot-pjsk-account",
                        "account.list_by_qq",
                        &json!({"qq_id": qq, "enabled_only": true}),
                    )
                    .await
                {
                    Ok(v) => {
                        let arr = v
                            .get("accounts")
                            .and_then(|a| a.as_array())
                            .cloned()
                            .unwrap_or_default();
                        if let Some(server) = server {
                            if let Some(a) = arr.iter().find(|a| {
                                a.get("game_server")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                                    .eq_ignore_ascii_case(&server)
                            }) {
                                let gid = a.get("game_id").and_then(|x| x.as_str()).unwrap_or("?");
                                let _ = plugin_common::send_text(
                                    &mut host,
                                    &event_raw,
                                    &format!("[{}] {gid}", server.to_uppercase()),
                                    trace_id,
                                )
                                .await;
                            } else {
                                let _ = plugin_common::send_text(
                                    &mut host,
                                    &event_raw,
                                    &format!("未绑定 [{}] 账户", server.to_uppercase()),
                                    trace_id,
                                )
                                .await;
                            }
                        } else {
                            let mut lines = vec![];
                            for a in &arr {
                                lines.push(format!(
                                    "[{}] {}",
                                    a.get("game_server")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("?")
                                        .to_uppercase(),
                                    a.get("game_id").and_then(|s| s.as_str()).unwrap_or("?")
                                ));
                            }
                            let msg = if lines.is_empty() {
                                "未绑定账户".into()
                            } else {
                                lines.join("\n")
                            };
                            let _ = plugin_common::send_text(&mut host, &event_raw, &msg, trace_id)
                                .await;
                        }
                    }
                    Err(err) => {
                        let _ = plugin_common::send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 查询失败: {err}"),
                            trace_id,
                        )
                        .await;
                    }
                }
            }
            "cmd.set-default-server" => {
                let server = groups
                    .first()
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default();
                if !valid_server(&server) {
                    let _ = plugin_common::send_text(
                        &mut host,
                        &event_raw,
                        "❌ 无效的服务器，请选择 jp/cn/en/tw/kr",
                        trace_id,
                    )
                    .await;
                    return Ok(HandleResult {});
                }
                match host
                    .call_dependency(
                        "external.amiabot-pjsk-account",
                        "account.set_preferred_server",
                        &json!({"qq_id": qq, "server": server}),
                    )
                    .await
                {
                    Ok(resp) => {
                        let ok = resp
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let text = if ok {
                            format!(
                                "默认服务器已设置为 [{}]\n可以使用\"个人信息\"或\"profile\"查看 profile",
                                server.to_uppercase()
                            )
                        } else {
                            let msg = resp
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("设置默认服务器失败");
                            format!("❌ {msg}")
                        };
                        let _ =
                            plugin_common::send_text(&mut host, &event_raw, &text, trace_id).await;
                    }
                    Err(err) => {
                        let _ = plugin_common::send_text(
                            &mut host,
                            &event_raw,
                            &format!("❌ 设置默认服务器失败: {err}"),
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
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
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

#[cfg(test)]
mod unit_tests {
    use super::shorten_error_body;

    #[test]
    fn shortens_error_body() {
        assert_eq!(shorten_error_body("  a\n b  "), "a b");
        let long = "x".repeat(300);
        let out = shorten_error_body(&long);
        assert!(out.ends_with("..."));
        assert!(out.chars().count() <= 243);
    }
}
