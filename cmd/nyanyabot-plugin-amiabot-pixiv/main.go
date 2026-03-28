package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
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

var (
	pixivArtworkPattern = regexp.MustCompile(`(?i)\b(?:https?://)?(?:www\.)?pixiv\.net/(?:[a-z]{2}/)?artworks/(\d+)(?:[?#/][^\s]*)?`)
	pixivHTTPClient     = &http.Client{Timeout: 30 * time.Second}
)

type pixivMediaManifest struct {
	PID       int              `json:"pid"`
	Title     string           `json:"title"`
	Type      string           `json:"type"`
	PageCount int              `json:"page_count"`
	Items     []pixivMediaItem `json:"items"`
}

type pixivMediaItem struct {
	Index int    `json:"index"`
	Kind  string `json:"kind"`
	Path  string `json:"path"`
}

type AmiabotPixiv struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages             string `json:"amiabot_pages"`
		AmiabotPagesDownloadBase string `json:"amiabot_pages_download_base"`
	}
}

func (e *AmiabotPixiv) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址；用于截图页面 URL（需能被 screenshot 服务访问）"},
			"amiabot_pages_download_base":{"type":"string","description":"Amiabot Pages 下载基地址；用于宿主机插件请求 /pixiv/illust/media、/pixiv/image、/pixiv/ugoira/gif（为空时回退 amiabot_pages）"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":"","amiabot_pages_download_base":""}`)

	return papi.Descriptor{
		Name:         "Amiabot Pixiv",
		PluginID:     "external.amiabot-pixiv",
		Version:      "0.2.1",
		Author:       "nyanyabot",
		Description:  "识别 Pixiv 作品链接，发送信息卡与原图",
		Dependencies: []string{"external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot Pixiv plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "pixiv-artwork",
				ID:          "cmd.pixiv-artwork",
				Description: "识别 pixiv.net/artworks/:id 链接并发送截图与原图",
				Pattern:     pixivArtworkPattern.String(),
				MatchRaw:    true,
				Handler:     "HandlePixivArtwork",
			},
		},
	}, nil
}

func (e *AmiabotPixiv) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	cfg := struct {
		AmiabotPages             string `json:"amiabot_pages"`
		AmiabotPagesDownloadBase string `json:"amiabot_pages_download_base"`
	}{}
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}

	e.mu.Lock()
	e.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	e.cfg.AmiabotPagesDownloadBase = strings.TrimSpace(cfg.AmiabotPagesDownloadBase)
	e.mu.Unlock()
	return nil
}

func (e *AmiabotPixiv) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (e *AmiabotPixiv) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	if listenerID == "cmd.pixiv-artwork" {
		return e.handlePixivArtwork(ctx, eventRaw, match)
	}
	return papi.HandleResult{}, nil
}

func (e *AmiabotPixiv) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

