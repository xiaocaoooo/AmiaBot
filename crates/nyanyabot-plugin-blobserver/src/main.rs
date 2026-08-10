use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nyanyabot_proto::{
    CommandMatch, ConfigSpec, Descriptor, ExportSpec, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{normalize_http_base, redact_secrets};
use reqwest::multipart;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;
use url::Url;

#[derive(Default, Clone, Deserialize)]
struct Cfg {
    #[serde(default)]
    blob_server: String,
    #[serde(default)]
    blob_token: String,
    #[serde(default)]
    blob_onebot_url: String,
}

struct BlobPlugin {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Cfg>,
    http: reqwest::Client,
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "BlobServer".into(),
        plugin_id: "external.blobserver".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description: "Blob upload helper plugin".into(),
        exports: vec![ExportSpec {
            name: "blob.upload_remote".into(),
            description: "下载远程文件并上传到 Blob Server，返回 Blob URL 与 OneBot URL".into(),
            params_schema: json!({"type":"object","required":["download_url","blob_id"]}),
            result_schema: json!({"type":"object","required":["blob_url","onebot_url"]}),
        }],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Blob plugin config".into()),
            schema: Some(json!({"type":"object","properties":{
                "blob_server":{"type":"string","description":"Blob Server 域名/地址"},
                "blob_token":{"type":"string"},
                "blob_onebot_url":{"type":"string","description":"发送给 OneBot 的 Blob URL 前缀"}
            }})),
            default: Some(json!({"blob_server":"","blob_token":"","blob_onebot_url":""})),
        }),
        ..Default::default()
    }
}

fn build_blob_url(blob_server: &str, id: &str) -> Result<String, StructuredError> {
    let base = normalize_http_base(blob_server);
    let mut u = Url::parse(&base).map_err(|e| StructuredError::internal(e.to_string()))?;
    let path = format!(
        "{}/v1/blobs/{}",
        u.path().trim_end_matches('/'),
        urlencoding_path(id)
    );
    u.set_path(&path);
    u.set_query(None);
    Ok(u.to_string())
}

fn urlencoding_path(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn append_token_to_url(u: &str, token: &str) -> String {
    let u = u.trim();
    let token = token.trim();
    if u.is_empty() || token.is_empty() {
        return u.to_string();
    }
    let Ok(mut pu) = Url::parse(u) else {
        return u.to_string();
    };
    let has_token = pu.query_pairs().any(|(k, _)| k == "token");
    if !has_token {
        pu.query_pairs_mut().append_pair("token", token);
    }
    pu.to_string()
}

fn rewrite_blob_url_for_onebot(u: &str, blob_server: &str, blob_onebot_url: &str) -> String {
    let u = u.trim();
    if u.is_empty() {
        return u.to_string();
    }
    let base = normalize_http_base(blob_server);
    if base.is_empty() || !u.starts_with(&base) {
        return u.to_string();
    }
    let new_base = normalize_http_base(blob_onebot_url);
    if new_base.is_empty() {
        return u.to_string();
    }
    format!("{new_base}{}", u.trim_start_matches(&base))
}

async fn download_to_temp(
    http: &reqwest::Client,
    download_url: &str,
    id: &str,
    kind: &str,
) -> Result<(PathBuf, String), StructuredError> {
    let resp = http
        .get(download_url)
        .header("User-Agent", "nyanyabot-plugin-blobserver/0.1")
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?
        .error_for_status()
        .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?;

    let mut filename = format!("{id}.bin");
    if let Some(cd) = resp.headers().get(reqwest::header::CONTENT_DISPOSITION)
        && let Ok(cd) = cd.to_str()
    {
        // very small parser: filename="..."
        if let Some(idx) = cd.find("filename=") {
            let mut name = cd[idx + 9..].trim().trim_matches('"').trim().to_string();
            if let Some(semi) = name.find(';') {
                name = name[..semi].trim().trim_matches('"').to_string();
            }
            if !name.is_empty() {
                filename = name;
            }
        }
    }
    if filename == format!("{id}.bin")
        && let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE)
        && let Ok(ct) = ct.to_str()
    {
        let ct = ct.split(';').next().unwrap_or(ct).trim();
        let ext = match ct {
            "image/png" => Some(".png"),
            "image/jpeg" | "image/jpg" => Some(".jpg"),
            "image/gif" => Some(".gif"),
            "image/webp" => Some(".webp"),
            "video/mp4" => Some(".mp4"),
            "application/pdf" => Some(".pdf"),
            _ => None,
        };
        if let Some(ext) = ext {
            filename = format!("{id}{ext}");
        }
    }
    if filename == format!("{id}.bin") {
        match kind {
            "image" => filename = format!("{id}.png"),
            "video" => filename = format!("{id}.mp4"),
            _ => {}
        }
    }

    let dir = Path::new("/tmp/nyanyabot-blob");
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    let tmp = dir.join(format!(
        "nyanyabot-blobserver-{}-{}.tmp",
        std::process::id(),
        id.replace(['/', '\\'], "_")
    ));
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    let mut f = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    f.write_all(&bytes)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    f.flush()
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    Ok((tmp, filename))
}

