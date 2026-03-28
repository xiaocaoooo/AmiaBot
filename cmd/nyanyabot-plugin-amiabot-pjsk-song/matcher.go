package main

import (
	"regexp"
	"sort"
	"strings"

	hclog "github.com/hashicorp/go-hclog"
)

// 匹配权重常量
const (
	WeightExact       = 1.0 // 精确匹配
	WeightPrefix      = 0.8 // 前缀匹配
	WeightContains    = 0.7 // 包含匹配
	WeightSubsequence = 0.5 // 子序列匹配
)

// FuzzySearch 模糊搜索入口
// 返回匹配结果列表，按置信度降序排列
func FuzzySearch(query string, am *AliasManager) []MatchResult {
	if am == nil || query == "" {
		return nil
	}

	normalizedQuery := normalizeString(query)
	if normalizedQuery == "" {
		return nil
	}

	data := am.GetData()
	if data == nil || len(data.Musics) == 0 {
		return nil
	}

	// 使用 map 去重，key 为 music_id
	resultMap := make(map[int]*MatchResult)

	// Level 1: 精确匹配
	am.mu.RLock()
	normalized := am.normalized
	musicMap := am.musicMap
	am.mu.RUnlock()

	if musicID, ok := normalized[normalizedQuery]; ok {
		if music, exists := musicMap[musicID]; exists {
			resultMap[musicID] = &MatchResult{
				MusicID:    musicID,
				Title:      music.Title,
				Confidence: WeightExact,
				MatchedKey: query,
			}
		}
	}

	// Level 2: 前缀/包含匹配
	for _, music := range data.Musics {
		if _, exists := resultMap[music.MusicID]; exists {
			continue // 已匹配，跳过
		}

		// 检查标题
		normalizedTitle := normalizeString(music.Title)
		confidence := calculatePrefixScore(normalizedQuery, normalizedTitle)
		if confidence > 0 {
			resultMap[music.MusicID] = &MatchResult{
				MusicID:    music.MusicID,
				Title:      music.Title,
				Confidence: confidence,
				MatchedKey: music.Title,
			}
			continue
		}

		// 检查别名
		for _, alias := range music.Aliases {
			normalizedAlias := normalizeString(alias)
			confidence := calculatePrefixScore(normalizedQuery, normalizedAlias)
			if confidence > 0 {
				resultMap[music.MusicID] = &MatchResult{
					MusicID:    music.MusicID,
					Title:      music.Title,
					Confidence: confidence,
					MatchedKey: alias,
				}
				break
			}
		}
	}

	// Level 3: 子序列匹配（仅当结果少于5个时）
	if len(resultMap) < 5 {
		for _, music := range data.Musics {
			if _, exists := resultMap[music.MusicID]; exists {
				continue
			}

			// 检查标题
			normalizedTitle := normalizeString(music.Title)
			if confidence := calculateSubsequenceScore(normalizedQuery, normalizedTitle); confidence > 0 {
				resultMap[music.MusicID] = &MatchResult{
					MusicID:    music.MusicID,
					Title:      music.Title,
					Confidence: confidence,
					MatchedKey: music.Title,
				}
				continue
			}

			// 检查别名
			for _, alias := range music.Aliases {
				normalizedAlias := normalizeString(alias)
				if confidence := calculateSubsequenceScore(normalizedQuery, normalizedAlias); confidence > 0 {
					resultMap[music.MusicID] = &MatchResult{
						MusicID:    music.MusicID,
						Title:      music.Title,
						Confidence: confidence,
						MatchedKey: alias,
					}
					break
				}
			}
		}
	}

	// 转换为切片并排序
	results := make([]MatchResult, 0, len(resultMap))
	for _, result := range resultMap {
		results = append(results, *result)
	}

	// 按置信度降序排序
	sort.Slice(results, func(i, j int) bool {
		return results[i].Confidence > results[j].Confidence
	})

	hclog.L().Debug("[Matcher] 搜索完成", "query", query, "results", len(results))
	return results
}

// normalizeString 标准化字符串
// - 转小写
// - 片假名转平假名
// - 繁体转简体（简单映射）
// - 移除空格和特殊符号
func normalizeString(s string) string {
	if s == "" {
		return ""
	}

	// 转小写
	s = strings.ToLower(s)

	// 片假名转平假名
	s = katakanaToHiragana(s)

	// 繁体转简体
	s = traditionalToSimple(s)

	// 移除特殊字符（保留字母、数字、中文、日文等）
	s = removeSpecialChars(s)

	return s
}

// katakanaToHiragana 片假名转平假名
// 日语中片假名和平假名是一一对应的，统一转换为平假名便于匹配
func katakanaToHiragana(s string) string {
	// 片假名 Unicode 范围: U+30A0 - U+30FF
	// 平假名 Unicode 范围: U+3040 - U+309F
	// 片假名到平假名的偏移量: 0x30A0 - 0x3040 = 0x60 (96)
	runes := []rune(s)
	for i, r := range runes {
		// 检查是否在片假名范围内（ excluding ヷヺ 等）
		if r >= 'ァ' && r <= 'ヶ' {
			// 片假名转平假名：偏移量为 0x60
			runes[i] = r - 0x60
		}
		// 处理长音符号（ー）保持不变，因为它在两种假名中都存在
	}
	return string(runes)
}

// removeSpecialChars 移除特殊字符
func removeSpecialChars(s string) string {
	// 只保留：字母、数字、中文、日文假名、韩文
	// 匹配需要移除的字符：空格、标点符号、特殊符号等
	re := regexp.MustCompile(`[\s\p{P}\p{S}−\-_！？。、！？，；：（）【】「」『』〈〉《"]+`)
	s = re.ReplaceAllString(s, "")
	return s
}

