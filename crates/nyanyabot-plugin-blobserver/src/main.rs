use std::sync::Arc;

use async_trait::async_trait;
use nyanyabot_proto::{
    CommandMatch, ConfigSpec, Descriptor, ExportSpec, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use plugin_common::{join_url, normalize_http_base, redact_secrets};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

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
                "blob_server":{"type":"string"},
                "blob_token":{"type":"string"},
                "blob_onebot_url":{"type":"string"}
            }})),
            default: Some(json!({"blob_server":"","blob_token":"","blob_onebot_url":""})),
        }),
        ..Default::default()
    }
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
        let req: Req = serde_json::from_value(params)
            .map_err(|e| StructuredError::invalid_params(e.to_string()))?;
        if req.download_url.trim().is_empty() || req.blob_id.trim().is_empty() {
            return Err(StructuredError::invalid_params(
                "download_url and blob_id are required",
            ));
        }
        let cfg = self.cfg.read().clone();
        if cfg.blob_server.trim().is_empty() {
            return Err(StructuredError::internal("blob_server is not configured"));
        }
        let bytes = self
            .http
            .get(&req.download_url)
            .send()
            .await
            .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?
            .error_for_status()
            .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?
            .bytes()
            .await
            .map_err(|e| StructuredError::internal(e.to_string()))?;

        let upload_url = join_url(&cfg.blob_server, &format!("blob/{}", req.blob_id.trim()));
        let mut builder = self.http.put(&upload_url).body(bytes.to_vec());
        if !cfg.blob_token.is_empty() {
            builder = builder.bearer_auth(&cfg.blob_token);
        }
        if req.kind == "image" || req.download_url.contains(".png") {
            builder = builder.header(reqwest::header::CONTENT_TYPE, "image/png");
        }
        builder
            .send()
            .await
            .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?
            .error_for_status()
            .map_err(|e| StructuredError::internal(redact_secrets(&e.to_string())))?;

        let blob_url = upload_url;
        let onebot_url = if cfg.blob_onebot_url.trim().is_empty() {
            blob_url.clone()
        } else {
            join_url(
                &cfg.blob_onebot_url,
                &format!("blob/{}", req.blob_id.trim()),
            )
        };
        let _ = normalize_http_base; // keep import used via join_url
        Ok(json!({"blob_url": blob_url, "onebot_url": onebot_url}))
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
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
mod mock_http_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_mock_blob() -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let h = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let req = String::from_utf8_lossy(&buf);
                if req.starts_with("GET ") {
                    let body = b"hello-image";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.write_all(body).await;
                } else if req.starts_with("POST ") || req.starts_with("PUT ") {
                    let body = br#"{"url":"http://blob/local/x","id":"x"}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.write_all(body).await;
                } else {
                    let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                }
            }
        });
        (format!("http://{addr}"), h)
    }

    #[tokio::test]
    async fn mock_upload_remote() {
        let (base, _h) = spawn_mock_blob().await;
        let host = Arc::new(AsyncRwLock::new(None));
        let plugin = BlobPlugin {
            host,
            cfg: RwLock::new(Cfg {
                blob_server: base.clone(),
                blob_token: "tok".into(),
                blob_onebot_url: format!("{base}/onebot"),
            }),
            http: reqwest::Client::new(),
        };
        // download from same mock
        let out = plugin
            .invoke(
                "blob.upload_remote",
                json!({
                    "download_url": format!("{base}/file.png"),
                    "blob_id": "abc",
                    "kind": "image"
                }),
                "test",
            )
            .await
            .expect("upload");
        assert!(
            out.get("blob_url").is_some() || out.get("onebot_url").is_some(),
            "{out}"
        );
    }
}
