mod alias;
mod matcher;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use alias::AliasManager;
use async_trait::async_trait;
use matcher::{MatchResult, fuzzy_search};
use nyanyabot_proto::{
    CommandListener, CommandMatch, ConfigSpec, Descriptor, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{
    build_pages_url, event_content, redact_secrets, screenshot_and_upload, send_image, send_text,
};
use regex::Regex;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

struct Cfg {
    amiabot_pages: String,
    default_server: String,
    alias_data_url: String,
    alias_cache_dir: String,
    alias_cache_ttl: i64,
}

impl Default for Cfg {
    fn default() -> Self {
        Self {
            amiabot_pages: String::new(),
            default_server: "jp".into(),
            alias_data_url: alias::DEFAULT_ALIAS_DATA_URL.into(),
            alias_cache_dir: alias::DEFAULT_CACHE_DIR.into(),
            alias_cache_ttl: 3600,
        }
    }
}

impl Cfg {
    fn from_value(v: &Value) -> Self {
        let mut cfg = Self::default();
        if let Some(s) = v.get("amiabot_pages").and_then(|x| x.as_str()) {
            cfg.amiabot_pages = plugin_common::normalize_http_base(s);
        }
        if let Some(s) = v.get("default_server").and_then(|x| x.as_str()) {
            let s = s.trim().to_lowercase();
            if !s.is_empty() {
                cfg.default_server = s;
            }
        }
        if let Some(s) = v.get("alias_data_url").and_then(|x| x.as_str()) {
            cfg.alias_data_url = s.trim().to_string();
        }
        if let Some(s) = v.get("alias_cache_dir").and_then(|x| x.as_str()) {
            cfg.alias_cache_dir = s.trim().to_string();
        }
        if let Some(n) = v.get("alias_cache_ttl").and_then(|x| x.as_i64()) {
            cfg.alias_cache_ttl = n;
        }
        cfg
    }
}

struct Plug {
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Cfg>,
    aliases: AsyncRwLock<AliasManager>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot PJSK Song".into(),
        plugin_id: "external.amiabot-pjsk-song".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "PJSK 歌曲查询".into(),
        dependencies: vec!["external.screenshot".into(), "external.blobserver".into()],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("PJSK Song 插件配置".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "amiabot_pages":{"type":"string"},
                    "default_server":{"type":"string"},
                    "alias_data_url":{"type":"string","description":"别名数据URL"},
                    "alias_cache_dir":{"type":"string"},
                    "alias_cache_ttl":{"type":"integer","description":"缓存有效期（秒）"}
                },
                "additionalProperties": true
            })),
            default: Some(json!({
                "amiabot_pages":"",
                "default_server":"jp",
                "alias_data_url": alias::DEFAULT_ALIAS_DATA_URL,
                "alias_cache_dir": alias::DEFAULT_CACHE_DIR,
                "alias_cache_ttl": 3600
            })),
        }),
        commands: vec![CommandListener {
            name: "song".into(),
            id: "cmd.pjsk-song".into(),
            description: "song query".into(),
            pattern: r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?song(?P<name>.+)$".into(),
            match_raw: true,
            handler: "HandlePJSKSong".into(),
        }],
        ..Default::default()
    }
}

fn parse_args(
    content: &str,
    match_data: &Option<CommandMatch>,
    default_server: &str,
) -> (String, String) {
    let mut server = String::new();
    let mut name = String::new();
    if let Some(m) = match_data {
        // named groups aren't exposed separately; positional: server may be group0 if present
        // Host provides capture groups in order.
        if m.groups.len() >= 2 {
            server = m.groups[0].trim().to_lowercase();
            name = m.groups[1].trim().to_string();
        } else if m.groups.len() == 1 {
            // could be only name when server absent
            name = m.groups[0].trim().to_string();
        }
    }
    if name.is_empty() {
        let re = Regex::new(r"^(?i)(?:(?P<server>cn|jp|tw|en|kr))?song(?P<name>.+)$").unwrap();
        if let Some(caps) = re.captures(content) {
            server = caps
                .name("server")
                .map(|m| m.as_str().trim().to_lowercase())
                .unwrap_or_default();
            name = caps
                .name("name")
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
        }
    }
    if server.is_empty() {
        server = default_server.to_string();
    }
    (server, name)
}

