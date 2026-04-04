package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"sort"
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

var (
	bindRegex = regexp.MustCompile(`^(?i)(?:(?P<server>cn|jp|tw|en|kr))?绑定(?P<id>\d+)$`)
	idRegex   = regexp.MustCompile(`^(?:(?P<server>cn|jp|tw|en|kr)|(?:烤))id$`)

	httpClient = &http.Client{Timeout: 15 * time.Second}
)

// PJSKBind 插件主结构
type PJSKBind struct {
	mu  sync.RWMutex
	cfg struct {
		AmiabotPages string `json:"amiabot_pages"`
	}
}

func (p *PJSKBind) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址；用于获取 profile 用户名"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":""}`)

	return papi.Descriptor{
		Name:         "Amiabot PJSK Bind",
		PluginID:     "external.amiabot-pjsk-bind",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "PJSK 账户绑定插件，提供绑定和查询 ID 的功能",
		Dependencies: []string{"external.amiabot-pjsk-account"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot PJSK Bind plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "pjsk-bind",
				ID:          "cmd.profile-bind",
				Description: "绑定 PJSK 游戏账号（如 绑定12345, jp绑定12345）",
				Pattern:     `^(?i)(?:(?P<server>cn|jp|tw|en|kr))?绑定(?P<id>\d+)$`,
				MatchRaw:    true,
				Handler:     "HandleBind",
			},
			{
				Name:        "pjsk-id",
				ID:          "cmd.profile-id",
				Description: "查询已绑定的游戏 ID（如 id, jpid, 烤id）",
				Pattern:     `^(?:(?P<server>cn|jp|tw|en|kr)|(?:烤))id$`,
				MatchRaw:    true,
				Handler:     "HandleID",
			},
		},
	}, nil
}

func (p *PJSKBind) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	cfg := struct {
		AmiabotPages string `json:"amiabot_pages"`
	}{}
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}

	p.mu.Lock()
	p.cfg.AmiabotPages = util.NormalizeHTTPBase(strings.TrimSpace(cfg.AmiabotPages))
	p.mu.Unlock()
	return nil
}

func (p *PJSKBind) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (p *PJSKBind) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	if listenerID == "cmd.profile-bind" {
		return p.handleBind(ctx, eventRaw, match)
	}
	if listenerID == "cmd.profile-id" {
		return p.handleID(ctx, eventRaw, match)
	}
	return papi.HandleResult{}, nil
}

func (p *PJSKBind) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

// parseBindArgs 解析绑定命令参数
func parseBindArgs(rawMessage string) (server, id string) {
	m := bindRegex.FindStringSubmatch(rawMessage)
	if len(m) < 3 {
		return
	}
	server = strings.ToLower(strings.TrimSpace(m[1]))
	if server == "" {
		server = "jp"
	}
	id = strings.TrimSpace(m[2])
	return
}

// parseIDArgs 解析 ID 查询命令参数
func parseIDArgs(rawMessage string) string {
	m := idRegex.FindStringSubmatch(rawMessage)
	if len(m) < 2 {
		return ""
	}
	// 第一个捕获组是 (?P<server>cn|jp|tw|en|kr)，第二个是 (?:烤)（非捕获）
	// m[1] 是 server 捕获组，如果为空说明是"烤id"
	server := strings.ToLower(strings.TrimSpace(m[1]))
	return server
}

// handleBind 处理绑定命令
func (p *PJSKBind) handleBind(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[Bind] 解析事件失败", "error", err)
		return papi.HandleResult{}, nil
	}
	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]
	rawMessage, _ := evt["raw_message"].(string)

	defer func() {
		if r := recover(); r != nil {
			err := fmt.Errorf("panic: %v", r)
			log.Error("[Bind] panic", "error", err)
			util.SendError(transport.Host(), msgType, groupID, userID, "❌ 绑定异常", err)
		}
	}()

	host := transport.Host()
	if host == nil {
		log.Warn("[Bind] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	server, gameID := parseBindArgs(rawMessage)
	log.Info("[Bind] 解析参数", "server", server, "game_id", gameID)

	if server == "" || gameID == "" {
		util.SendText(host, msgType, groupID, userID, "❌ 参数不正确，请发送 绑定+你的游戏ID，如 j p绑定12345")
		return papi.HandleResult{}, nil
	}

	// 调用 account.add
	qqIDInt := evtToQQID(evt)
	addResult, err := host.CallDependency(ctx, "external.amiabot-pjsk-account", "account.add", map[string]any{
		"qq_id":       qqIDInt,
		"game_server": server,
		"game_id":     gameID,
	})
	if err != nil {
		log.Error("[Bind] 调用 account.add 失败", "error", err)
		util.SendError(host, msgType, groupID, userID, "❌ 绑定失败", err)
		return papi.HandleResult{}, nil
	}

	var addResp struct {
		Success bool   `json:"success"`
		Message string `json:"message"`
	}
	if err := json.Unmarshal(addResult, &addResp); err != nil || !addResp.Success {
		msg := "绑定失败"
		if addResp.Message != "" {
			msg = addResp.Message
		}
		util.SendText(host, msgType, groupID, userID, "❌ "+msg)
		return papi.HandleResult{}, nil
	}

	// 通过 pages 获取 profile 用户名
	username, err := p.fetchProfileName(ctx, server, gameID)
	if err != nil {
		log.Warn("[Bind] 获取 profile 失败", "error", err)
		serverUpper := strings.ToUpper(server)
		util.SendText(host, msgType, groupID, userID, fmt.Sprintf("绑定成功！\n[%s]\n\n获取用户名失败: %s", serverUpper, err.Error()))
		return papi.HandleResult{}, nil
	}

	serverUpper := strings.ToUpper(server)
	util.SendText(host, msgType, groupID, userID, fmt.Sprintf("绑定成功！\n[%s] %s", serverUpper, username))
	return papi.HandleResult{}, nil
}

// handleID 处理 ID 查询命令
func (p *PJSKBind) handleID(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		log.Error("[Bind] 解析事件失败", "error", err)
		return papi.HandleResult{}, nil
	}
	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]

	defer func() {
		if r := recover(); r != nil {
			err := fmt.Errorf("panic: %v", r)
			log.Error("[Bind] panic", "error", err)
			util.SendError(transport.Host(), msgType, groupID, userID, "❌ 查询异常", err)
		}
	}()

	host := transport.Host()
	if host == nil {
		log.Warn("[Bind] host 为 nil，终止")
		return papi.HandleResult{}, nil
	}

	rawMessage, _ := evt["raw_message"].(string)
	specificServer := parseIDArgs(rawMessage)

	qqIDInt := evtToQQID(evt)

	// 调用 account.list_by_qq
	listResult, err := host.CallDependency(ctx, "external.amiabot-pjsk-account", "account.list_by_qq", map[string]any{
		"qq_id":         qqIDInt,
		"enabled_only":  true,
	})
	if err != nil {
		log.Error("[Bind] 调用 account.list_by_qq 失败", "error", err)
		util.SendError(host, msgType, groupID, userID, "❌ 查询失败", err)
		return papi.HandleResult{}, nil
	}

	var listResp struct {
		Success  bool     `json:"success"`
		Accounts []Account `json:"accounts"`
		Message  string   `json:"message"`
	}
	if err := json.Unmarshal(listResult, &listResp); err != nil {
		log.Error("[Bind] 解析账户列表失败", "error", err)
		util.SendText(host, msgType, groupID, userID, "❌ 查询失败")
		return papi.HandleResult{}, nil
	}

	if !listResp.Success {
		msg := "未找到绑定的账号"
		if listResp.Message != "" {
			msg = listResp.Message
		}
		util.SendText(host, msgType, groupID, userID, msg)
		return papi.HandleResult{}, nil
	}

	accounts := listResp.Accounts
	if len(accounts) == 0 {
		util.SendText(host, msgType, groupID, userID, "还未绑定任何账号，请发送 绑定+游戏ID 进行绑定")
		return papi.HandleResult{}, nil
	}

	// 按服务器名称排序
	sort.Slice(accounts, func(i, j int) bool {
		return accounts[i].GameServer < accounts[j].GameServer
	})

	if specificServer != "" {
		// 只返回指定服务器的
		var filtered []Account
		for _, acc := range accounts {
			if acc.GameServer == specificServer && acc.Enabled {
				filtered = append(filtered, acc)
			}
		}
		if len(filtered) == 0 {
			serverUpper := strings.ToUpper(specificServer)
			util.SendText(host, msgType, groupID, userID, fmt.Sprintf("未找到 [%s] 服务器已绑定的账号", serverUpper))
			return papi.HandleResult{}, nil
		}
		var lines []string
		for _, acc := range filtered {
			serverUpper := strings.ToUpper(acc.GameServer)
			lines = append(lines, fmt.Sprintf("[%s] %s", serverUpper, acc.GameID))
		}
		util.SendText(host, msgType, groupID, userID, strings.Join(lines, "\n"))
		return papi.HandleResult{}, nil
	}

	// 返回所有已启用的
	var lines []string
	for _, acc := range accounts {
		if acc.Enabled {
			serverUpper := strings.ToUpper(acc.GameServer)
			lines = append(lines, fmt.Sprintf("[%s] %s", serverUpper, acc.GameID))
		}
	}
	if len(lines) == 0 {
		util.SendText(host, msgType, groupID, userID, "未找到已启用的账号")
		return papi.HandleResult{}, nil
	}
	util.SendText(host, msgType, groupID, userID, strings.Join(lines, "\n"))
	return papi.HandleResult{}, nil
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

// fetchProfileName 通过 pages 获取 profile 用户名
func (p *PJSKBind) fetchProfileName(ctx context.Context, server, gameID string) (string, error) {
	p.mu.RLock()
	pagesHost := p.cfg.AmiabotPages
	p.mu.RUnlock()

	if pagesHost == "" {
		return "", fmt.Errorf("pages 地址未配置")
	}

	pagesURL := util.BuildPagesURL(pagesHost, "/pjsk/profile/raw", map[string]string{
		"server": server,
		"id":     gameID,
	})

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, pagesURL, nil)
	if err != nil {
		return "", fmt.Errorf("创建请求失败: %w", err)
	}
	req.Header.Set("User-Agent", "amiabot-pjsk-bind/1.0")
	req.Header.Set("Accept", "application/json")

	resp, err := httpClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("请求 pages 失败: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("pages 返回 %d: %s", resp.StatusCode, shortenErrorBody(body))
	}

	var result struct {
		User struct {
			Name string `json:"name"`
		} `json:"user"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", fmt.Errorf("解析 pages 返回失败: %w", err)
	}

	username := strings.TrimSpace(result.User.Name)
	if username == "" {
		return "", fmt.Errorf("未获取到用户名")
	}
	return username, nil
}

func shortenErrorBody(body []byte) string {
	text := strings.TrimSpace(string(body))
	if text == "" {
		return "空响应"
	}
	text = strings.ReplaceAll(text, "\n", " ")
	text = strings.ReplaceAll(text, "\r", " ")
	text = strings.Join(strings.Fields(text), " ")
	if len(text) > 240 {
		return text[:240] + "..."
	}
	return text
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &PJSKBind{}},
		},
	})
}