async fn blob_prepare(
    http: &reqwest::Client,
    blob_server: &str,
    blob_token: &str,
    id: &str,
) -> Result<bool, StructuredError> {
    let base = normalize_http_base(blob_server);
    let mut u = Url::parse(&base).map_err(|e| StructuredError::internal(e.to_string()))?;
    let path = format!("{}/v1/blobs/prepare", u.path().trim_end_matches('/'));
    u.set_path(&path);
    let mut builder = http
        .post(u.to_string())
        .timeout(Duration::from_secs(10))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({"id": id}));
    if !blob_token.is_empty() {
        builder = builder.bearer_auth(blob_token);
    }
    let resp = builder
        .send()
        .await
        .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?
        .error_for_status()
        .map_err(|e| {
            StructuredError::internal(redact_secrets(&format!("blob prepare failed: {e}")))
        })?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    Ok(body
        .get("upload_required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

async fn upload_file_to_blob(
    http: &reqwest::Client,
    blob_server: &str,
    blob_token: &str,
    id: &str,
    file_path: &Path,
    filename: &str,
) -> Result<(), StructuredError> {
    let upload_required = blob_prepare(http, blob_server, blob_token, id).await?;
    if !upload_required {
        return Ok(());
    }
    let upload_url = build_blob_url(blob_server, id)?;
    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
    let part = multipart::Part::bytes(bytes).file_name(filename.to_string());
    let form = multipart::Form::new().part("file", part);
    let mut builder = http
        .post(upload_url)
        .timeout(Duration::from_secs(300))
        .multipart(form);
    if !blob_token.is_empty() {
        builder = builder.bearer_auth(blob_token);
    }
    builder
        .send()
        .await
        .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?
        .error_for_status()
        .map_err(|e| {
            StructuredError::internal(redact_secrets(&format!("blob upload failed: {e}")))
        })?;
    Ok(())
}

#[async_trait]
impl Plugin for BlobPlugin {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }

    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        let parsed: Cfg = serde_json::from_value(config).unwrap_or_default();
        *self.cfg.write() = Cfg {
            blob_server: parsed.blob_server.trim().to_string(),
            blob_token: parsed.blob_token.trim().to_string(),
            blob_onebot_url: parsed.blob_onebot_url.trim().to_string(),
        };
        Ok(())
    }

    async fn invoke(
        &self,
        method: &str,
        params: Value,
        _caller: &str,
    ) -> Result<Value, StructuredError> {
        if method != "blob.upload_remote" {
            return Err(StructuredError::not_found("method is not exported"));
        }
        #[derive(Deserialize)]
        struct Req {
            download_url: String,
            blob_id: String,
            #[serde(default)]
            kind: String,
        }
        let mut req: Req = serde_json::from_value(params)
            .map_err(|e| StructuredError::invalid_params(e.to_string()))?;
        req.download_url = req.download_url.trim().to_string();
        req.blob_id = req.blob_id.trim().to_string();
        req.kind = req.kind.trim().to_string();
        if req.download_url.is_empty() {
            return Err(StructuredError::invalid_params("download_url is required"));
        }
        if req.blob_id.is_empty() {
            return Err(StructuredError::invalid_params("blob_id is required"));
        }
        if req.kind.is_empty() {
            req.kind = "file".into();
        }
        let cfg = self.cfg.read().clone();
        if cfg.blob_server.trim().is_empty() {
            return Err(StructuredError::internal("blob_server is not configured"));
        }

        let (tmp, filename) =
            download_to_temp(&self.http, &req.download_url, &req.blob_id, &req.kind).await?;
        let upload_res = upload_file_to_blob(
            &self.http,
            &cfg.blob_server,
            &cfg.blob_token,
            &req.blob_id,
            &tmp,
            &filename,
        )
        .await;
        let _ = tokio::fs::remove_file(&tmp).await;
        upload_res?;

        let blob_url = build_blob_url(&cfg.blob_server, &req.blob_id)?;
        let mut onebot_url = blob_url.clone();
        if !cfg.blob_onebot_url.trim().is_empty() {
            onebot_url =
                rewrite_blob_url_for_onebot(&onebot_url, &cfg.blob_server, &cfg.blob_onebot_url);
        }
        if !cfg.blob_token.is_empty() {
            onebot_url = append_token_to_url(&onebot_url, &cfg.blob_token);
        }
        Ok(json!({"blob_url": blob_url, "onebot_url": onebot_url}))
    }

    async fn handle(
        &self,
        _: &str,
        _: Value,
        _: Option<CommandMatch>,
        _: &str,
    ) -> Result<HandleResult, StructuredError> {
        Ok(HandleResult::handled())
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
    let plugin = Arc::new(BlobPlugin {
        host: host.clone(),
        cfg: RwLock::new(Cfg::default()),
        http: reqwest::Client::builder().use_rustls_tls().build()?,
    });
    run_plugin_with_host(plugin, host).await
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
    use super::*;
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    async fn spawn_go_like_blob_server() -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = vec![0u8; 65536];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                log2.lock().await.push(req.clone());
                let (status, body) =
                    if req.starts_with("POST ") && req.contains("/v1/blobs/prepare") {
                        (200, r#"{"upload_required":true}"#)
                    } else if req.starts_with("POST ") && req.contains("/v1/blobs/") {
                        (200, r#"{"ok":true}"#)
                    } else if req.starts_with("GET ") {
                        // source download
                        (200, "PNGDATA")
                    } else {
                        (404, "no")
                    };
                let resp = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });
        (format!("http://{addr}"), log)
    }

    #[tokio::test]
    async fn upload_remote_uses_v1_prepare_and_multipart() {
        let (base, log) = spawn_go_like_blob_server().await;
        // separate tiny download server
        let dl = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dl_addr = dl.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = dl.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let body = b"hello-image";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.write_all(body).await;
        });

        let plugin = BlobPlugin {
            host: Arc::new(AsyncRwLock::new(None)),
            cfg: RwLock::new(Cfg {
                blob_server: base.clone(),
                blob_token: "tok".into(),
                blob_onebot_url: format!("{base}/onebot"),
            }),
            http: reqwest::Client::builder().use_rustls_tls().build().unwrap(),
        };
        let out = plugin
            .invoke(
                "blob.upload_remote",
                json!({
                    "download_url": format!("http://{dl_addr}/src.png"),
                    "blob_id": "abc",
                    "kind": "image"
                }),
                "caller",
            )
            .await
            .unwrap();
        let blob_url = out.get("blob_url").and_then(|v| v.as_str()).unwrap();
        let onebot_url = out.get("onebot_url").and_then(|v| v.as_str()).unwrap();
        assert!(blob_url.contains("/v1/blobs/abc"), "{blob_url}");
        assert!(onebot_url.contains("/onebot/v1/blobs/abc"), "{onebot_url}");
        assert!(onebot_url.contains("token=tok"), "{onebot_url}");

        let entries = log.lock().await.clone();
        assert!(
            entries.iter().any(|e| e.contains("/v1/blobs/prepare")),
            "missing prepare: {entries:?}"
        );
        assert!(
            entries.iter().any(|e| e.contains("POST ")
                && e.contains("/v1/blobs/abc")
                && e.contains("multipart")),
            "missing multipart upload: {entries:?}"
        );
    }

    #[test]
    fn rewrite_and_token_helpers() {
        let u = "http://blob:8080/v1/blobs/x";
        let rewritten = rewrite_blob_url_for_onebot(u, "http://blob:8080", "http://public");
        assert_eq!(rewritten, "http://public/v1/blobs/x");
        let with_tok = append_token_to_url(&rewritten, "secret");
        assert!(with_tok.contains("token=secret"));
    }
}