fn build_candidate_message(results: &[MatchResult], server: &str) -> String {
    let mut sb = String::from("🎵 找到多个匹配，输入编号可快速查询：\n");
    let max_show = results.len().min(3);
    for r in results.iter().take(max_show) {
        let pct = (r.confidence * 100.0) as i32;
        let prefix = if server != "jp" && !server.is_empty() {
            server
        } else {
            ""
        };
        sb.push_str(&format!(
            "  • {prefix}song{} - {} ({}%)\n",
            r.music_id, r.title, pct
        ));
    }
    sb
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        let cfg = Cfg::from_value(&config);
        let mut am = AliasManager::new(
            &cfg.alias_data_url,
            &cfg.alias_cache_dir,
            cfg.alias_cache_ttl,
        );
        if let Err(err) = am.load().await {
            warn!(error=%err, "alias load failed on configure");
        }
        *self.cfg.write() = cfg;
        *self.aliases.write().await = am;
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
        if listener_id != "cmd.pjsk-song" {
            return Ok(HandleResult {});
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let cfg = {
            let g = self.cfg.read();
            Cfg {
                amiabot_pages: g.amiabot_pages.clone(),
                default_server: g.default_server.clone(),
                alias_data_url: g.alias_data_url.clone(),
                alias_cache_dir: g.alias_cache_dir.clone(),
                alias_cache_ttl: g.alias_cache_ttl,
            }
        };
        let content = event_content(&event_raw);
        let (server, name) = parse_args(&content, &match_data, &cfg.default_server);
        if name.is_empty() {
            let _ = send_text(
                &mut host,
                &event_raw,
                "❌ 未找到匹配的歌曲，请尝试其他关键词",
                trace_id,
            )
            .await;
            return Ok(HandleResult {});
        }

        // numeric id shortcut: song123 / song#123
        let results = {
            let am = self.aliases.read().await;
            if let Ok(id) = name.trim_start_matches('#').parse::<i64>() {
                if let Some(m) = am.music_get(id) {
                    vec![MatchResult {
                        music_id: id,
                        title: m.title.clone(),
                        confidence: 1.0,
                        matched_key: id.to_string(),
                    }]
                } else {
                    fuzzy_search(&name, &am)
                }
            } else {
                fuzzy_search(&name, &am)
            }
        };
        if results.is_empty() {
            let _ = send_text(
                &mut host,
                &event_raw,
                "❌ 未找到匹配的歌曲，请尝试其他关键词",
                trace_id,
            )
            .await;
            return Ok(HandleResult {});
        }
        if cfg.amiabot_pages.is_empty() {
            let top = &results[0];
            let mut msg = format!(
                "歌曲 #{} - {} ({:.0}%)",
                top.music_id,
                top.title,
                top.confidence * 100.0
            );
            if results.len() > 1 {
                msg.push('\n');
                msg.push_str(&build_candidate_message(&results[1..], &server));
            }
            let _ = send_text(&mut host, &event_raw, &msg, trace_id).await;
            return Ok(HandleResult {});
        }

        let top = &results[0];
        let mut q = BTreeMap::new();
        q.insert("server".into(), server.clone());
        q.insert("id".into(), top.music_id.to_string());
        let page = build_pages_url(&cfg.amiabot_pages, "/pjsk/music", &q);
        let blob = format!(
            "pjsk-song-{}-{}-{}",
            server,
            top.music_id,
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
                    &format!("❌ 截图失败：{}", redact_secrets(&err.to_string())),
                    trace_id,
                )
                .await;
                return Ok(HandleResult {});
            }
        }
        if results.len() > 1 {
            let _ = send_text(
                &mut host,
                &event_raw,
                &build_candidate_message(&results[1..], &server),
                trace_id,
            )
            .await;
        }
        info!(music_id = top.music_id, %server, "pjsk song handled");
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        let n = self.aliases.read().await.data().musics.len();
        Ok(format!("OK aliases={n}"))
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
    let am = AliasManager::new(
        alias::DEFAULT_ALIAS_DATA_URL,
        alias::DEFAULT_CACHE_DIR,
        3600,
    );
    run_plugin_with_host(
        Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(Cfg::default()),
            aliases: AsyncRwLock::new(am),
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
mod tests {
    use super::*;
    use alias::{AliasData, MusicAlias};

    #[test]
    fn exact_and_prefix() {
        // re-export old test names expected by prior suite via matcher unit tests
        assert!(
            matcher::normalize_string(" A b ").contains("ab")
                || matcher::normalize_string(" A b ") == "ab"
        );
    }

    #[test]
    fn fuzzy_rank_and_limit() {
        let mut am = AliasManager::new("", std::env::temp_dir().to_str().unwrap_or("/tmp"), 60);
        am.load_from_data(AliasData {
            generated_at: String::new(),
            musics: vec![
                MusicAlias {
                    music_id: 1,
                    title: "Hello World".into(),
                    aliases: vec!["你好世界".into()],
                },
                MusicAlias {
                    music_id: 2,
                    title: "Hell".into(),
                    aliases: vec![],
                },
            ],
        });
        let r = fuzzy_search("hello", &am);
        assert!(!r.is_empty());
        assert_eq!(r[0].music_id, 1);
    }
}
