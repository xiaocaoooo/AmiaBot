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
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
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
	screenshotURL, _ := util.BuildScreenshotViaPlugin(ctx, host, statusURL)
	if screenshotURL == "" {
		return papi.HandleResult{}, nil
	}

	// 上传图片到 blobserver
	imageID := fmt.Sprintf("zeabur-status-%d", time.Now().Unix())
	uploadedURL := util.UploadViaBlobPlugin(ctx, host, screenshotURL, imageID, "image")
	if uploadedURL != "" {
		screenshotURL = uploadedURL
	}

	// 发送图片
	_ = util.SendImage(ctx, host, msgType, groupID, userID, screenshotURL)

	return papi.HandleResult{}, nil
}

// buildStatusPageURL 构建状态页 URL。
func buildStatusPageURL(amiabotPages string) string {
	base := util.NormalizeHTTPBase(amiabotPages)
	u, err := url.Parse(base)
	if err != nil {
		return ""
	}
	u.Path = strings.TrimRight(u.Path, "/") + "/status/zeabur"
	return u.String()
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
