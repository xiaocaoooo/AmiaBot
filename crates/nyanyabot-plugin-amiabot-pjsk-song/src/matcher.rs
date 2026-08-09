use std::collections::HashMap;

use crate::alias::AliasManager;

pub const WEIGHT_EXACT: f64 = 1.0;
pub const WEIGHT_PREFIX: f64 = 0.8;
pub const WEIGHT_CONTAINS: f64 = 0.7;
pub const WEIGHT_SUBSEQUENCE: f64 = 0.5;

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub music_id: i64,
    pub title: String,
    pub confidence: f64,
    #[allow(dead_code)]
    pub matched_key: String,
}

pub fn normalize_string(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn calculate_prefix_score(query: &str, target: &str) -> f64 {
    if query.is_empty() || target.is_empty() {
        return 0.0;
    }
    if query == target {
        return WEIGHT_EXACT;
    }
    if target.starts_with(query) {
        return WEIGHT_PREFIX;
    }
    if target.contains(query) {
        return WEIGHT_CONTAINS;
    }
    0.0
}

fn calculate_subsequence_score(query: &str, target: &str) -> f64 {
    if query.is_empty() || target.is_empty() {
        return 0.0;
    }
    let mut ti = target.chars().peekable();
    for qc in query.chars() {
        loop {
            match ti.next() {
                Some(tc) if tc == qc => break,
                Some(_) => continue,
                None => return 0.0,
            }
        }
    }
    WEIGHT_SUBSEQUENCE
}

pub fn fuzzy_search(query: &str, am: &AliasManager) -> Vec<MatchResult> {
    let normalized_query = normalize_string(query);
    if normalized_query.is_empty() {
        return Vec::new();
    }
    let data = am.data();
    if data.musics.is_empty() {
        return Vec::new();
    }

    let mut result_map: HashMap<i64, MatchResult> = HashMap::new();

    // Level 1 exact via normalized index
    if let Some(music_id) = am.normalized_get(&normalized_query)
        && let Some(music) = am.music_get(music_id)
    {
        result_map.insert(
            music_id,
            MatchResult {
                music_id,
                title: music.title.clone(),
                confidence: WEIGHT_EXACT,
                matched_key: query.to_string(),
            },
        );
    }

    // Level 2 prefix/contains
    for music in &data.musics {
        if result_map.contains_key(&music.music_id) {
            continue;
        }
        let nt = normalize_string(&music.title);
        let mut conf = calculate_prefix_score(&normalized_query, &nt);
        let mut key = music.title.clone();
        if conf <= 0.0 {
            for alias in &music.aliases {
                let na = normalize_string(alias);
                conf = calculate_prefix_score(&normalized_query, &na);
                if conf > 0.0 {
                    key = alias.clone();
                    break;
                }
            }
        }
        if conf > 0.0 {
            result_map.insert(
                music.music_id,
                MatchResult {
                    music_id: music.music_id,
                    title: music.title.clone(),
                    confidence: conf,
                    matched_key: key,
                },
            );
        }
    }

    // Level 3 subsequence if few results
    if result_map.len() < 5 {
        for music in &data.musics {
            if result_map.contains_key(&music.music_id) {
                continue;
            }
            let nt = normalize_string(&music.title);
            let mut conf = calculate_subsequence_score(&normalized_query, &nt);
            let mut key = music.title.clone();
            if conf <= 0.0 {
                for alias in &music.aliases {
                    let na = normalize_string(alias);
                    conf = calculate_subsequence_score(&normalized_query, &na);
                    if conf > 0.0 {
                        key = alias.clone();
                        break;
                    }
                }
            }
            if conf > 0.0 {
                result_map.insert(
                    music.music_id,
                    MatchResult {
                        music_id: music.music_id,
                        title: music.title.clone(),
                        confidence: conf,
                        matched_key: key,
                    },
                );
            }
        }
    }

    let mut out: Vec<_> = result_map.into_values().collect();
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.music_id.cmp(&b.music_id))
    });
    // keep top 10 like typical UX
    out.truncate(10);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_prefix() {
        assert!((calculate_prefix_score("abc", "abc") - WEIGHT_EXACT).abs() < 1e-9);
        assert!((calculate_prefix_score("ab", "abcd") - WEIGHT_PREFIX).abs() < 1e-9);
        assert!((calculate_prefix_score("bc", "abcd") - WEIGHT_CONTAINS).abs() < 1e-9);
    }

    #[test]
    fn fuzzy_rank_and_limit() {
        assert!((calculate_subsequence_score("ac", "abc") - WEIGHT_SUBSEQUENCE).abs() < 1e-9);
        assert_eq!(calculate_subsequence_score("ad", "abc"), 0.0);
    }
}
