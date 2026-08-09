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

mod matcher {
    pub fn normalize(s: &str) -> String {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(|c| c.to_lowercase())
            .collect()
    }
    pub fn score(query: &str, title: &str) -> f64 {
        let q = normalize(query);
        let t = normalize(title);
        if q.is_empty() || t.is_empty() {
            return 0.0;
        }
        if q == t {
            return 1.0;
        }
        if t.starts_with(&q) {
            return 0.8;
        }
        if t.contains(&q) {
            return 0.7;
        }
        // subsequence
        let mut ti = t.chars().peekable();
        for qc in q.chars() {
            loop {
                match ti.next() {
                    Some(tc) if tc == qc => break,
                    Some(_) => continue,
                    None => return 0.0,
                }
            }
        }
        0.5
    }
    pub fn best_matches<'a>(query: &str, titles: &[&'a str], limit: usize) -> Vec<(&'a str, f64)> {
        let mut scored: Vec<_> = titles
            .iter()
            .map(|t| (*t, score(query, t)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(limit).collect()
    }
}

struct Plug {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
    titles: RwLock<Vec<String>>,
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
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(
                json!({"amiabot_pages":"","default_server":"jp","alias_data_url":"","alias_cache_dir":"","alias_cache_ttl":86400}),
            ),
        }),
        commands: vec![CommandListener {
            name: "song".into(),
            id: "cmd.pjsk-song".into(),
            description: "song query".into(),
            pattern: r"(?i)^(?:(?P<server>cn|jp|tw|en|kr))?song(?P<name>.+)$".into(),
            match_raw: true,
            handler: "Handle".into(),
        }],
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
        if listener_id != "cmd.pjsk-song" {
            return Ok(HandleResult {});
        }
        let host = self.host.read().await.clone();
        let Some(mut host) = host else {
            return Ok(HandleResult {});
        };
        let cfg = self.cfg.read().clone();
        let pages = cfg
            .get("amiabot_pages")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let default_server = cfg
            .get("default_server")
            .and_then(|v| v.as_str())
            .unwrap_or("jp");
        let groups = match_data.map(|m| m.groups).unwrap_or_default();
        let server = groups
            .first()
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(default_server);
        let name = groups
            .get(1)
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();
        if pages.is_empty() {
            let _ =
                plugin_common::send_text(&mut host, &event_raw, "amiabot_pages 未配置", trace_id)
                    .await;
            return Ok(HandleResult {});
        }
        let titles = self.titles.read().clone();
        if !titles.is_empty() {
            let refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
            let matches = matcher::best_matches(&name, &refs, 5);
            if matches.len() > 1 && matches[0].1 < 1.0 {
                let list = matches
                    .iter()
                    .map(|(t, s)| format!("- {t} ({s:.2})"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let _ = plugin_common::send_text(
                    &mut host,
                    &event_raw,
                    &format!("多个匹配:\n{list}"),
                    trace_id,
                )
                .await;
                return Ok(HandleResult {});
            }
        }
        let page_url = plugin_common::join_url(&pages, &format!("/pjsk/song/{server}/{}", name));
        match plugin_common::screenshot_and_upload(&mut host, &page_url, "pjsk-song", json!({}))
            .await
        {
            Ok(url) => {
                let _ = plugin_common::send_image(&mut host, &event_raw, &url, trace_id).await;
            }
            Err(err) => {
                let _ = plugin_common::send_text(
                    &mut host,
                    &event_raw,
                    &format!("失败: {err}"),
                    trace_id,
                )
                .await;
            }
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
            titles: RwLock::new(vec![]),
        }),
        host,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::matcher::*;
    #[test]
    fn exact_and_prefix() {
        assert_eq!(score("foo", "foo"), 1.0);
        assert!(score("fo", "foo") >= 0.8);
        assert!(score("oo", "foo") >= 0.7);
        assert!(score("fo", "xfoo") >= 0.5);
    }

    #[test]
    fn fuzzy_rank_and_limit() {
        let titles = ["夜に駆ける", "夜叉", "夜行"];
        let got = best_matches("夜", &titles, 2);
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|(t, s)| t.contains('夜') && *s > 0.0));
        let exact = best_matches("夜叉", &titles, 5);
        assert_eq!(exact[0].0, "夜叉");
        assert_eq!(exact[0].1, 1.0);
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