func (e *AmiabotPixiv) handlePixivArtwork(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[Pixiv] 解析事件失败", "error", err)
		return papi.HandleResult{}, nil
	}

	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]
	selfID := evt["self_id"]
	rawMessage, _ := evt["raw_message"].(string)
	if selfID == nil {
		selfID = userID
	}
	if selfID == nil {
		selfID = 0
	}

	defer func() {
		if r := recover(); r != nil {
			err := fmt.Errorf("panic: %v", r)
			log.Error("[Pixiv] panic", "error", err)
			sendError(transport.Host(), msgType, groupID, userID, "❌ Pixiv 解析异常", err)
		}
	}()

	host := transport.Host()
	if host == nil {
		log.Warn("[Pixiv] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	pid := extractPixivArtworkID(rawMessage, match)
	if pid == "" {
		log.Info("[Pixiv] 未解析到作品 ID")
		return papi.HandleResult{}, nil
	}

	e.mu.RLock()
	pagesHost := e.cfg.AmiabotPages
	downloadBase := resolvePixivMediaBase(e.cfg.AmiabotPages, e.cfg.AmiabotPagesDownloadBase)
	e.mu.RUnlock()
	if pagesHost == "" {
		log.Warn("[Pixiv] amiabot_pages 未配置")
		sendText(host, msgType, groupID, userID, "❌ 服务未配置")
		return papi.HandleResult{}, nil
	}

	pageURL := buildPixivPageURL(pagesHost, pid)
	if pageURL == "" {
		log.Warn("[Pixiv] 页面 URL 构造失败", "pages_host", pagesHost, "pid", pid)
		sendText(host, msgType, groupID, userID, "❌ 服务未配置")
		return papi.HandleResult{}, nil
	}

	screenshotURL, err := buildScreenshotViaPlugin(host, pageURL)
	if err != nil {
		log.Warn("[Pixiv] 截图失败", "pid", pid, "error", err)
		sendError(host, msgType, groupID, userID, "❌ 截图失败", err)
		return papi.HandleResult{}, nil
	}
	if strings.TrimSpace(screenshotURL) == "" {
		log.Warn("[Pixiv] 截图 URL 为空", "pid", pid)
		sendText(host, msgType, groupID, userID, "❌ 截图失败")
		return papi.HandleResult{}, nil
	}

	cardBlobID := fmt.Sprintf("pixiv-artwork-%s-%d", pid, time.Now().Unix())
	if uploaded := uploadViaBlobPlugin(ctx, host, screenshotURL, cardBlobID, "image"); uploaded != "" {
		screenshotURL = uploaded
	}
	if err := sendImage(host, msgType, groupID, userID, screenshotURL); err != nil {
		log.Warn("[Pixiv] 发送截图失败", "pid", pid, "error", err)
	}

	manifest, err := fetchPixivMediaManifest(ctx, downloadBase, pid)
	if err != nil {
		log.Warn("[Pixiv] 获取原图清单失败", "pid", pid, "error", err)
		sendError(host, msgType, groupID, userID, "❌ 获取原图失败", err)
		return papi.HandleResult{}, nil
	}
	if len(manifest.Items) == 0 {
		log.Info("[Pixiv] 原图清单为空", "pid", pid, "type", manifest.Type)
		sendText(host, msgType, groupID, userID, "⚠️ 未获取到可发送的原图")
		return papi.HandleResult{}, nil
	}

	mediaURLs := resolvePixivMediaURLs(ctx, host, downloadBase, pid, manifest.Items)
	if len(mediaURLs) == 0 {
		log.Warn("[Pixiv] 原图 URL 解析结果为空", "pid", pid)
		sendText(host, msgType, groupID, userID, "⚠️ 未获取到可发送的原图")
		return papi.HandleResult{}, nil
	}

	if len(mediaURLs) == 1 {
		if err := sendImage(host, msgType, groupID, userID, mediaURLs[0]); err != nil {
			log.Warn("[Pixiv] 发送原图失败", "pid", pid, "error", err)
			sendError(host, msgType, groupID, userID, "❌ 发送原图失败", err)
		}
		return papi.HandleResult{}, nil
	}

	forwardNodes := buildPixivForwardNodes(mediaURLs, selfID, "AmiaBot Pixiv")
	if err := sendForward(host, msgType, groupID, userID, forwardNodes); err != nil {
		log.Warn("[Pixiv] 发送合并转发失败", "pid", pid, "error", err)
		sendError(host, msgType, groupID, userID, "❌ 发送原图失败", err)
	}
	return papi.HandleResult{}, nil
}

func extractPixivArtworkID(rawMessage string, match *papi.CommandMatch) string {
	if match != nil && len(match.Groups) > 0 {
		if pid := strings.TrimSpace(match.Groups[0]); pid != "" {
			return pid
		}
	}

	m := pixivArtworkPattern.FindStringSubmatch(rawMessage)
	if len(m) >= 2 {
		return strings.TrimSpace(m[1])
	}
	return ""
}

func buildPixivPageURL(pagesHost string, pid string) string {
	pid = strings.TrimSpace(pid)
	if pid == "" {
		return ""
	}
	return buildPagesURL(pagesHost, "/pixiv/illust/info", map[string]string{"pid": pid})
}

func resolvePixivMediaBase(pagesHost string, downloadBase string) string {
	downloadBase = strings.TrimSpace(downloadBase)
	if downloadBase != "" {
		return downloadBase
	}
	return strings.TrimSpace(pagesHost)
}

func fetchPixivMediaManifest(ctx context.Context, pagesHost string, pid string) (*pixivMediaManifest, error) {
	manifestURL := buildPagesURL(pagesHost, "/pixiv/illust/media", map[string]string{"pid": strings.TrimSpace(pid)})
	if manifestURL == "" {
		return nil, fmt.Errorf("原图清单 URL 构造失败")
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, manifestURL, nil)
	if err != nil {
		return nil, fmt.Errorf("创建原图清单请求失败: %w", err)
	}
	resp, err := pixivHTTPClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("请求原图清单失败: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("读取原图清单失败: %w", err)
	}
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("原图清单请求失败: HTTP %d %s", resp.StatusCode, extractErrorMessage(body))
	}

	var manifest pixivMediaManifest
	if err := json.Unmarshal(body, &manifest); err != nil {
		return nil, fmt.Errorf("解析原图清单失败: %w", err)
	}
	return &manifest, nil
}

func resolvePixivMediaURLs(ctx context.Context, host hostCaller, pagesHost string, pid string, items []pixivMediaItem) []string {
	timestamp := time.Now().Unix()
	urls := make([]string, 0, len(items))
	for _, item := range items {
		mediaURL := buildPagesAssetURL(pagesHost, item.Path)
		if mediaURL == "" {
			continue
		}
		blobID := fmt.Sprintf("pixiv-media-%s-%d-%d", pid, item.Index, timestamp)
		if uploaded := uploadViaBlobPlugin(ctx, host, mediaURL, blobID, "image"); uploaded != "" {
			mediaURL = uploaded
		}
		urls = append(urls, mediaURL)
	}
	return urls
}

func buildPixivForwardNodes(mediaURLs []string, senderID any, nickname string) []map[string]any {
	nodes := make([]map[string]any, 0, len(mediaURLs))
	total := len(mediaURLs)
	if strings.TrimSpace(nickname) == "" {
		nickname = "AmiaBot Pixiv"
	}
	for index, mediaURL := range mediaURLs {
		if strings.TrimSpace(mediaURL) == "" {
			continue
		}
		nodes = append(nodes, map[string]any{
			"type": "node",
			"data": map[string]any{
				"user_id":  senderID,
				"nickname": nickname,
				"content": []map[string]any{
					{"type": "text", "data": map[string]any{"text": fmt.Sprintf("P%d / %d", index+1, total)}},
					{"type": "image", "data": map[string]any{"file": mediaURL}},
				},
			},
		})
	}
	return nodes
}

func extractErrorMessage(body []byte) string {
	var payload struct {
		Error string `json:"error"`
	}
	if err := json.Unmarshal(body, &payload); err == nil {
		if msg := strings.TrimSpace(payload.Error); msg != "" {
			return msg
		}
	}
	trimmed := strings.TrimSpace(string(body))
	if len(trimmed) > 120 {
		trimmed = trimmed[:120] + "..."
	}
	return trimmed
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &AmiabotPixiv{}},
		},
	})
}
