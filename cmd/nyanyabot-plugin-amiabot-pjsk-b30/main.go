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

var pjskB30Regex = regexp.MustCompile(`^(?:(?P<server>cn|jp|tw|en|kr))?b30$`)

// PJSKB30 插件主结构
type PJSKB30 struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages  string `json:"amiabot_pages"`
		DefaultServer string `json:"default_server"`
	}
}

func (p *PJSKB30) Descriptor(ctx context.Context) (papi.Descriptor, error) {
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
		Name:         "Amiabot PJSK B30",
		PluginID:     "external.amiabot-pjsk-b30",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "PJSK B30 查询插件，发送 b30 截图",
		Dependencies: []string{"external.amiabot-pjsk-account", "external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot PJSK B30 plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "pjsk-b30",
				ID:          "cmd.pjsk-b30",
				Description: "查看 PJSK B30（如 b30, cnb30）",
				Pattern:     `^(?:(?P<server>cn|jp|tw|en|kr))?b30$`,
				MatchRaw:    true,
				Handler:     "HandleB30",
			},
		},
	}, nil
}

func (p *PJSKB30) Configure(ctx context.Context, config json.RawMessage) error {
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
	p.mu.Lock()
	p.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	p.cfg.DefaultServer = defSrv
	p.mu.Unlock()
	return nil
}

func (p *PJSKB30) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (p *PJSKB30) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	if listenerID == "cmd.pjsk-b30" {
		return p.handleB30(ctx, eventRaw, match)
	}
	return papi.HandleResult{}, nil
}

func (p *PJSKB30) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

func (p *PJSKB30) handleB30(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[B30] 解析事件失败", "error", err)
		return papi.HandleResult{}, nil
	}
	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]
	rawMessage, _ := evt["raw_message"].(string)

	defer func() {
		if r := recover(); r != nil {
			err := fmt.Errorf("panic: %v", r)
			log.Error("[B30] panic", "error", err)
			util.SendError(transport.Host(), msgType, groupID, userID, "❌ B30 查询异常", err)
		}
	}()

	host := transport.Host()
	if host == nil {
		log.Warn("[B30] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	qqIDInt := evtToQQID(evt)

	// 调用 account.list_by_qq 获取所有启用账户
	listResult, err := host.CallDependency(ctx, "external.amiabot-pjsk-account", "account.list_by_qq", map[string]any{
		"qq_id":        qqIDInt,
		"enabled_only": true,
	})
	if err != nil {
		log.Error("[B30] 调用 account.list_by_qq 失败", "error", err)
		util.SendError(host, msgType, groupID, userID, "❌ 查询绑定失败", err)
		return papi.HandleResult{}, nil
	}

	var listResp struct {
		Success  bool      `json:"success"`
		Accounts []Account `json:"accounts"`
	}
	if err := json.Unmarshal(listResult, &listResp); err != nil {
		log.Error("[B30] 解析账户列表失败", "error", err)
		util.SendText(host, msgType, groupID, userID, "❌ 查询失败")
		return papi.HandleResult{}, nil
	}

	if !listResp.Success || len(listResp.Accounts) == 0 {
		util.SendText(host, msgType, groupID, userID, "你还没有绑定任何账号。\n请发送 绑定+你的游戏ID 进行绑定，如 jp绑定12345")
		return papi.HandleResult{}, nil
	}

	// 解析服务器
	server := p.parseServer(rawMessage, match)

	// 如果指定了 server 为空，优先使用用户默认服务器
	if server == "" {
		// 调用 account.get_preferred_server 获取用户默认服务器
		getSrvResult, getSrvErr := host.CallDependency(ctx, "external.amiabot-pjsk-account", "account.get_preferred_server", map[string]any{
			"qq_id": qqIDInt,
		})
		if getSrvErr == nil {
			var getSrvResp struct {
				Success bool   `json:"success"`
				Server  string `json:"server"`
			}
			if err := json.Unmarshal(getSrvResult, &getSrvResp); err == nil && getSrvResp.Success && getSrvResp.Server != "" {
				server = getSrvResp.Server
			}
		}
	}

	// 如果仍没有服务器，回退到全局配置
	if server == "" {
		p.mu.RLock()
		server = p.cfg.DefaultServer
		p.mu.RUnlock()
	}

	// 在账户列表中查找该服务器的第一个账户
	var targetGameID string
	for _, acc := range listResp.Accounts {
		if acc.GameServer == server && acc.Enabled {
			targetGameID = acc.GameID
			break
		}
	}

	if targetGameID == "" {
		// 没有该服务器的账户，列出已有的
		serverUpper := strings.ToUpper(server)
		var lines []string
		for _, acc := range listResp.Accounts {
			srv := strings.ToUpper(acc.GameServer)
			lines = append(lines, fmt.Sprintf("[%s]", srv))
		}
		util.SendText(host, msgType, groupID, userID, fmt.Sprintf("你尚未绑定 [%s] 服务器的账号。\n你已绑定的服务器: %s\n可以通过 服务器前缀+b30 切换，如 cnb30", serverUpper, strings.Join(lines, ", ")))
		return papi.HandleResult{}, nil
	}

	p.mu.RLock()
	pagesHost := p.cfg.AmiabotPages
	p.mu.RUnlock()

	if pagesHost == "" {
		log.Warn("[B30] amiabot_pages 未配置，终止")
		util.SendText(host, msgType, groupID, userID, "❌ 服务未配置")
		return papi.HandleResult{}, nil
	}

	pageURL := util.BuildPagesURL(pagesHost, "/pjsk/b30", map[string]string{
		"server": server,
		"id":     targetGameID,
	})
	log.Info("[B30] 页面 URL", "url", pageURL)

	screenshotURL, screenshotErr := util.BuildScreenshotViaPlugin(host, pageURL)
	if screenshotErr != nil {
		log.Warn("[B30] 截图失败", "error", screenshotErr)
		util.SendError(host, msgType, groupID, userID, "❌ 截图失败", screenshotErr)
		return papi.HandleResult{}, nil
	}

	blobID := fmt.Sprintf("pjsk-b30-%s-%s-%d", server, targetGameID, time.Now().Unix())
	if uploaded := util.UploadViaBlobPlugin(ctx, host, screenshotURL, blobID, "image"); uploaded != "" {
		screenshotURL = uploaded
	}

	_ = util.SendImage(host, msgType, groupID, userID, screenshotURL)
	return papi.HandleResult{}, nil
}

func (p *PJSKB30) parseServer(rawMessage string, match *papi.CommandMatch) string {
	m := pjskB30Regex.FindStringSubmatch(rawMessage)
	if len(m) < 2 {
		return ""
	}
	return strings.ToLower(strings.TrimSpace(m[1]))
}

// Account 从 account 插件返回的账户结构
type Account struct {
	QQID       int64  `json:"qq_id"`
	GameServer string `json:"game_server"`
	GameID     string `json:"game_id"`
	Enabled    bool   `json:"enabled"`
}

// evtToQQID 从事件 map 中安全提取 QQ 号（兼容 float64 和 json.Number）
func evtToQQID(evt map[string]any) int64 {
	switch v := evt["user_id"].(type) {
	case float64:
		return int64(v)
	case json.Number:
		if n, err := v.Int64(); err == nil {
			return n
		}
	}
	return 0
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &PJSKB30{}},
		},
	})
}
