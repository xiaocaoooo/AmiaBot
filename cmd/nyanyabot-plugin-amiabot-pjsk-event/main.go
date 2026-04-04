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
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

type PJSKEvent struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages  string `json:"amiabot_pages"`
		DefaultServer string `json:"default_server"`
	}
}

func (e *PJSKEvent) Descriptor(ctx context.Context) (papi.Descriptor, error) {
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
		Name:         "Amiabot PJSK Event",
		PluginID:     "external.amiabot-pjsk-event",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "PJSK 活动查询插件，发送截图",
		Dependencies: []string{"external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot PJSK Event plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "pjsk-event",
				ID:          "cmd.pjsk-event",
				Description: "PJSK 活动查询（如 event, jpevent, cn查活动, enevent50）",
				Pattern:     `^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:event|查活动)(?P<id>[0-9]*)$`,
				MatchRaw:    true,
				Handler:     "HandlePJSKEvent",
			},
		},
	}, nil
}

func (e *PJSKEvent) Configure(ctx context.Context, config json.RawMessage) error {
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

func (e *PJSKEvent) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (e *PJSKEvent) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	hclog.L().Info("[Event] Handle() CALLED", "listenerID", listenerID)
	if listenerID == "cmd.pjsk-event" {
		return e.handlePJSKEvent(ctx, eventRaw, match)
	}
	return papi.HandleResult{}, nil
}

func (e *PJSKEvent) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

func (e *PJSKEvent) handlePJSKEvent(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	log.Info("[Event] ===== 开始处理 =====")

	// 解析事件以获取 msgType/groupID/userID
	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[Event] 解析事件失败", "error", err)
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
			log.Error("[Event] panic", "error", err)
			util.SendError(transport.Host(), msgType, groupID, userID, "❌ 活动查询异常", err)
		}
	}()

	log.Info("[Event] 收到消息", "raw_message", rawMessage, "msg_type", msgType)

	host := transport.Host()
	if host == nil {
		log.Warn("[Event] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	server, id := e.parseArgs(rawMessage, match)
	log.Info("[Event] 解析结果", "server", server, "id", id)

	if server == "" {
		log.Warn("[Event] server 为空，终止")
		util.SendText(host, msgType, groupID, userID, "❌ 服务器参数无效")
		return papi.HandleResult{}, nil
	}

	e.mu.RLock()
	pagesHost := e.cfg.AmiabotPages
	e.mu.RUnlock()

	if pagesHost == "" {
		log.Warn("[Event] amiabot_pages 未配置，终止")
		util.SendText(host, msgType, groupID, userID, "❌ 服务未配置")
		return papi.HandleResult{}, nil
	}

	params := map[string]string{"server": server}
	if id != "" {
		params["id"] = id
	}
	pageURL := util.BuildPagesURL(pagesHost, "/pjsk/event", params)
	log.Info("[Event] 页面 URL", "url", pageURL)

	log.Info("[Event] 调用截图插件...")
	screenshotURL, screenshotErr := util.BuildScreenshotViaPlugin(host, pageURL)
	log.Info("[Event] 截图 URL", "url", screenshotURL, "error", screenshotErr)
	if screenshotErr != nil {
		log.Warn("[Event] 截图失败", "error", screenshotErr)
		util.SendError(host, msgType, groupID, userID, "❌ 截图失败", screenshotErr)
		return papi.HandleResult{}, nil
	}

	blobID := fmt.Sprintf("pjsk-event-%s-%s-%d", server, id, time.Now().Unix())
	log.Info("[Event] 调用 blobserver 上传...", "blob_id", blobID)
	if uploaded := util.UploadViaBlobPlugin(ctx, host, screenshotURL, blobID, "image"); uploaded != "" {
		log.Info("[Event] 上传成功", "url", uploaded)
		screenshotURL = uploaded
	}

	log.Info("[Event] 发送图片消息...")
	_ = util.SendImage(host, msgType, groupID, userID, screenshotURL)
	log.Info("[Event] ===== 处理完成 =====")
	return papi.HandleResult{}, nil
}

func (e *PJSKEvent) parseArgs(rawMessage string, match *papi.CommandMatch) (server, id string) {
	re := regexp.MustCompile(`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?(?:event|查活动)(?P<id>[0-9]*)$`)
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
			transport.PluginName: &transport.Map{PluginImpl: &PJSKEvent{}},
		},
	})
}
