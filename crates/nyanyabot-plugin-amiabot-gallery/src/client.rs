use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GalleryConfig {
    pub gallery_server: String,
    pub amiabot_pages: String,
}

impl Default for GalleryConfig {
    fn default() -> Self {
        Self {
            gallery_server: plugin_common::normalize_http_base("http://127.0.0.1:3000"),
            amiabot_pages: String::new(),
        }
    }
}

impl GalleryConfig {
    pub fn from_value(v: &serde_json::Value) -> Self {
        let mut cfg = Self::default();
        if let Some(s) = v.get("gallery_server").and_then(|x| x.as_str()) {
            let n = plugin_common::normalize_http_base(s);
            if !n.is_empty() {
                cfg.gallery_server = n;
            }
        }
        if let Some(s) = v.get("amiabot_pages").and_then(|x| x.as_str()) {
            cfg.amiabot_pages = plugin_common::normalize_http_base(s);
        }
        cfg
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GalleryDetail {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ImageDetail {
    pub id: Uuid,
    #[serde(default)]
    pub sha256_hex: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ext: String,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct GalleryApiError {
    pub status_code: u16,
    pub message: String,
    pub duplicate_image_id: Option<Uuid>,
}

impl std::fmt::Display for GalleryApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.message.trim().is_empty() {
            write!(f, "{}", self.message)
        } else if self.status_code > 0 {
            write!(f, "画廊服务返回 HTTP {}", self.status_code)
        } else {
            write!(f, "画廊服务发生错误")
        }
    }
}

impl std::error::Error for GalleryApiError {}

#[derive(Clone)]
pub struct GalleryClient {
    base: String,
    http: reqwest::Client,
}

impl GalleryClient {
    pub fn new(cfg: &GalleryConfig) -> Self {
        Self {
            base: cfg.gallery_server.clone(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("http client"),
        }
    }

    pub fn build_render_url(&self, image_id: Uuid) -> String {
        format!("{}/files/{image_id}", self.base.trim_end_matches('/'))
    }

    async fn decode_api_error(&self, resp: reqwest::Response) -> GalleryApiError {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let mut message = body.clone();
        let mut duplicate_image_id = None;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(m) = v
                .get("error")
                .or_else(|| v.get("message"))
                .and_then(|x| x.as_str())
            {
                message = m.to_string();
            }
            if let Some(id) = v
                .get("image_id")
                .or_else(|| v.get("duplicate_image_id"))
                .and_then(|x| x.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                duplicate_image_id = Some(id);
            }
        }
        GalleryApiError {
            status_code: status,
            message,
            duplicate_image_id,
        }
    }

    fn transport_err(e: impl ToString) -> GalleryApiError {
        GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: None,
        }
    }

    pub async fn create_gallery(&self, name: &str) -> Result<GalleryDetail, GalleryApiError> {
        let url = format!("{}/galleries", self.base.trim_end_matches('/'));
        let body = json!({
            "name": name,
            "aliases": [name],
        });
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(Self::transport_err)?;
        if !(resp.status().is_success() || resp.status().as_u16() == 201) {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(Self::transport_err)
    }

    pub async fn list_galleries(
        &self,
        search: Option<&str>,
    ) -> Result<Vec<GalleryDetail>, GalleryApiError> {
        let mut url = format!("{}/galleries", self.base.trim_end_matches('/'));
        if let Some(q) = search.map(str::trim).filter(|s| !s.is_empty()) {
            url.push_str(&format!("?search={}", urlencoding_simple(q)));
        }
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(Self::transport_err)?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(Self::transport_err)
    }

    /// Exact match on gallery name or alias (case-insensitive). First hit wins.
    pub async fn resolve_gallery(
        &self,
        name: &str,
    ) -> Result<Option<GalleryDetail>, GalleryApiError> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(None);
        }
        let list = self.list_galleries(Some(name)).await?;
        let needle = name.to_lowercase();
        Ok(list.into_iter().find(|g| {
            g.name.to_lowercase() == needle || g.aliases.iter().any(|a| a.to_lowercase() == needle)
        }))
    }

    pub async fn upload_image(
        &self,
        gallery_id: Uuid,
        filename: &str,
        data: Vec<u8>,
        force: bool,
    ) -> Result<ImageDetail, GalleryApiError> {
        let mut url = format!(
            "{}/galleries/{gallery_id}/images",
            self.base.trim_end_matches('/')
        );
        if force {
            url.push_str("?force=true");
        }
        let filename = if filename.trim().is_empty() {
            "image.bin"
        } else {
            filename
        };
        let mime = guess_mime(filename);
        let part = Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str(mime)
            .map_err(Self::transport_err)?;
        let form = Form::new().part("file", part);
        let resp = self
            .http
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(Self::transport_err)?;
        let status = resp.status().as_u16();
        // 200 reuse / 201 created
        if status != 200 && status != 201 {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(Self::transport_err)
    }

    pub async fn list_gallery_images(
        &self,
        gallery_id: Uuid,
    ) -> Result<Vec<ImageDetail>, GalleryApiError> {
        let url = format!(
            "{}/galleries/{gallery_id}/images",
            self.base.trim_end_matches('/')
        );
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(Self::transport_err)?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(Self::transport_err)
    }

    pub async fn get_image(&self, id: Uuid) -> Result<ImageDetail, GalleryApiError> {
        let url = format!("{}/images/{id}", self.base.trim_end_matches('/'));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(Self::transport_err)?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(Self::transport_err)
    }
}

fn guess_mime(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "application/octet-stream"
    }
}

fn urlencoding_simple(s: &str) -> String {
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

pub async fn download_bytes(url: &str) -> Result<(String, Vec<u8>), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    let name = resp
        .url()
        .path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("image.bin")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok((name, bytes.to_vec()))
}

#[cfg(test)]
mod client_unit_tests {
    use super::*;

    #[test]
    fn render_url_shape() {
        let cfg = GalleryConfig {
            gallery_server: "http://g.example.com".into(),
            amiabot_pages: String::new(),
        };
        let client = GalleryClient::new(&cfg);
        let id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        assert_eq!(
            client.build_render_url(id),
            "http://g.example.com/files/11111111-1111-1111-1111-111111111111"
        );
    }

    #[test]
    fn decode_duplicate_image_id() {
        let body = r#"{"error":"Perceptual duplicate detected","image_id":"22222222-2222-2222-2222-222222222222"}"#;
        let v: serde_json::Value = serde_json::from_str(body).unwrap();
        let id = v
            .get("image_id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        assert_eq!(
            id,
            Some(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap())
        );
    }
}
