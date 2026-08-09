use std::sync::Arc;

use async_trait::async_trait;
use nyanyabot_proto::{
    CommandMatch, ConfigSpec, Descriptor, ExportSpec, HandleResult, HostClient, Plugin,
    StructuredError,
};
use parking_lot::RwLock;
use plugin_common::normalize_http_base;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;
use url::Url;

#[derive(Default, Deserialize)]
struct Cfg {
    #[serde(default)]
    screenshot_server: String,
}

#[derive(Default, Deserialize)]
struct BuildURLParams {
    #[serde(default)]
    page_url: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    selector: String,
    width: Option<i64>,
    height: Option<i64>,
    #[serde(default)]
    format: String,
    quality: Option<i64>,
    wait_time: Option<i64>,
    #[serde(default)]
    wait_for: String,
    full_page: Option<bool>,
    #[serde(default)]
    headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    user_agent: String,
    device_scale: Option<f64>,
    mobile: Option<bool>,
    landscape: Option<bool>,
    timeout: Option<i64>,
    transparent: Option<bool>,
}

struct ScreenshotPlugin {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Cfg>,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Screenshot".into(),
        plugin_id: "external.screenshot".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "Screenshot URL builder plugin".into(),
        exports: vec![ExportSpec {
            name: "screenshot.build_url".into(),
            description: "根据 page_url 生成截图服务 URL".into(),
            params_schema: json!({"type":"object"}),
            result_schema: json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}),
        }],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Screenshot plugin config".into()),
            schema: Some(
                json!({"type":"object","properties":{"screenshot_server":{"type":"string"}}}),
            ),
            default: Some(json!({"screenshot_server":""})),
        }),
        ..Default::default()
    }
}

#[async_trait]
impl Plugin for ScreenshotPlugin {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        let parsed: Cfg = serde_json::from_value(config).unwrap_or_default();
        self.cfg.write().screenshot_server = parsed.screenshot_server.trim().to_string();
        Ok(())
    }
    async fn invoke(
        &self,
        method: &str,
        params: Value,
        _caller: &str,
    ) -> Result<Value, StructuredError> {
        if method != "screenshot.build_url" {
            return Err(StructuredError::not_found("method is not exported"));
        }
        let mut req: BuildURLParams = serde_json::from_value(params)
            .map_err(|e| StructuredError::invalid_params(e.to_string()))?;
        req.page_url = req.page_url.trim().to_string();
        req.url = req.url.trim().to_string();
        if req.page_url.is_empty() {
            req.page_url = req.url.clone();
        }
        if req.page_url.is_empty() {
            return Err(StructuredError::invalid_params("page_url is required"));
        }
        let server = self.cfg.read().screenshot_server.clone();
        if server.trim().is_empty() {
            return Err(StructuredError::internal(
                "screenshot_server is not configured",
            ));
        }
        let built = build_screenshot_url(&server, &req).map_err(StructuredError::invalid_params)?;
        Ok(json!({"url": built}))
    }
    async fn handle(
        &self,
        _: &str,
        _: Value,
        _: Option<CommandMatch>,
        _: &str,
    ) -> Result<HandleResult, StructuredError> {
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        Ok("OK".into())
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        Ok(())
    }
}

fn build_screenshot_url(server: &str, req: &BuildURLParams) -> Result<String, String> {
    let base = normalize_http_base(server);
    let mut u = Url::parse(&base).map_err(|e| e.to_string())?;
    let path = format!("{}/screenshot", u.path().trim_end_matches('/'));
    u.set_path(&path);
    {
        let mut q = u.query_pairs_mut();
        q.append_pair("url", &req.page_url);
        if !req.selector.trim().is_empty() {
            q.append_pair("selector", req.selector.trim());
        }
        if let Some(v) = req.width {
            q.append_pair("width", &v.to_string());
        }
        if let Some(v) = req.height {
            q.append_pair("height", &v.to_string());
        }
        if !req.format.trim().is_empty() {
            q.append_pair("format", req.format.trim());
        }
        if let Some(v) = req.quality {
            q.append_pair("quality", &v.to_string());
        }
        if let Some(v) = req.wait_time {
            q.append_pair("wait_time", &v.to_string());
        }
        if !req.wait_for.trim().is_empty() {
            q.append_pair("wait_for", req.wait_for.trim());
        }
        if let Some(v) = req.full_page {
            q.append_pair("full_page", if v { "true" } else { "false" });
        }
        if !req.headers.is_empty() {
            q.append_pair(
                "headers",
                &serde_json::to_string(&req.headers).unwrap_or_default(),
            );
        }
        if !req.user_agent.trim().is_empty() {
            q.append_pair("user_agent", req.user_agent.trim());
        }
        if let Some(v) = req.device_scale {
            q.append_pair("device_scale", &v.to_string());
        }
        if let Some(v) = req.mobile {
            q.append_pair("mobile", if v { "true" } else { "false" });
        }
        if let Some(v) = req.landscape {
            q.append_pair("landscape", if v { "true" } else { "false" });
        }
        if let Some(v) = req.timeout {
            q.append_pair("timeout", &v.to_string());
        }
        if let Some(v) = req.transparent {
            q.append_pair("transparent", if v { "true" } else { "false" });
        }
    }
    Ok(u.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();
    let host = Arc::new(AsyncRwLock::new(None));
    let plugin = Arc::new(ScreenshotPlugin {
        host: host.clone(),
        cfg: RwLock::new(Cfg::default()),
    });
    nyanyabot_proto::run_plugin_with_host(plugin, host).await
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
mod build_url_tests {
    use super::*;
    #[test]
    fn build_url_includes_params() {
        let req = BuildURLParams {
            page_url: "https://example.com/a".into(),
            selector: "#main".into(),
            width: Some(800),
            height: Some(600),
            format: "png".into(),
            full_page: Some(true),
            ..Default::default()
        };
        let url = build_screenshot_url("http://shot:8080", &req).unwrap();
        assert!(url.contains("/screenshot"));
        assert!(url.contains("url=https%3A%2F%2Fexample.com%2Fa") || url.contains("example.com"));
        assert!(url.contains("selector"));
        assert!(url.contains("800"));
        assert!(url.contains("full_page=true"));
    }
}
