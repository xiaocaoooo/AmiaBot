use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct GalleryConfig {
    pub gallery_server: String,
    pub gallery_read_token: String,
    pub gallery_write_token: String,
    pub amiabot_pages: String,
}

impl Default for GalleryConfig {
    fn default() -> Self {
        Self {
            gallery_server: plugin_common::normalize_http_base("http://127.0.0.1:25006"),
            gallery_read_token: String::new(),
            gallery_write_token: String::new(),
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
        if let Some(s) = v.get("gallery_read_token").and_then(|x| x.as_str()) {
            cfg.gallery_read_token = s.trim().to_string();
        }
        if let Some(s) = v.get("gallery_write_token").and_then(|x| x.as_str()) {
            cfg.gallery_write_token = s.trim().to_string();
        }
        if let Some(s) = v.get("amiabot_pages").and_then(|x| x.as_str()) {
            cfg.amiabot_pages = plugin_common::normalize_http_base(s);
        }
        cfg
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GalleryTag {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct GalleryImage {
    pub id: i64,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub fid: String,
    #[serde(default)]
    pub file_size: i64,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GalleryImageWithTags {
    #[serde(flatten)]
    pub image: GalleryImage,
    #[serde(default)]
    pub tags: Vec<GalleryTag>,
}

#[derive(Debug)]
pub struct GalleryApiError {
    pub status_code: u16,
    pub message: String,
    pub duplicate_image_id: i64,
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
    read_token: String,
    write_token: String,
    http: reqwest::Client,
}

impl GalleryClient {
    pub fn new(cfg: &GalleryConfig) -> Self {
        Self {
            base: cfg.gallery_server.clone(),
            read_token: cfg.gallery_read_token.clone(),
            write_token: cfg.gallery_write_token.clone(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("http client"),
        }
    }

    pub fn build_render_url(&self, image_id: i64) -> String {
        format!(
            "{}/v1/images/{image_id}/file",
            self.base.trim_end_matches('/')
        )
    }

    async fn decode_api_error(&self, resp: reqwest::Response) -> GalleryApiError {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        let mut message = body.clone();
        let mut duplicate_image_id = 0i64;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(m) = v
                .get("message")
                .or_else(|| v.get("error"))
                .and_then(|x| x.as_str())
            {
                message = m.to_string();
            }
            if let Some(id) = v
                .get("duplicate_image_id")
                .or_else(|| v.get("existing_id"))
                .and_then(|x| x.as_i64())
            {
                duplicate_image_id = id;
            }
        }
        GalleryApiError {
            status_code: status,
            message,
            duplicate_image_id,
        }
    }

    fn apply_auth(&self, mut req: reqwest::RequestBuilder, write: bool) -> reqwest::RequestBuilder {
        let token = if write {
            &self.write_token
        } else {
            &self.read_token
        };
        if !token.is_empty() {
            req = req.bearer_auth(token);
        }
        req
    }

    pub async fn create_tag(&self, name: &str) -> Result<GalleryTag, GalleryApiError> {
        let url = format!("{}/v1/tags", self.base.trim_end_matches('/'));
        let req = self.http.post(&url).json(&json!({"name": name.trim()}));
        let req = self.apply_auth(req, true);
        let resp = req.send().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        if resp.status().as_u16() != 201 {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })
    }

    pub async fn list_tags(&self, q: &str, limit: i32) -> Result<Vec<GalleryTag>, GalleryApiError> {
        let mut url = format!("{}/v1/tags", self.base.trim_end_matches('/'));
        let mut params = vec![];
        if !q.trim().is_empty() {
            params.push(format!("q={}", urlencoding_simple(q.trim())));
        }
        if limit > 0 {
            params.push(format!("limit={limit}"));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let req = self.apply_auth(self.http.get(&url), false);
        let resp = req.send().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        #[derive(Deserialize)]
        struct Payload {
            items: Vec<GalleryTag>,
        }
        let payload: Payload = resp.json().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        Ok(payload.items)
    }

    pub async fn find_exact_tag(&self, name: &str) -> Result<Option<GalleryTag>, GalleryApiError> {
        let items = self.list_tags(name, 100).await?;
        Ok(items
            .into_iter()
            .find(|t| t.name.trim().eq_ignore_ascii_case(name.trim())))
    }

    pub async fn find_missing_tags(&self, tags: &[String]) -> Result<Vec<String>, GalleryApiError> {
        let mut missing = Vec::new();
        for tag in tags {
            if self.find_exact_tag(tag).await?.is_none() {
                missing.push(tag.clone());
            }
        }
        Ok(missing)
    }

    pub async fn upload_image(
        &self,
        filename: &str,
        data: Vec<u8>,
        tags: &[String],
        force: bool,
    ) -> Result<GalleryImageWithTags, GalleryApiError> {
        let mut form = Form::new();
        let part = Part::bytes(data).file_name(filename.to_string());
        form = form.part("file", part);
        for tag in tags {
            form = form.text("tags", tag.clone());
        }
        if force {
            form = form.text("force", "true");
        }
        let url = format!("{}/v1/images/upload", self.base.trim_end_matches('/'));
        let req = self.apply_auth(self.http.post(&url).multipart(form), true);
        let resp = req.send().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        let status = resp.status().as_u16();
        if status != 200 && status != 201 {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })
    }

    pub async fn get_image(&self, id: i64) -> Result<GalleryImageWithTags, GalleryApiError> {
        let url = format!("{}/v1/images/{id}", self.base.trim_end_matches('/'));
        let req = self.apply_auth(self.http.get(&url), false);
        let resp = req.send().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })
    }

    pub async fn random_image(
        &self,
        tags: &[String],
    ) -> Result<GalleryImageWithTags, GalleryApiError> {
        let mut url = format!("{}/v1/images/random", self.base.trim_end_matches('/'));
        if !tags.is_empty() {
            let joined = tags
                .iter()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(",");
            if !joined.is_empty() {
                url.push_str(&format!("?tags={}", urlencoding_simple(&joined)));
            }
        }
        let req = self.apply_auth(self.http.get(&url), false);
        let resp = req.send().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        resp.json().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })
    }

    #[allow(dead_code)]
    pub async fn list_images_by_tag(
        &self,
        tag: &str,
        page: i32,
        page_size: i32,
    ) -> Result<(Vec<GalleryImageWithTags>, i64), GalleryApiError> {
        let url = format!(
            "{}/v1/images?tag={}&page={}&page_size={}",
            self.base.trim_end_matches('/'),
            urlencoding_simple(tag),
            page.max(1),
            page_size.max(1)
        );
        let req = self.apply_auth(self.http.get(&url), false);
        let resp = req.send().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        if !resp.status().is_success() {
            return Err(self.decode_api_error(resp).await);
        }
        #[derive(Deserialize)]
        struct Payload {
            items: Vec<GalleryImageWithTags>,
            #[serde(default)]
            total: i64,
        }
        let payload: Payload = resp.json().await.map_err(|e| GalleryApiError {
            status_code: 0,
            message: e.to_string(),
            duplicate_image_id: 0,
        })?;
        Ok((payload.items, payload.total))
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
