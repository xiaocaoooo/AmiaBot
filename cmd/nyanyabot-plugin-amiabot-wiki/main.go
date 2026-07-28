// nyanyabot-plugin-amiabot-wiki
//
// setwiki 绑定群默认 GitHub 仓库；wiki 向 DeepWiki 提问。
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

const (
	defaultDeepWikiURL     = "https://mcp.deepwiki.com/mcp"
	defaultDeepWikiTimeout = 120
	// OneBot 文本消息不宜过长，按 rune 分片发送。
	maxMessageRunes = 3500
)

var (
	setWikiRegex = regexp.MustCompile(`^(?i)setwiki\s+(?P<owner>\S+)\/(?P<repo>\S+)$`)
	wikiRegex    = regexp.MustCompile(`^(?i)wiki\s+(?:(?P<owner>\S+)\/(?P<repo>\S+)\s+)?(?P<question>.+)$`)
)

// WikiPlugin 是 DeepWiki 插件实现。
type WikiPlugin struct {
	mu  sync.RWMutex
	cfg config
	db  *wikiStore
}

type config struct {
	DatabaseURL        string `json:"database_url"`
	DeepWikiURL        string `json:"deepwiki_url"`
	DeepWikiTimeoutSec int    `json:"deepwiki_timeout_sec"`
}

type groupMemberInfo struct {
	Role string `json:"role"`
}

