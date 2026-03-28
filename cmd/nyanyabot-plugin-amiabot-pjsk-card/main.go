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

type PJSKCard struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages  string `json:"amiabot_pages"`
		DefaultServer string `json:"default_server"`
	}
}

func (e *PJSKCard) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址；为空则无法生成截图 URL"},
			"default_server":{"type":"string","description":"默认服务器 (jp/cn/en/tw/kr)，不填时为 jp"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":"","default_server":"jp"}`)
	return papi.Descriptor{
		Name:         "Amiabot PJSK Card",
		PluginID:     "external.amiabot-pjsk-card",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "PJSK 卡面查询插件，发送截图",
		Dependencies: []string{"external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot PJSK Card plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "pjsk-card",
				ID:          "cmd.pjsk-card",
				Description: "PJSK 卡面查询（如 card1, jpcard1, cn查卡5）",
				Pattern:     `^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:card|查卡)(?P<id>[0-9]+)$`,
				MatchRaw:    true,
				Handler:     "HandlePJSKCard",
			},
		},
	}, nil
}

func (e *PJSKCard) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	cfg := struct {
		AmiabotPages  string `json:"amiabot_pages"`
		DefaultServer string `json:"default_server"`
	}{
		DefaultServer: "jp",
	}
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}
	defSrv := strings.TrimSpace(cfg.DefaultServer)
	if defSrv == "" {
		defSrv = "jp"
	}
	e.mu.Lock()
	e.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	e.cfg.DefaultServer = defSrv
	e.mu.Unlock()
	return nil
}

func (e *PJSKCard) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (e *PJSKCard) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	hclog.L().Info("[Card] Handle() CALLED", "listenerID", listenerID)
	if listenerID == "cmd.pjsk-card" {
		return e.handlePJSKCard(ctx, eventRaw, match)
	}
	return papi.HandleResult{}, nil
}

func (e *PJSKCard) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

func (e *PJSKCard) handlePJSKCard(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	log.Info("[Card] ===== 开始处理 =====")

	// 解析事件以获取 msgType/groupID/userID 用于 sendError
	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[Card] 解析事件失败", "error", err)
		return papi.HandleResult{}, nil
	}
	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]
	rawMessage, _ := evt["raw_message"].(string)

	// recover 兜底 panic
	defer func() {
		if r := recover(); r != nil {
			err := fmt.Errorf("panic: %v", r)
			log.Error("[Card] panic", "error", err)
			sendError(transport.Host(), msgType, groupID, userID, "❌ 卡面查询异常", err)
		}
	}()

	log.Info("[Card] 收到消息", "raw_message", rawMessage, "msg_type", msgType)

	host := transport.Host()
	if host == nil {
		log.Warn("[Card] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	server, id := e.parseArgs(rawMessage, match)
	log.Info("[Card] 解析结果", "server", server, "id", id)

	if server == "" || id == "" {
		log.Warn("[Card] 参数不完整，终止")
		sendText(host, msgType, groupID, userID, "❌ 参数不完整，请使用格式: card+编号")
		return papi.HandleResult{}, nil
	}

	e.mu.RLock()
	pagesHost := e.cfg.AmiabotPages
	e.mu.RUnlock()

	if pagesHost == "" {
		log.Warn("[Card] amiabot_pages 未配置，终止")
		sendText(host, msgType, groupID, userID, "❌ 服务未配置")
		return papi.HandleResult{}, nil
	}

	pageURL := buildPagesURL(pagesHost, "/pjsk/card", map[string]string{"server": server, "id": id})
	log.Info("[Card] 页面 URL", "url", pageURL)

	log.Info("[Card] 调用截图插件...")
	screenshotURL, screenshotErr := buildScreenshotViaPlugin(host, pageURL)
	log.Info("[Card] 截图 URL", "url", screenshotURL, "error", screenshotErr)
	if screenshotErr != nil {
		log.Warn("[Card] 截图失败", "error", screenshotErr)
		sendError(host, msgType, groupID, userID, "❌ 截图失败", screenshotErr)
		return papi.HandleResult{}, nil
	}

	blobID := fmt.Sprintf("pjsk-card-%s-%s-%d", server, id, time.Now().Unix())
	log.Info("[Card] 调用 blobserver 上传...", "blob_id", blobID)
	if uploaded := uploadViaBlobPlugin(ctx, host, screenshotURL, blobID, "image"); uploaded != "" {
		log.Info("[Card] 上传成功", "url", uploaded)
		screenshotURL = uploaded
	}

	log.Info("[Card] 发送图片消息...")
	_ = sendImage(host, msgType, groupID, userID, screenshotURL)
	log.Info("[Card] ===== 处理完成 =====")
	return papi.HandleResult{}, nil
}

func (e *PJSKCard) parseArgs(rawMessage string, match *papi.CommandMatch) (server, id string) {
	re := regexp.MustCompile(`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:card|查卡)(?P<id>[0-9]+)$`)
	m := re.FindStringSubmatch(rawMessage)
	if len(m) >= 3 {
		server = strings.ToLower(strings.TrimSpace(m[1]))
		id = strings.TrimSpace(m[2])
	}
	if server == "" {
		e.mu.RLock()
		server = e.cfg.DefaultServer
		e.mu.RUnlock()
	}
	return
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &PJSKCard{}},
		},
	})
}
