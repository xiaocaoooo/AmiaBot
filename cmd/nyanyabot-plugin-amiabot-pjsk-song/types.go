package main

import (
	"sync"
	"time"
)

// MusicAlias 单首歌曲的别名数据
type MusicAlias struct {
	MusicID int      `json:"music_id"`
	Title   string   `json:"title"`
	Aliases []string `json:"aliases"`
}

// AliasData 别名数据集
type AliasData struct {
	GeneratedAt string       `json:"generated_at"`
	Musics      []MusicAlias `json:"musics"`
}

// MatchResult 匹配结果
type MatchResult struct {
	MusicID    int
	Title      string
	Confidence float64 // 0.0 - 1.0
	MatchedKey string  // 匹配到的名称/别名
}

// AliasManager 别名管理器
type AliasManager struct {
	mu         sync.RWMutex
	data       *AliasData
	normalized map[string]int      // 标准化名称 -> music_id 的快速索引
	musicMap   map[int]*MusicAlias // music_id -> MusicAlias 的映射
	lastLoad   time.Time
	cacheDir   string
	cacheTTL   time.Duration
	dataUrl    string
}
