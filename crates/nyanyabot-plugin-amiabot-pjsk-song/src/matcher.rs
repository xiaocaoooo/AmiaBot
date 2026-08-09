use std::collections::HashMap;

use crate::alias::AliasManager;
use regex::Regex;
use std::sync::OnceLock;

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

fn special_chars_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"[\s\p{P}\p{S}−\-_！？。、！？，；：（）【】「」『』〈〉《"]+"#).unwrap()
    })
}

fn traditional_to_simple(s: &str) -> String {
    const MAP: &[(char, char)] = &[
        ('夢', '梦'),
        ('開', '开'),
        ('傳', '传'),
        ('說', '说'),
        ('訴', '诉'),
        ('時', '时'),
        ('機', '机'),
        ('樂', '乐'),
        ('斷', '断'),
        ('選', '选'),
        ('項', '项'),
        ('詢', '询'),
        ('問', '问'),
        ('變', '变'),
        ('為', '为'),
        ('實', '实'),
        ('際', '际'),
        ('數', '数'),
        ('據', '据'),
        ('書', '书'),
        ('經', '经'),
        ('驗', '验'),
        ('學', '学'),
        ('習', '习'),
        ('圖', '图'),
        ('視', '视'),
        ('頻', '频'),
        ('響', '响'),
        ('體', '体'),
        ('關', '关'),
        ('於', '于'),
        ('詳', '详'),
        ('細', '细'),
        ('節', '节'),
        ('訊', '讯'),
        ('標', '标'),
        ('題', '题'),
        ('類', '类'),
        ('別', '别'),
        ('籤', '签'),
        ('記', '记'),
        ('錄', '录'),
        ('購', '购'),
        ('買', '买'),
        ('賣', '卖'),
        ('換', '换'),
        ('贈', '赠'),
        ('獲', '获'),
        ('積', '积'),
        ('兌', '兑'),
        ('獎', '奖'),
        ('勵', '励'),
        ('設', '设'),
        ('調', '调'),
        ('確', '确'),
        ('認', '认'),
        ('錯', '错'),
        ('誤', '误'),
        ('敗', '败'),
        ('異', '异'),
        ('績', '绩'),
        ('評', '评'),
        ('價', '价'),
        ('測', '测'),
        ('試', '试'),
        ('證', '证'),
        ('報', '报'),
        ('導', '导'),
        ('發', '发'),
        ('現', '现'),
        ('決', '决'),
        ('辦', '办'),
        ('規', '规'),
        ('則', '则'),
        ('條', '条'),
        ('約', '约'),
        ('義', '义'),
        ('務', '务'),
        ('責', '责'),
        ('任', '任'),
        ('權', '权'),
        ('協', '协'),
        ('議', '议'),
        ('檔', '档'),
        ('資', '资'),
        ('料', '料'),
        ('夾', '夹'),
        ('源', '源'),
        ('庫', '库'),
        ('網', '网'),
        ('頁', '页'),
        ('鏈', '链'),
        ('址', '址'),
        ('域', '域'),
        ('輸', '输'),
        ('層', '层'),
        ('級', '级'),
        ('結', '结'),
        ('構', '构'),
        ('計', '计'),
        ('劃', '划'),
        ('範', '范'),
        ('圍', '围'),
        ('場', '场'),
        ('業', '业'),
        ('邏', '逻'),
        ('輯', '辑'),
        ('運', '运'),
        ('處', '处'),
        ('儲', '储'),
        ('備', '备'),
        ('還', '还'),
        ('復', '复'),
        ('製', '制'),
        ('編', '编'),
        ('刪', '删'),
        ('檢', '检'),
        ('過', '过'),
        ('濾', '滤'),
        ('統', '统'),
        ('畫', '画'),
        ('顯', '显'),
        ('隱', '隐'),
        ('展', '展'),
        ('收', '收'),
        ('縮', '缩'),
        ('寬', '宽'),
        ('長', '长'),
        ('淺', '浅'),
        ('輕', '轻'),
        ('強', '强'),
        ('優', '优'),
        ('壞', '坏'),
        ('對', '对'),
        ('虛', '虚'),
        ('滿', '满'),
        ('遠', '远'),
        ('內', '内'),
        ('後', '后'),
        ('間', '间'),
        ('週', '周'),
        ('環', '环'),
        ('境', '境'),
        ('界', '界'),
        ('線', '线'),
        ('點', '点'),
        ('質', '质'),
        ('值', '值'),
        ('號', '号'),
        ('詞', '词'),
        ('種', '种'),
        ('樣', '样'),
        ('狀', '状'),
        ('態', '态'),
        ('況', '况'),
        ('轉', '转'),
        ('動', '动'),
        ('靜', '静'),
        ('進', '进'),
        ('啟', '启'),
        ('停', '停'),
        ('遞', '递'),
        ('達', '达'),
        ('歷', '历'),
        ('階', '阶'),
        ('驟', '骤'),
        ('輪', '轮'),
        ('迴', '回'),
        ('圈', '圈'),
        ('循', '循')
    ];
    s.chars()
        .map(|c| MAP.iter().find(|(t, _)| *t == c).map(|(_, s)| *s).unwrap_or(c))
        .collect()
}

fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| {
            // ァ..=ヶ
            if ('\u{30A1}'..='\u{30F6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn remove_special_chars(s: &str) -> String {
    special_chars_re().replace_all(s, "").into_owned()
}

pub fn normalize_string(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let s = s.to_lowercase();
    let s = katakana_to_hiragana(&s);
    let s = traditional_to_simple(&s);
    remove_special_chars(&s)
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

    #[test]
    fn normalize_katakana_and_traditional() {
        assert_eq!(normalize_string("カタカナ"), normalize_string("かたかな"));
        assert_eq!(normalize_string("夢想"), "梦想");
        assert_eq!(normalize_string("Hello World!"), "helloworld");
    }
}
