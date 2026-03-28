package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"time"

	hclog "github.com/hashicorp/go-hclog"
)

const (
	DefaultAliasDataUrl = "https://raw.githubusercontent.com/moe-sekai/MoeSekai-Hub/main/data/music_alias/music_aliases.json"
	DefaultCacheTTL     = 1 * time.Hour
	DefaultCacheDir     = "/tmp/nyanyabot"
	CacheFileName       = "pjsk-song-alias-cache.json"
)

// NewAliasManager 创建新的别名管理器
func NewAliasManager(dataUrl, cacheDir string, cacheTTL time.Duration) *AliasManager {
	if dataUrl == "" {
		dataUrl = DefaultAliasDataUrl
	}
	if cacheDir == "" {
		cacheDir = DefaultCacheDir
	}
	if cacheTTL <= 0 {
		cacheTTL = DefaultCacheTTL
	}

	am := &AliasManager{
		data:       &AliasData{},
		normalized: make(map[string]int),
		musicMap:   make(map[int]*MusicAlias),
		cacheDir:   cacheDir,
		cacheTTL:   cacheTTL,
		dataUrl:    dataUrl,
	}

	return am
}

// Load 加载别名数据（优先从缓存读取）
func (am *AliasManager) Load() error {
	am.mu.Lock()
	defer am.mu.Unlock()

	// 尝试从缓存加载
	if am.isCacheValid() {
		if err := am.loadFromCache(); err == nil {
			hclog.L().Info("[AliasManager] 从缓存加载别名数据成功", "count", len(am.data.Musics))
			return nil
		}
	}

	// 从远程加载
	if err := am.loadFromURL(); err != nil {
		hclog.L().Warn("[AliasManager] 从远程加载失败，尝试使用缓存", "error", err)
		// 尝试使用过期的缓存
		if err := am.loadFromCache(); err != nil {
			return fmt.Errorf("远程加载失败且无可用缓存: %w", err)
		}
		return nil
	}

	// 保存到缓存
	if err := am.saveToCache(); err != nil {
		hclog.L().Warn("[AliasManager] 保存缓存失败", "error", err)
	}

	return nil
}

// loadFromURL 从远程URL加载别名数据
func (am *AliasManager) loadFromURL() error {
	hclog.L().Info("[AliasManager] 从远程加载别名数据", "url", am.dataUrl)

	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Get(am.dataUrl)
	if err != nil {
		return fmt.Errorf("HTTP请求失败: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("HTTP状态码: %d", resp.StatusCode)
	}

	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("读取响应体失败: %w", err)
	}

	var aliasData AliasData
	if err := json.Unmarshal(data, &aliasData); err != nil {
		return fmt.Errorf("解析JSON失败: %w", err)
	}

	am.data = &aliasData
	am.lastLoad = time.Now()
	am.buildIndex()

	hclog.L().Info("[AliasManager] 远程加载成功", "count", len(am.data.Musics))
	return nil
}

// loadFromCache 从本地缓存加载
func (am *AliasManager) loadFromCache() error {
	cachePath := am.getCachePath()
	data, err := os.ReadFile(cachePath)
	if err != nil {
		return fmt.Errorf("读取缓存文件失败: %w", err)
	}

	var aliasData AliasData
	if err := json.Unmarshal(data, &aliasData); err != nil {
		return fmt.Errorf("解析缓存JSON失败: %w", err)
	}

	am.data = &aliasData
	am.buildIndex()

	return nil
}

// saveToCache 保存到本地缓存
func (am *AliasManager) saveToCache() error {
	cachePath := am.getCachePath()

	// 确保目录存在
	if err := os.MkdirAll(filepath.Dir(cachePath), 0755); err != nil {
		return fmt.Errorf("创建缓存目录失败: %w", err)
	}

	data, err := json.MarshalIndent(am.data, "", "  ")
	if err != nil {
		return fmt.Errorf("序列化JSON失败: %w", err)
	}

	if err := os.WriteFile(cachePath, data, 0644); err != nil {
		return fmt.Errorf("写入缓存文件失败: %w", err)
	}

	hclog.L().Info("[AliasManager] 缓存已保存", "path", cachePath)
	return nil
}

// isCacheValid 检查缓存是否有效（未过期）
func (am *AliasManager) isCacheValid() bool {
	cachePath := am.getCachePath()
	info, err := os.Stat(cachePath)
	if err != nil {
		return false
	}

	return time.Since(info.ModTime()) < am.cacheTTL
}

// getCachePath 获取缓存文件路径
func (am *AliasManager) getCachePath() string {
	return filepath.Join(am.cacheDir, CacheFileName)
}

// buildIndex 构建标准化索引和映射
func (am *AliasManager) buildIndex() {
	am.normalized = make(map[string]int)
	am.musicMap = make(map[int]*MusicAlias)

	for i := range am.data.Musics {
		music := &am.data.Musics[i]
		am.musicMap[music.MusicID] = music

		// 索引标题
		normalizedTitle := normalizeString(music.Title)
		am.normalized[normalizedTitle] = music.MusicID

		// 索引别名
		for _, alias := range music.Aliases {
			normalizedAlias := normalizeString(alias)
			am.normalized[normalizedAlias] = music.MusicID
		}

		// 索引数字ID（作为字符串）
		idStr := fmt.Sprintf("%d", music.MusicID)
		am.normalized[idStr] = music.MusicID
	}
}

// GetMusicByID 根据ID获取歌曲信息
func (am *AliasManager) GetMusicByID(id int) *MusicAlias {
	am.mu.RLock()
	defer am.mu.RUnlock()
	return am.musicMap[id]
}

// GetData 获取别名数据（用于匹配）
func (am *AliasManager) GetData() *AliasData {
	am.mu.RLock()
	defer am.mu.RUnlock()
	return am.data
}

// GetNormalizedIndex 获取标准化索引
func (am *AliasManager) GetNormalizedIndex() map[string]int {
	am.mu.RLock()
	defer am.mu.RUnlock()
	return am.normalized
}

// RefreshIfNeeded 检查并刷新数据（如果需要）
func (am *AliasManager) RefreshIfNeeded() error {
	am.mu.RLock()
	lastLoad := am.lastLoad
	am.mu.RUnlock()

	if time.Since(lastLoad) < am.cacheTTL {
		return nil
	}

	return am.Load()
}
