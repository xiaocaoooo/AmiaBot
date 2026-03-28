package main

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"sync"
	"time"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
)

type PJSKSong struct {
	mu           sync.RWMutex
	cfg          config
	aliasManager *AliasManager
}

type config struct {
	AmiabotPages  string `json:"amiabot_pages"`
	DefaultServer string `json:"default_server"`
	AliasDataUrl  string `json:"alias_data_url"`
	AliasCacheDir string `json:"alias_cache_dir"`
	AliasCacheTTL int    `json:"alias_cache_ttl"` // 秒
}

func (e *PJSKSong) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址；为空则无法生成截图 URL"},
			"default_server":{"type":"string","description":"默认服务器 (jp/cn/en/tw/kr)，不填时为 jp"},
			"alias_data_url":{"type":"string","description":"别名数据URL，默认使用 MoeSekai-Hub 的数据"},
			"alias_cache_dir":{"type":"string","description":"别名缓存目录，默认 /tmp/nyanyabot"},
			"alias_cache_ttl":{"type":"integer","description":"缓存有效期（秒），默认 3600"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":"","default_server":"jp","alias_data_url":"https://raw.githubusercontent.com/moe-sekai/MoeSekai-Hub/main/data/music_alias/music_aliases.json","alias_cache_dir":"/tmp/nyanyabot","alias_cache_ttl":3600}`)
	return papi.Descriptor{
		Name:         "Amiabot PJSK Song",
		PluginID:     "external.amiabot-pjsk-song",
		Version:      "0.2.0",
		Author:       "nyanyabot",
		Description:  "PJSK 歌曲查询插件，支持别名模糊匹配，发送截图",
		Dependencies: []string{"external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot PJSK Song plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "pjsk-song",
				ID:          "cmd.pjsk-song",
				Description: "PJSK 歌曲查询（如 songtyw, jpsong消失, song1）",
				Pattern:     `^(?i)(?:(?P<server>cn|jp|tw|en|kr))?song(?P<name>.+)$`,
				MatchRaw:    true,
				Handler:     "HandlePJSKSong",
			},
		},
	}, nil
}

func (e *PJSKSong) Configure(ctx context.Context, configJSON json.RawMessage) error {
	_ = ctx
	cfg := config{
		DefaultServer: "jp",
		AliasDataUrl:  DefaultAliasDataUrl,
		AliasCacheDir: DefaultCacheDir,
		AliasCacheTTL: int(DefaultCacheTTL.Seconds()),
	}
	if len(configJSON) > 0 {
		_ = json.Unmarshal(configJSON, &cfg)
	}
	defSrv := strings.TrimSpace(cfg.DefaultServer)
	if defSrv == "" {
		defSrv = "jp"
	}

	e.mu.Lock()
	e.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	e.cfg.DefaultServer = defSrv
	e.cfg.AliasDataUrl = strings.TrimSpace(cfg.AliasDataUrl)
	e.cfg.AliasCacheDir = strings.TrimSpace(cfg.AliasCacheDir)
	e.cfg.AliasCacheTTL = cfg.AliasCacheTTL
	e.mu.Unlock()

	// 初始化别名管理器并加载数据
	cacheTTL := time.Duration(cfg.AliasCacheTTL) * time.Second
	if cacheTTL <= 0 {
		cacheTTL = DefaultCacheTTL
	}
	e.aliasManager = NewAliasManager(cfg.AliasDataUrl, cfg.AliasCacheDir, cacheTTL)

	go func() {
		if err := e.aliasManager.Load(); err != nil {
			hclog.L().Error("[Song] 加载别名数据失败", "error", err)
		}
	}()

	return nil
}

func (e *PJSKSong) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (e *PJSKSong) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	hclog.L().Info("[Song] Handle() CALLED", "listenerID", listenerID)
	if listenerID == "cmd.pjsk-song" {
		return e.handlePJSKSong(ctx, eventRaw, match)
	}
	return papi.HandleResult{}, nil
}

func (e *PJSKSong) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

