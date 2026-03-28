package main

import (
	"fmt"
	"strings"
	"testing"
)

func TestNormalizeString(t *testing.T) {
	tests := []struct {
		input    string
		contains string // 检查结果是否包含这个子串
	}{
		{"TYW", "tyw"},
		{"Tell Your World", "tell"},           // 移除空格后应包含 tell
		{"夢開始的地方", "梦"},                       // 繁体转简体应包含"梦"
		{"ロキ", "ろき"},                          // 片假名转平假名
		{"Tell Your World!", "tellyourworld"}, // 移除特殊字符
		{"  tyw  ", "tyw"},                    // 移除空格
	}

	for _, tt := range tests {
		result := normalizeString(tt.input)
		if !strings.Contains(result, tt.contains) {
			t.Errorf("normalizeString(%q) = %q, should contain %q", tt.input, result, tt.contains)
		}
	}
}

func TestKatakanaToHiragana(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"ロキ", "ろき"}, // 片假名转平假名
		{"テオ", "てお"}, // 片假名转平假名
		{"ハッピーシンセサイザ", "はっぴーしんせさいざ"}, // 片假名转平假名
		{"タイムマシン", "たいむましん"},         // 片假名转平假名
		{"ヒバナ", "ひばな"},               // 片假名转平假名
		{"あいうえお", "あいうえお"},           // 平假名保持不变
		{"AIUEO", "AIUEO"},           // 英文保持不变（大小写由 normalizeString 处理）
		{"混合Testロキ", "混合Testろき"},     // 混合输入
	}

	for _, tt := range tests {
		result := katakanaToHiragana(tt.input)
		if result != tt.expected {
			t.Errorf("katakanaToHiragana(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestIsSubsequence(t *testing.T) {
	tests := []struct {
		query  string
		target string
		expect bool
	}{
		{"tyw", "tellyourworld", true},
		{"tywd", "tellyourworld", true}, // d 在 "world" 中存在
		{"rk", "roki", true},
		{"abc", "axbxc", true},
		{"abc", "acb", false}, // 顺序不对
		{"", "anything", true},
		{"something", "", false},
		{"xyz", "tellyourworld", false}, // xyz 都不存在
	}

	for _, tt := range tests {
		result := isSubsequence(tt.query, tt.target)
		if result != tt.expect {
			t.Errorf("isSubsequence(%q, %q) = %v, want %v", tt.query, tt.target, result, tt.expect)
		}
	}
}

func TestCalculatePrefixScore(t *testing.T) {
	tests := []struct {
		query    string
		target   string
		minScore float64
		maxScore float64
	}{
		{"tyw", "tyw", 1.0, 1.0},            // 精确匹配
		{"tell", "tellyourworld", 0.1, 1.0}, // 前缀匹配
		{"your", "tellyourworld", 0.1, 1.0}, // 包含匹配
		{"xyz", "tellyourworld", 0.0, 0.0},  // 无匹配
	}

	for _, tt := range tests {
		result := calculatePrefixScore(tt.query, tt.target)
		if result < tt.minScore || result > tt.maxScore {
			t.Errorf("calculatePrefixScore(%q, %q) = %v, want between %v and %v",
				tt.query, tt.target, result, tt.minScore, tt.maxScore)
		}
	}
}

func TestCalculateSubsequenceScore(t *testing.T) {
	tests := []struct {
		query    string
		target   string
		minScore float64
	}{
		{"tyw", "tellyourworld", 0.1}, // 是子序列
		{"xyz", "tellyourworld", 0.0}, // 不是子序列
		{"rk", "roki", 0.1},           // 是子序列
	}

	for _, tt := range tests {
		result := calculateSubsequenceScore(tt.query, tt.target)
		if result < tt.minScore {
			t.Errorf("calculateSubsequenceScore(%q, %q) = %v, want >= %v",
				tt.query, tt.target, result, tt.minScore)
		}
	}
}

func TestFuzzySearch(t *testing.T) {
	// 创建测试用的别名管理器
	am := &AliasManager{
		data: &AliasData{
			Musics: []MusicAlias{
				{
					MusicID: 1,
					Title:   "Tell Your World",
					Aliases: []string{"tyw", "告诉你的世界", "梦开始的地方", "google chrome"},
				},
				{
					MusicID: 2,
					Title:   "ロキ",
					Aliases: []string{"roki", "rk", "罗辑"},
				},
			},
		},
		normalized: make(map[string]int),
		musicMap:   make(map[int]*MusicAlias),
	}

	// 构建索引（模拟 buildIndex 的逻辑）
	for i := range am.data.Musics {
		music := &am.data.Musics[i]
		am.musicMap[music.MusicID] = music

		// 索引标题
		am.normalized[normalizeString(music.Title)] = music.MusicID

		// 索引别名
		for _, alias := range music.Aliases {
			am.normalized[normalizeString(alias)] = music.MusicID
		}

		// 索引数字ID（作为字符串）
		idStr := fmt.Sprintf("%d", music.MusicID)
		am.normalized[idStr] = music.MusicID
	}

	tests := []struct {
		query       string
		expectFound bool
		expectID    int
	}{
		{"tyw", true, 1},             // 精确别名匹配
		{"rk", true, 2},              // 精确别名匹配
		{"1", true, 1},               // 数字ID匹配
		{"Tell Your World", true, 1}, // 标题匹配
		{"tell", true, 1},            // 前缀匹配
	}

	for _, tt := range tests {
		results := FuzzySearch(tt.query, am)
		if tt.expectFound {
			if len(results) == 0 {
				t.Errorf("FuzzySearch(%q) expected found, got empty", tt.query)
				continue
			}
			if results[0].MusicID != tt.expectID {
				t.Errorf("FuzzySearch(%q) = %d, want %d", tt.query, results[0].MusicID, tt.expectID)
			}
		} else {
			if len(results) > 0 {
				t.Errorf("FuzzySearch(%q) expected empty, got %d results", tt.query, len(results))
			}
		}
	}
}