// traditionalToSimple 繁体转简体（简单映射）
// 使用常见的繁简对照表
func traditionalToSimple(s string) string {
	// 常见繁简对照映射（只包含繁简不同的字）
	mapping := map[rune]rune{
		'夢': '梦', '開': '开', '傳': '传', '說': '说', '訴': '诉',
		'時': '时', '機': '机', '樂': '乐', '斷': '断', '選': '选',
		'項': '项', '詢': '询', '問': '问', '變': '变', '為': '为',
		'實': '实', '際': '际', '數': '数', '據': '据', '書': '书',
		'經': '经', '驗': '验', '學': '学', '習': '习', '圖': '图',
		'視': '视', '頻': '频', '響': '响', '體': '体', '關': '关',
		'於': '于', '詳': '详', '細': '细', '節': '节', '訊': '讯',
		'標': '标', '題': '题', '類': '类', '別': '别', '籤': '签',
		'記': '记', '錄': '录', '購': '购', '買': '买', '賣': '卖',
		'換': '换', '贈': '赠', '獲': '获', '積': '积', '兌': '兑',
		'獎': '奖', '勵': '励', '設': '设', '調': '调', '確': '确',
		'認': '认', '錯': '错', '誤': '误', '敗': '败', '異': '异',
		'績': '绩', '評': '评', '價': '价', '測': '测', '試': '试',
		'證': '证', '報': '报', '導': '导', '發': '发', '現': '现',
		'決': '决', '辦': '办', '規': '规', '則': '则', '條': '条',
		'約': '约', '義': '义', '務': '务', '責': '责', '任': '任',
		'權': '权', '協': '协', '議': '议', '檔': '档', '資': '资',
		'料': '料', '夾': '夹', '源': '源', '庫': '库', '網': '网',
		'頁': '页', '鏈': '链', '址': '址', '域': '域', '輸': '输',
		'層': '层', '級': '级', '結': '结', '構': '构', '計': '计',
		'劃': '划', '範': '范', '圍': '围', '場': '场', '業': '业',
		'邏': '逻', '輯': '辑', '運': '运', '處': '处', '儲': '储',
		'備': '备', '還': '还', '復': '复', '製': '制', '編': '编',
		'刪': '删', '檢': '检', '過': '过', '濾': '滤', '統': '统',
		'畫': '画', '顯': '显', '隱': '隐', '展': '展', '收': '收',
		'縮': '缩', '寬': '宽', '長': '长', '淺': '浅', '輕': '轻',
		'強': '强', '優': '优', '壞': '坏', '對': '对', '虛': '虚',
		'滿': '满', '遠': '远', '內': '内', '後': '后', '間': '间',
		'週': '周', '環': '环', '境': '境', '界': '界', '線': '线',
		'點': '点', '質': '质', '值': '值', '號': '号', '詞': '词',
		'種': '种', '樣': '样', '狀': '状', '態': '态', '況': '况',
		'轉': '转', '動': '动', '靜': '静', '進': '进', '啟': '启',
		'停': '停', '遞': '递', '達': '达', '歷': '历', '階': '阶',
		'驟': '骤', '輪': '轮', '迴': '回', '圈': '圈', '循': '循',
	}

	runes := []rune(s)
	for i, r := range runes {
		if simple, ok := mapping[r]; ok {
			runes[i] = simple
		}
	}
	return string(runes)
}

// calculatePrefixScore 计算前缀/包含匹配分数
func calculatePrefixScore(query, target string) float64 {
	if query == "" || target == "" {
		return 0
	}

	// 精确匹配（已在调用前检查）
	if query == target {
		return WeightExact
	}

	// 前缀匹配
	if strings.HasPrefix(target, query) {
		// 根据匹配长度占比计算分数
		ratio := float64(len(query)) / float64(len(target))
		return WeightPrefix * ratio
	}

	// 包含匹配
	if strings.Contains(target, query) {
		ratio := float64(len(query)) / float64(len(target))
		return WeightContains * ratio
	}

	return 0
}

// calculateSubsequenceScore 计算子序列匹配分数
func isSubsequence(query, target string) bool {
	if len(query) == 0 {
		return true
	}
	if len(target) == 0 {
		return false
	}

	queryRunes := []rune(query)
	targetRunes := []rune(target)

	i := 0
	for _, t := range targetRunes {
		if i < len(queryRunes) && queryRunes[i] == t {
			i++
		}
	}

	return i == len(queryRunes)
}

// calculateSubsequenceScore 计算子序列匹配分数
func calculateSubsequenceScore(query, target string) float64 {
	if !isSubsequence(query, target) {
		return 0
	}

	queryRunes := []rune(query)
	targetRunes := []rune(target)

	// 基础分数：匹配字符占比
	baseScore := float64(len(queryRunes)) / float64(len(targetRunes))

	// 连续匹配加成
	consecutiveBonus := 0.0
	consecutive := 0
	maxConsecutive := 0

	j := 0
	for _, t := range targetRunes {
		if j < len(queryRunes) && queryRunes[j] == t {
			consecutive++
			if consecutive > maxConsecutive {
				maxConsecutive = consecutive
			}
			j++
		} else {
			consecutive = 0
		}
	}

	if len(queryRunes) > 0 {
		consecutiveBonus = float64(maxConsecutive) / float64(len(queryRunes)) * 0.3
	}

	// 首字母匹配加成
	firstCharBonus := 0.0
	if len(queryRunes) > 0 && len(targetRunes) > 0 && queryRunes[0] == targetRunes[0] {
		firstCharBonus = 0.1
	}

	return WeightSubsequence * (baseScore + consecutiveBonus + firstCharBonus)
}
