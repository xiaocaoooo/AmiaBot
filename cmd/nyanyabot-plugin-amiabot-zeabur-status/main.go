// nyanyabot-plugin-amiabot-zeabur-status
//
// 当用户输入 `status` 或 `状态` 时，截图输出 Zeabur 状态页面。
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/url"
	"strings"
	"sync"
	"time"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
)

// ZeaburStatus 是插件的实现类型。
type ZeaburStatus struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages string `json:"amiabot_pages"`
	}
}

// Descriptor 返回插件自描述信息。
func (z *ZeaburStatus) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx

	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 服务地址（用于访问 /status/zeabur 页面）；为空则无法生成截图"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":""}`)

	return papi.Descriptor{
		Name:        "Amiabot Zeabur Status",
		PluginID:    "external.amiabot-zeabur-status",
		Version:     "0.1.0",
		Author:      "nyanyabot",
		Description: "输入 status 或状态时，截图输出 Zeabur 状态页面",
		Dependencies: []string{
			"external.screenshot",
			"external.blobserver",
		},
		Exports: []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "zeabur-status",
				ID:          "cmd.zeabur-status",
				Description: "输入 status 或状态时，截图输出 Zeabur 状态页面",
				Pattern:     `(?i)^(status|状态)$`,
				MatchRaw:    false,
				Handler:     "HandleStatus",
			},
		},
	}, nil
}

// Configure 接收宿主下发的配置。
func (z *ZeaburStatus) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	cfg := struct {
		AmiabotPages string `json:"amiabot_pages"`
	}{}
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}
	z.mu.Lock()
	z.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	z.mu.Unlock()
	return nil
}

// Invoke 处理跨插件方法调用。
func (z *ZeaburStatus) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

// Handle 是宿主分发入口。
func (z *ZeaburStatus) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	_ = match

	switch listenerID {
	case "cmd.zeabur-status":
		return z.handleStatus(ctx, eventRaw)
	default:
		return papi.HandleResult{}, nil
	}
}

// Shutdown 在插件被宿主关闭时调用。
func (z *ZeaburStatus) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

// hostCaller 抽象宿主提供的 OneBot 调用能力。
type hostCaller interface {
	CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error)
	CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error)
}

// handleStatus 处理 status/状态 命令。
func (z *ZeaburStatus) handleStatus(ctx context.Context, eventRaw ob11.Event) (papi.HandleResult, error) {
	host := transport.Host()
	if host == nil {
		return papi.HandleResult{}, nil
	}

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		return papi.HandleResult{}, nil
	}

	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]

	// 读取配置
	z.mu.RLock()
	pagesHost := z.cfg.AmiabotPages
	z.mu.RUnlock()

	if pagesHost == "" {
		return papi.HandleResult{}, nil
	}

	// 构建状态页 URL
	statusURL := buildStatusPageURL(pagesHost)
	if statusURL == "" {
		return papi.HandleResult{}, nil
	}

	// 调用 screenshot 插件生成截图 URL
	screenshotURL := buildScreenshotURL(host, statusURL)
	if screenshotURL == "" {
		return papi.HandleResult{}, nil
	}

	// 上传图片到 blobserver
	imageID := fmt.Sprintf("zeabur-status-%d", time.Now().Unix())
	uploadedURL := uploadViaBlobPlugin(ctx, host, screenshotURL, imageID, "image")
	if uploadedURL != "" {
		screenshotURL = uploadedURL
	}

	// 发送图片
	_ = sendImage(host, msgType, groupID, userID, screenshotURL)

	return papi.HandleResult{}, nil
}

// buildStatusPageURL 构建状态页 URL。
func buildStatusPageURL(amiabotPages string) string {
	base := normalizeHTTPBase(amiabotPages)
	u, err := url.Parse(base)
	if err != nil {
		return ""
	}
	u.Path = strings.TrimRight(u.Path, "/") + "/status/zeabur"
	return u.String()
}

// buildScreenshotURL 调用 screenshot 插件生成截图 URL。
func buildScreenshotURL(host hostCaller, pageURL string) string {
	if host == nil || strings.TrimSpace(pageURL) == "" {
		return ""
	}
	result, err := host.CallDependency(context.Background(), "external.screenshot", "screenshot.build_url", map[string]any{
		"page_url": pageURL,
		"selector": "#screenshot-wrapper",
	})
	if err != nil {
		return ""
	}
	var out struct {
		URL string `json:"url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		return ""
	}
	return strings.TrimSpace(out.URL)
}

// uploadViaBlobPlugin 调用 blobserver 插件上传远程文件。
func uploadViaBlobPlugin(ctx context.Context, host hostCaller, downloadURL string, blobID string, kind string) string {
	if host == nil || strings.TrimSpace(downloadURL) == "" || strings.TrimSpace(blobID) == "" {
		return ""
	}
	result, err := host.CallDependency(ctx, "external.blobserver", "blob.upload_remote", map[string]any{
		"download_url": downloadURL,
		"blob_id":      blobID,
		"kind":         kind,
	})
	if err != nil {
		return ""
	}
	var out struct {
		BlobURL   string `json:"blob_url"`
		OneBotURL string `json:"onebot_url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		return ""
	}
	if strings.TrimSpace(out.OneBotURL) != "" {
		return strings.TrimSpace(out.OneBotURL)
	}
	return strings.TrimSpace(out.BlobURL)
}

// sendImage 发送图片消息。
func sendImage(host hostCaller, msgType string, groupID any, userID any, url string) error {
	if host == nil {
		return nil
	}
	if msgType == "group" {
		_, err := host.CallOneBot(context.Background(), "send_group_msg", map[string]any{
			"group_id": groupID,
			"message": []map[string]any{
				{"type": "image", "data": map[string]any{"file": url}},
			},
		})
		return err
	}
	_, err := host.CallOneBot(context.Background(), "send_private_msg", map[string]any{
		"user_id": userID,
		"message": []map[string]any{
			{"type": "image", "data": map[string]any{"file": url}},
		},
	})
	return err
}

// normalizeHTTPBase 标准化 HTTP 基础 URL。
func normalizeHTTPBase(hostOrURL string) string {
	hostOrURL = strings.TrimSpace(hostOrURL)
	if hostOrURL == "" {
		return ""
	}
	if strings.HasPrefix(hostOrURL, "http://") || strings.HasPrefix(hostOrURL, "https://") {
		return strings.TrimRight(hostOrURL, "/")
	}
	return "http://" + strings.TrimRight(hostOrURL, "/")
}

func main() {
	logger := hclog.New(&hclog.LoggerOptions{Name: "nyanyabot-plugin-amiabot-zeabur-status", Level: hclog.Info})

	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &ZeaburStatus{}},
		},
		Logger: logger,
	})
}
