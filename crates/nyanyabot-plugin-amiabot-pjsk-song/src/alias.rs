use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

pub const DEFAULT_ALIAS_DATA_URL: &str = "https://raw.githubusercontent.com/moe-sekai/MoeSekai-Hub/main/data/music_alias/music_aliases.json";
pub const DEFAULT_CACHE_DIR: &str = "/tmp/nyanyabot";
pub const CACHE_FILE_NAME: &str = "pjsk-song-alias-cache.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MusicAlias {
    #[serde(default, alias = "id")]
    pub music_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AliasData {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub musics: Vec<MusicAlias>,
}

pub struct AliasManager {
    data: AliasData,
    normalized: HashMap<String, i64>,
    music_map: HashMap<i64, MusicAlias>,
    last_load: Option<SystemTime>,
    cache_dir: PathBuf,
    cache_ttl: Duration,
    data_url: String,
}

impl AliasManager {
    pub fn new(data_url: &str, cache_dir: &str, cache_ttl_secs: i64) -> Self {
        let data_url = if data_url.trim().is_empty() {
            DEFAULT_ALIAS_DATA_URL.to_string()
        } else {
            data_url.trim().to_string()
        };
        let cache_dir = if cache_dir.trim().is_empty() {
            PathBuf::from(DEFAULT_CACHE_DIR)
        } else {
            PathBuf::from(cache_dir.trim())
        };
        let cache_ttl = if cache_ttl_secs > 0 {
            Duration::from_secs(cache_ttl_secs as u64)
        } else {
            Duration::from_secs(3600)
        };
        Self {
            data: AliasData::default(),
            normalized: HashMap::new(),
            music_map: HashMap::new(),
            last_load: None,
            cache_dir,
            cache_ttl,
            data_url,
        }
    }

    pub fn data(&self) -> &AliasData {
        &self.data
    }

    pub fn normalized_get(&self, key: &str) -> Option<i64> {
        self.normalized.get(key).copied()
    }

    pub fn music_get(&self, id: i64) -> Option<&MusicAlias> {
        self.music_map.get(&id)
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join(CACHE_FILE_NAME)
    }

    fn is_cache_valid(&self) -> bool {
        let path = self.cache_path();
        let Ok(meta) = fs::metadata(&path) else {
            return false;
        };
        let Ok(modified) = meta.modified() else {
            return false;
        };
        SystemTime::now()
            .duration_since(modified)
            .map(|d| d <= self.cache_ttl)
            .unwrap_or(false)
    }

    fn rebuild_indexes(&mut self) {
        self.normalized.clear();
        self.music_map.clear();
        for music in &self.data.musics {
            self.music_map.insert(music.music_id, music.clone());
            let title_n = crate::matcher::normalize_string(&music.title);
            if !title_n.is_empty() {
                self.normalized.insert(title_n, music.music_id);
            }
            for alias in &music.aliases {
                let a = crate::matcher::normalize_string(alias);
                if !a.is_empty() {
                    self.normalized.insert(a, music.music_id);
                }
            }
            // also index music id as string for song123 style? Go uses song{id} via candidate only
            self.normalized
                .insert(music.music_id.to_string(), music.music_id);
        }
        self.last_load = Some(SystemTime::now());
    }

    fn load_from_cache(&mut self) -> Result<(), String> {
        let path = self.cache_path();
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        self.data = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        self.rebuild_indexes();
        Ok(())
    }

    fn save_to_cache(&self) -> Result<(), String> {
        fs::create_dir_all(&self.cache_dir).map_err(|e| e.to_string())?;
        let path = self.cache_path();
        let raw = serde_json::to_vec_pretty(&self.data).map_err(|e| e.to_string())?;
        // atomic-ish write
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, raw).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn load_from_url(&mut self) -> Result<(), String> {
        info!(url = %self.data_url, "loading pjsk song aliases");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(&self.data_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        self.data = resp.json().await.map_err(|e| e.to_string())?;
        self.rebuild_indexes();
        Ok(())
    }

    pub async fn load(&mut self) -> Result<(), String> {
        if self.is_cache_valid() {
            match self.load_from_cache() {
                Ok(()) => {
                    info!(count = self.data.musics.len(), "aliases loaded from cache");
                    return Ok(());
                }
                Err(err) => warn!(error=%err, "alias cache load failed"),
            }
        }

        match self.load_from_url().await {
            Ok(()) => {
                if let Err(err) = self.save_to_cache() {
                    warn!(error=%err, "alias cache save failed");
                }
                info!(count = self.data.musics.len(), "aliases loaded from remote");
                Ok(())
            }
            Err(err) => {
                warn!(error=%err, "remote alias load failed; trying stale cache");
                self.load_from_cache()
                    .map_err(|e| format!("remote load failed and no cache: {err}; {e}"))?;
                Ok(())
            }
        }
    }

    /// Test helper: inject data without network.
    #[cfg(test)]
    pub fn load_from_data(&mut self, data: AliasData) {
        self.data = data;
        self.rebuild_indexes();
    }
}