func (p *WikiPlugin) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"database_url":{"type":"string","description":"PostgreSQL 数据库连接字符串（必填，用于存储群默认仓库）"},
			"deepwiki_url":{"type":"string","description":"DeepWiki MCP 服务地址"},
			"deepwiki_timeout_sec":{"type":"integer","description":"DeepWiki 请求超时（秒）"}
		},
		"required":["database_url"],
		"additionalProperties":false
	}`)
	def := json.RawMessage(fmt.Sprintf(
		`{"database_url":"","deepwiki_url":%q,"deepwiki_timeout_sec":%d}`,
		defaultDeepWikiURL,
		defaultDeepWikiTimeout,
	))

	return papi.Descriptor{
		Name:         "Amiabot Wiki",
		PluginID:     "external.amiabot-wiki",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "通过 DeepWiki 查询 GitHub 仓库文档；setwiki 设置群默认仓库，wiki 提问",
		Dependencies: []string{},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot Wiki plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "setwiki",
				ID:          "cmd.setwiki",
				Description: "设置本群默认 GitHub 仓库（仅群主/管理员），如 setwiki owner/repo",
				Pattern:     `^(?i)setwiki\s+(?P<owner>\S+)\/(?P<repo>\S+)$`,
				MatchRaw:    true,
				Handler:     "HandleSetWiki",
			},
			{
				Name:        "wiki",
				ID:          "cmd.wiki",
				Description: "向 DeepWiki 提问，如 wiki 问题 或 wiki owner/repo 问题",
				Pattern:     `^(?i)wiki\s+(?:(?P<owner>\S+)\/(?P<repo>\S+)\s+)?(?P<question>.+)$`,
				MatchRaw:    true,
				Handler:     "HandleWiki",
			},
		},
		Events: []papi.EventListener{},
	}, nil
}

func (p *WikiPlugin) Configure(ctx context.Context, configJSON json.RawMessage) error {
	_ = ctx
	cfg := config{
		DeepWikiURL:        defaultDeepWikiURL,
		DeepWikiTimeoutSec: defaultDeepWikiTimeout,
	}
	if len(configJSON) > 0 {
		_ = json.Unmarshal(configJSON, &cfg)
	}
	cfg.DatabaseURL = strings.TrimSpace(cfg.DatabaseURL)
	cfg.DeepWikiURL = strings.TrimSpace(cfg.DeepWikiURL)
	if cfg.DeepWikiURL == "" {
		cfg.DeepWikiURL = defaultDeepWikiURL
	}
	if cfg.DeepWikiTimeoutSec <= 0 {
		cfg.DeepWikiTimeoutSec = defaultDeepWikiTimeout
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	if p.cfg.DatabaseURL != cfg.DatabaseURL || p.db == nil {
		if p.db != nil {
			_ = p.db.Close()
			p.db = nil
		}
		if cfg.DatabaseURL != "" {
			store, err := openWikiStore(cfg.DatabaseURL)
			if err != nil {
				hclog.L().Error("[Wiki] 数据库连接失败", "error", err)
			} else {
				p.db = store
				hclog.L().Info("[Wiki] 数据库连接成功")
			}
		}
	}

	p.cfg = cfg
	return nil
}

func (p *WikiPlugin) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (p *WikiPlugin) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	switch listenerID {
	case "cmd.setwiki":
		return p.handleSetWiki(ctx, eventRaw, match)
	case "cmd.wiki":
		return p.handleWiki(ctx, eventRaw, match)
	default:
		return papi.HandleResult{}, nil
	}
}

func (p *WikiPlugin) Shutdown(ctx context.Context) error {
	_ = ctx
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.db != nil {
		err := p.db.Close()
		p.db = nil
		return err
	}
	return nil
}

func (p *WikiPlugin) Status(ctx context.Context) (string, error) {
	_ = ctx
	return "OK", nil
}

func (p *WikiPlugin) handleSetWiki(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	evt, err := parseEvent(eventRaw)
	if err != nil {
		log.Error("[Wiki] 解析 setwiki 事件失败", "error", err)
		return papi.HandleResult{}, nil
	}

	host := transport.Host()
	if host == nil {
		log.Warn("[Wiki] host 为 nil，终止 setwiki")
		return papi.HandleResult{}, nil
	}

	defer func() {
		if r := recover(); r != nil {
			util.SendError(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ setwiki 异常", fmt.Errorf("panic: %v", r))
		}
	}()

	if evt.MsgType != "group" {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ setwiki 只能在群聊中使用")
		return papi.HandleResult{}, nil
	}

	groupID := anyToInt64(evt.GroupID)
	userID := anyToInt64(evt.UserID)
	if groupID <= 0 || userID <= 0 {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 无法识别群或用户")
		return papi.HandleResult{}, nil
	}

	owner, repo := parseSetWikiArgs(evt.Content, match)
	if owner == "" || repo == "" {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 参数不正确，请发送 setwiki owner/repo")
		return papi.HandleResult{}, nil
	}

	ok, err := p.isGroupAdmin(ctx, host, groupID, userID)
	if err != nil {
		util.SendError(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 权限校验失败", err)
		return papi.HandleResult{}, nil
	}
	if !ok {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 仅群主或管理员可设置本群默认仓库")
		return papi.HandleResult{}, nil
	}

	p.mu.RLock()
	store := p.db
	p.mu.RUnlock()
	if store == nil {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 数据库未配置，请在插件配置中填写 database_url")
		return papi.HandleResult{}, nil
	}

	if err := store.SetGroupRepo(ctx, groupID, owner, repo, userID); err != nil {
		util.SendError(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 保存仓库失败", err)
		return papi.HandleResult{}, nil
	}

	util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID,
		fmt.Sprintf("✅ 已设置本群默认仓库：%s/%s", owner, repo))
	return papi.HandleResult{}, nil
}

func (p *WikiPlugin) handleWiki(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	evt, err := parseEvent(eventRaw)
	if err != nil {
		log.Error("[Wiki] 解析 wiki 事件失败", "error", err)
		return papi.HandleResult{}, nil
	}

	host := transport.Host()
	if host == nil {
		log.Warn("[Wiki] host 为 nil，终止 wiki")
		return papi.HandleResult{}, nil
	}

	defer func() {
		if r := recover(); r != nil {
			util.SendError(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ wiki 异常", fmt.Errorf("panic: %v", r))
		}
	}()

	owner, repo, question := parseWikiArgs(evt.Content, match)
	if question == "" {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 请提供问题，例如：wiki 这个项目是做什么的？")
		return papi.HandleResult{}, nil
	}

	if owner == "" || repo == "" {
		if evt.MsgType != "group" {
			util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 私聊请指定仓库：wiki owner/repo 问题")
			return papi.HandleResult{}, nil
		}
		groupID := anyToInt64(evt.GroupID)
		if groupID <= 0 {
			util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 无法识别群")
			return papi.HandleResult{}, nil
		}
		p.mu.RLock()
		store := p.db
		p.mu.RUnlock()
		if store == nil {
			util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 数据库未配置，请先配置 database_url 并 setwiki")
			return papi.HandleResult{}, nil
		}
		var found bool
		owner, repo, found, err = store.GetGroupRepo(ctx, groupID)
		if err != nil {
			util.SendError(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 读取默认仓库失败", err)
			return papi.HandleResult{}, nil
		}
		if !found {
			util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ 本群尚未设置默认仓库，请管理员发送 setwiki owner/repo")
			return papi.HandleResult{}, nil
		}
	}

	repoName := owner + "/" + repo
	p.mu.RLock()
	deepwikiURL := p.cfg.DeepWikiURL
	timeoutSec := p.cfg.DeepWikiTimeoutSec
	p.mu.RUnlock()

	util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID,
		fmt.Sprintf("🔎 正在查询 %s …", repoName))

	answer, err := askDeepWiki(ctx, deepwikiURL, repoName, question, time.Duration(timeoutSec)*time.Second)
	if err != nil {
		util.SendError(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ DeepWiki 查询失败", err)
		return papi.HandleResult{}, nil
	}
	answer = strings.TrimSpace(answer)
	if answer == "" {
		util.SendText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, "❌ DeepWiki 未返回内容")
		return papi.HandleResult{}, nil
	}

	header := fmt.Sprintf("📚 %s\n\n", repoName)
	sendLongText(ctx, host, evt.MsgType, evt.GroupID, evt.UserID, header+answer)
	return papi.HandleResult{}, nil
}

func (p *WikiPlugin) isGroupAdmin(ctx context.Context, host util.HostCaller, groupID, userID int64) (bool, error) {
	member, err := callOneBotJSON[groupMemberInfo](ctx, host, "get_group_member_info", map[string]any{
		"group_id": groupID,
		"user_id":  userID,
		"no_cache": true,
	})
	if err != nil {
		return false, err
	}
	role := strings.ToLower(strings.TrimSpace(member.Role))
	return role == "owner" || role == "admin", nil
}

func parseSetWikiArgs(content string, match *papi.CommandMatch) (owner, repo string) {
	if match != nil && len(match.Groups) >= 2 {
		owner = strings.TrimSpace(match.Groups[0])
		repo = strings.TrimSpace(match.Groups[1])
		if owner != "" && repo != "" {
			return owner, repo
		}
	}
	m := setWikiRegex.FindStringSubmatch(strings.TrimSpace(content))
	if len(m) >= 3 {
		return strings.TrimSpace(m[1]), strings.TrimSpace(m[2])
	}
	return "", ""
}

func parseWikiArgs(content string, match *papi.CommandMatch) (owner, repo, question string) {
	if match != nil && len(match.Groups) >= 3 {
		owner = strings.TrimSpace(match.Groups[0])
		repo = strings.TrimSpace(match.Groups[1])
		question = strings.TrimSpace(match.Groups[2])
		if question != "" {
			return owner, repo, question
		}
	}
	m := wikiRegex.FindStringSubmatch(strings.TrimSpace(content))
	if len(m) >= 4 {
		return strings.TrimSpace(m[1]), strings.TrimSpace(m[2]), strings.TrimSpace(m[3])
	}
	return "", "", ""
}

type eventInfo struct {
	MsgType string
	GroupID any
	UserID  any
	Content string
}

func parseEvent(eventRaw ob11.Event) (eventInfo, error) {
	var evt map[string]any
	dec := json.NewDecoder(bytes.NewReader(eventRaw))
	dec.UseNumber()
	if err := dec.Decode(&evt); err != nil {
		return eventInfo{}, err
	}
	content := strings.TrimSpace(anyToString(evt["content"]))
	if content == "" {
		content = strings.TrimSpace(anyToString(evt["raw_message"]))
	}
	return eventInfo{
		MsgType: strings.TrimSpace(anyToString(evt["message_type"])),
		GroupID: evt["group_id"],
		UserID:  evt["user_id"],
		Content: content,
	}, nil
}

func sendLongText(ctx context.Context, host util.HostCaller, msgType string, groupID, userID any, text string) {
	chunks := splitByRunes(text, maxMessageRunes)
	for _, chunk := range chunks {
		util.SendText(ctx, host, msgType, groupID, userID, chunk)
	}
}

func splitByRunes(s string, limit int) []string {
	s = strings.TrimSpace(s)
	if s == "" {
		return nil
	}
	if limit <= 0 || utf8.RuneCountInString(s) <= limit {
		return []string{s}
	}
	runes := []rune(s)
	out := make([]string, 0, (len(runes)/limit)+1)
	for len(runes) > 0 {
		n := limit
		if n > len(runes) {
			n = len(runes)
		}
		// 尽量在换行处分片
		if n < len(runes) {
			if idx := lastIndexRune(runes[:n], '\n'); idx > limit/2 {
				n = idx + 1
			}
		}
		out = append(out, strings.TrimSpace(string(runes[:n])))
		runes = runes[n:]
		for len(runes) > 0 && runes[0] == '\n' {
			runes = runes[1:]
		}
	}
	return out
}

func lastIndexRune(runes []rune, r rune) int {
	for i := len(runes) - 1; i >= 0; i-- {
		if runes[i] == r {
			return i
		}
	}
	return -1
}

func callOneBotJSON[T any](ctx context.Context, host util.HostCaller, action string, params any) (T, error) {
	var zero T
	resp, err := host.CallOneBot(ctx, action, params)
	if err != nil {
		return zero, err
	}
	if resp.RetCode != 0 || (resp.Status != "" && !strings.EqualFold(resp.Status, "ok")) {
		msg := firstNonEmpty(resp.Wording, resp.Msg, resp.Status, strconv.Itoa(resp.RetCode))
		return zero, fmt.Errorf("%s 返回失败: %s", action, msg)
	}
	if len(resp.Data) == 0 {
		return zero, fmt.Errorf("%s 返回空数据", action)
	}
	var out T
	dec := json.NewDecoder(bytes.NewReader(resp.Data))
	dec.UseNumber()
	if err := dec.Decode(&out); err != nil {
		return zero, fmt.Errorf("解析 %s 返回失败: %w", action, err)
	}
	return out, nil
}

func anyToInt64(v any) int64 {
	switch x := v.(type) {
	case nil:
		return 0
	case int64:
		return x
	case int:
		return int64(x)
	case int32:
		return int64(x)
	case float64:
		return int64(x)
	case json.Number:
		n, _ := x.Int64()
		return n
	case string:
		n, _ := strconv.ParseInt(strings.TrimSpace(x), 10, 64)
		return n
	default:
		return 0
	}
}

func anyToString(v any) string {
	switch x := v.(type) {
	case nil:
		return ""
	case string:
		return x
	case json.Number:
		return x.String()
	case fmt.Stringer:
		return x.String()
	default:
		return fmt.Sprint(x)
	}
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value != "" {
			return value
		}
	}
	return ""
}

func main() {
	logger := hclog.New(&hclog.LoggerOptions{Name: "nyanyabot-plugin-amiabot-wiki", Level: hclog.Info})
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &WikiPlugin{}},
		},
		Logger: logger,
	})
}