func (e *PJSKSong) handlePJSKSong(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	log.Info("[Song] ===== 开始处理 =====")

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[Song] 解析事件失败", "error", err)
		return papi.HandleResult{}, nil
	}
	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]
	rawMessage, _ := evt["raw_message"].(string)

	// recover
	defer func() {
		if r := recover(); r != nil {
			err := fmt.Errorf("panic: %v", r)
			log.Error("[Song] panic", "error", err)
			sendError(transport.Host(), msgType, groupID, userID, "❌ 歌曲查询异常", err)
		}
	}()

	log.Info("[Song] 收到消息", "raw_message", rawMessage, "msg_type", msgType)

	host := transport.Host()
	if host == nil {
		log.Warn("[Song] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	// 解析参数并进行模糊匹配
	server, results := e.parseArgs(rawMessage, match)
	log.Info("[Song] 解析结果", "server", server, "results_count", len(results))

	if server == "" || len(results) == 0 {
		log.Warn("[Song] 未找到匹配的歌曲")
		sendText(host, msgType, groupID, userID, "❌ 未找到匹配的歌曲，请尝试其他关键词")
		return papi.HandleResult{}, nil
	}

	e.mu.RLock()
	pagesHost := e.cfg.AmiabotPages
	e.mu.RUnlock()

	if pagesHost == "" {
		log.Warn("[Song] amiabot_pages 未配置，终止")
		sendText(host, msgType, groupID, userID, "❌ 服务未配置")
		return papi.HandleResult{}, nil
	}

	// 获取第一个结果（最高匹配度）
	topResult := results[0]
	id := fmt.Sprintf("%d", topResult.MusicID)

	// 构建页面URL并发送截图
	pageURL := buildPagesURL(pagesHost, "/pjsk/music", map[string]string{"server": server, "id": id})
	log.Info("[Song] 页面 URL", "url", pageURL)

	log.Info("[Song] 调用截图插件...")
	screenshotURL, screenshotErr := buildScreenshotViaPlugin(host, pageURL)
	log.Info("[Song] 截图 URL", "url", screenshotURL, "error", screenshotErr)
	if screenshotErr != nil {
		log.Warn("[Song] 截图失败", "error", screenshotErr)
		sendError(host, msgType, groupID, userID, "❌ 截图失败", screenshotErr)
		return papi.HandleResult{}, nil
	}

	blobID := fmt.Sprintf("pjsk-song-%s-%s-%d", server, id, time.Now().Unix())
	log.Info("[Song] 调用 blobserver 上传...", "blob_id", blobID)
	if uploaded := uploadViaBlobPlugin(ctx, host, screenshotURL, blobID, "image"); uploaded != "" {
		log.Info("[Song] 上传成功", "url", uploaded)
		screenshotURL = uploaded
	}

	log.Info("[Song] 发送图片消息...")
	_ = sendImage(host, msgType, groupID, userID, screenshotURL)

	// 如果有多个匹配结果，发送候选列表
	if len(results) > 1 {
		candidateMsg := buildCandidateMessage(results[1:], server)
		log.Info("[Song] 发送候选列表", "count", len(results)-1)
		sendText(host, msgType, groupID, userID, candidateMsg)
	}

	log.Info("[Song] ===== 处理完成 =====")
	return papi.HandleResult{}, nil
}

// parseArgs 解析参数并进行模糊匹配
// 返回 server 和匹配结果列表
func (e *PJSKSong) parseArgs(rawMessage string, match *papi.CommandMatch) (server string, results []MatchResult) {
	re := regexp.MustCompile(`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?song(?P<name>.+)$`)
	m := re.FindStringSubmatch(rawMessage)
	if len(m) >= 3 {
		server = strings.ToLower(strings.TrimSpace(m[1]))
		name := strings.TrimSpace(m[2])
		if name != "" && e.aliasManager != nil {
			results = FuzzySearch(name, e.aliasManager)
		}
	}
	if server == "" {
		e.mu.RLock()
		server = e.cfg.DefaultServer
		e.mu.RUnlock()
	}
	return
}

// buildCandidateMessage 构建候选列表消息
func buildCandidateMessage(results []MatchResult, server string) string {
	var sb strings.Builder
	sb.WriteString("🎵 找到多个匹配，输入编号可快速查询：\n")

	maxShow := 3
	if len(results) < maxShow {
		maxShow = len(results)
	}

	for i := 0; i < maxShow; i++ {
		r := results[i]
		confidencePercent := int(r.Confidence * 100)
		serverPrefix := ""
		if server != "" && server != "jp" {
			serverPrefix = server
		}
		sb.WriteString(fmt.Sprintf("  • %ssong%d - %s (%d%%)\n", serverPrefix, r.MusicID, r.Title, confidencePercent))
	}

	return sb.String()
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &PJSKSong{}},
		},
	})
}
