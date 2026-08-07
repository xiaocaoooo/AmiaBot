package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
		"sync"
	"time"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"nhooyr.io/websocket"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
	"github.com/xiaocaoooo/amiabot/cmd/nyanyabot-plugin-amiabot-onebot-websocket-client/jsonata"
)

type rule struct {
	Expression string `json:"expression"`
	Action     string `json:"action"`
	ReplyJSON  string `json:"reply_json,omitempty"`
}

type wsConfig struct {
	WSURL           string `json:"ws_url"`
	WSToken         string `json:"ws_token"`
	UpstreamRules   []rule `json:"upstream_rules"`
	DownstreamRules []rule `json:"downstream_rules"`
}

type compiledRule struct {
	expression string
	action     string
	replyJSON  string
	expr       *jsonata.Expr
}

type OneBotWSClient struct {
	mu            sync.RWMutex
	cfg           wsConfig
	compiledUps   []compiledRule
	compiledDowns []compiledRule

	wsConn     *websocket.Conn
	connCtx    context.Context
	connCancel context.CancelFunc
	logger     hclog.Logger
	running    bool
	sendChan   chan []byte
}

func (p *OneBotWSClient) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx

	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"ws_url":{"type":"string","description":"下游 WebSocket 服务端 URL，例如 ws://127.0.0.1:8080/onebot"},
			"ws_token":{"type":"string","description":"下游 WebSocket 连接所需的 Access Token / Authorization Bearer"},
			"upstream_rules":{
				"type":"array",
				"description":"上游消息（NyaNyaBot 事件 -> 下游 WS）的过滤规则列表。按顺序匹配，首个匹配的规则生效。若无规则匹配，默认透传。",
				"items":{
					"type":"object",
					"properties":{
						"expression":{"type":"string","description":"JSONata 过滤表达式，例如：post_type = 'message' and message_type = 'group'"},
						"action":{"type":"string","enum":["passthrough","reject"],"description":"执行动作：passthrough (透传) 或 reject (拒绝)"}
					},
					"required":["expression","action"]
				}
			},
			"downstream_rules":{
				"type":"array",
				"description":"下游消息（下游 WS 动作请求 -> NyaNyaBot）的过滤规则列表。按顺序匹配，首个匹配的规则生效。若无规则匹配，默认透传。",
				"items":{
					"type":"object",
					"properties":{
						"expression":{"type":"string","description":"JSONata 过滤表达式，例如：action = 'send_msg'"},
						"action":{"type":"string","enum":["passthrough","reject","match"],"description":"执行动作：passthrough (透传/调用 OneBot 并返回)、reject (拒绝/返回错误) 或 match (直接返回静态 JSON)"},
						"reply_json":{"type":"string","description":"当 action 为 match 时，直接返回给下游的 JSON 字符串或对象模板"}
					},
					"required":["expression","action"]
				}
			}
		},
		"additionalProperties":true
	}`)

	def := json.RawMessage(`{
		"ws_url": "",
		"ws_token": "",
		"upstream_rules": [],
		"downstream_rules": []
	}`)

	return papi.Descriptor{
		Name:         "OneBot WebSocket Client",
		PluginID:     "external.amiabot-onebot-websocket-client",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "OneBot WebSocket 客户端插件，实现上游 NyaNyaBot 事件与下游 WebSocket 消息的双向过滤与透传。",
		Dependencies: []string{},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "OneBot WebSocket Client 插件配置",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{},
		Events: []papi.EventListener{
			{
				Name:        "ws-client-message",
				ID:          "evt.onebot-ws.message",
				Description: "捕获所有上游 message 消息事件",
				Event:       "message",
				Handler:     "HandleEvent",
			},
			{
				Name:        "ws-client-notice",
				ID:          "evt.onebot-ws.notice",
				Description: "捕获所有上游 notice 通知事件",
				Event:       "notice",
				Handler:     "HandleEvent",
			},
			{
				Name:        "ws-client-request",
				ID:          "evt.onebot-ws.request",
				Description: "捕获所有上游 request 请求事件",
				Event:       "request",
				Handler:     "HandleEvent",
			},
			{
				Name:        "ws-client-meta",
				ID:          "evt.onebot-ws.meta_event",
				Description: "捕获所有上游 meta_event 元事件",
				Event:       "meta_event",
				Handler:     "HandleEvent",
			},
		},
	}, nil
}

func (p *OneBotWSClient) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	p.mu.Lock()
	defer p.mu.Unlock()

	var newCfg wsConfig
	if err := json.Unmarshal(config, &newCfg); err != nil {
		return err
	}

	p.logger.Info("received configuration update", "ws_url", newCfg.WSURL)

	// 编译上游规则
	ups := make([]compiledRule, 0, len(newCfg.UpstreamRules))
	for _, r := range newCfg.UpstreamRules {
		expr, err := jsonata.Compile(r.Expression)
		if err != nil {
			p.logger.Error("failed to compile upstream expression", "expr", r.Expression, "error", err)
			continue
		}
		ups = append(ups, compiledRule{
			expression: r.Expression,
			action:     r.Action,
			expr:       expr,
		})
	}

	// 编译下游规则
	downs := make([]compiledRule, 0, len(newCfg.DownstreamRules))
	for _, r := range newCfg.DownstreamRules {
		expr, err := jsonata.Compile(r.Expression)
		if err != nil {
			p.logger.Error("failed to compile downstream expression", "expr", r.Expression, "error", err)
			continue
		}
		downs = append(downs, compiledRule{
			expression: r.Expression,
			action:     r.Action,
			replyJSON:  r.ReplyJSON,
			expr:       expr,
		})
	}

	urlChanged := newCfg.WSURL != p.cfg.WSURL || newCfg.WSToken != p.cfg.WSToken
	p.cfg = newCfg
	p.compiledUps = ups
	p.compiledDowns = downs

	if p.running && urlChanged {
		p.logger.Info("WebSocket target URL or token changed, restarting client connection...")
		p.stopConnection()
		p.startConnection()
	}

	return nil
}

func (p *OneBotWSClient) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, fmt.Errorf("method not implemented")
}

func (p *OneBotWSClient) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	_ = match

	p.logger.Debug("handle upstream event triggered", "listener_id", listenerID)

	p.mu.RLock()
	compiledUps := p.compiledUps
	running := p.running
	sendChan := p.sendChan
	p.mu.RUnlock()

	if !running || sendChan == nil {
		return papi.HandleResult{}, nil
	}

	// 解析事件数据用于评估
	var data interface{}
	if err := json.Unmarshal(eventRaw, &data); err != nil {
		p.logger.Error("failed to unmarshal event raw JSON", "error", err)
		return papi.HandleResult{}, nil
	}

	// 评估上游规则
	passthrough := true
	for _, rule := range compiledUps {
		val, err := rule.expr.Eval(data)
		if err != nil {
			p.logger.Warn("failed to evaluate upstream rule", "expr", rule.expression, "error", err)
			continue
		}
		if isTruthy(val) {
			p.logger.Info("upstream rule matched", "expr", rule.expression, "action", rule.action)
			if rule.action == "reject" {
				passthrough = false
			} else if rule.action == "passthrough" {
				passthrough = true
			}
			break
		}
	}

	if passthrough {
		// 发送到 WebSocket 写入队列
		select {
		case sendChan <- eventRaw:
			p.logger.Debug("upstream event queued for sending to downstream WS")
		default:
			p.logger.Warn("upstream event send queue is full, dropping event")
		}
	} else {
		p.logger.Info("upstream event dropped by rules")
	}

	return papi.HandleResult{}, nil
}

func (p *OneBotWSClient) Status(ctx context.Context) (string, error) {
	_ = ctx
	p.mu.RLock()
	defer p.mu.RUnlock()
	if p.running {
		return "running", nil
	}
	return "stopped", nil
}

func (p *OneBotWSClient) Shutdown(ctx context.Context) error {
	_ = ctx
	p.mu.Lock()
	defer p.mu.Unlock()
	p.logger.Info("shutting down plugin")
	p.stopConnection()
	return nil
}

func (p *OneBotWSClient) startConnection() {
	if p.cfg.WSURL == "" {
		p.logger.Warn("ws_url is empty, skipping WebSocket client connection start")
		return
	}

	p.connCtx, p.connCancel = context.WithCancel(context.Background())
	p.sendChan = make(chan []byte, 1024)
	p.running = true

	go p.connectionLoop(p.connCtx)
}

func (p *OneBotWSClient) stopConnection() {
	p.running = false
	if p.connCancel != nil {
		p.connCancel()
	}
	if p.wsConn != nil {
		_ = p.wsConn.Close(websocket.StatusNormalClosure, "plugin shutting down")
		p.wsConn = nil
	}
	p.sendChan = nil
}

func (p *OneBotWSClient) connectionLoop(ctx context.Context) {
	delay := 1 * time.Second
	maxDelay := 30 * time.Second

	for {
		select {
		case <-ctx.Done():
			p.logger.Info("connection loop terminated")
			return
		default:
		}

		p.mu.RLock()
		wsURL := p.cfg.WSURL
		wsToken := p.cfg.WSToken
		p.mu.RUnlock()

		if wsURL == "" {
			time.Sleep(1 * time.Second)
			continue
		}

		p.logger.Info("connecting to downstream WS", "url", wsURL)

		opts := &websocket.DialOptions{
			HTTPHeader: make(http.Header),
		}

		if wsToken != "" {
			opts.HTTPHeader.Set("Authorization", "Bearer "+wsToken)
		}

		// 检查 URL 中是否带 token
		u, err := url.Parse(wsURL)
		if err == nil && wsToken != "" && u.Query().Get("access_token") == "" {
			q := u.Query()
			q.Set("access_token", wsToken)
			u.RawQuery = q.Encode()
			wsURL = u.String()
		}

		conn, _, err := websocket.Dial(ctx, wsURL, opts)
		if err != nil {
			p.logger.Error("failed to connect to downstream WS, retrying...", "error", err, "delay", delay)
			select {
			case <-ctx.Done():
				return
			case <-time.After(delay):
			}
			delay *= 2
			if delay > maxDelay {
				delay = maxDelay
			}
			continue
		}

		p.logger.Info("successfully connected to downstream WS")
		delay = 1 * time.Second // 重置退避延迟

		p.mu.Lock()
		p.wsConn = conn
		p.mu.Unlock()

		// 启动读写协程
		wg := sync.WaitGroup{}
		wg.Add(2)

		readCtx, readCancel := context.WithCancel(ctx)

		go func() {
			defer wg.Done()
			p.readLoop(readCtx, conn)
			readCancel()
		}()

		go func() {
			defer wg.Done()
			p.writeLoop(readCtx, conn)
			_ = conn.Close(websocket.StatusAbnormalClosure, "write loop failed")
		}()

		wg.Wait()
		readCancel()

		p.logger.Warn("disconnected from downstream WS, preparing to reconnect")
		p.mu.Lock()
		if p.wsConn == conn {
			p.wsConn = nil
		}
		p.mu.Unlock()

		select {
		case <-ctx.Done():
			return
		case <-time.After(1 * time.Second):
		}
	}
}

func (p *OneBotWSClient) writeLoop(ctx context.Context, conn *websocket.Conn) {
	for {
		select {
		case <-ctx.Done():
			return
		case payload, ok := <-p.sendChan:
			if !ok {
				return
			}
			err := conn.Write(ctx, websocket.MessageText, payload)
			if err != nil {
				p.logger.Error("failed to write message to downstream WS", "error", err)
				return
			}
			p.logger.Debug("successfully sent event payload to downstream WS")
		}
	}
}

type downstreamRequest struct {
	Action string          `json:"action"`
	Params json.RawMessage `json:"params,omitempty"`
	Echo   string          `json:"echo,omitempty"`
}

func (p *OneBotWSClient) readLoop(ctx context.Context, conn *websocket.Conn) {
	for {
		select {
		case <-ctx.Done():
			return
		default:
		}

		_, payload, err := conn.Read(ctx)
		if err != nil {
			p.logger.Error("failed to read from downstream WS", "error", err)
			return
		}

		p.logger.Debug("received message from downstream WS")
		go p.handleDownstreamMessage(payload)
	}
}

func (p *OneBotWSClient) handleDownstreamMessage(payload []byte) {
	p.mu.RLock()
	compiledDowns := p.compiledDowns
	p.mu.RUnlock()

	var data interface{}
	if err := json.Unmarshal(payload, &data); err != nil {
		p.logger.Error("failed to unmarshal downstream payload to map", "error", err)
		return
	}

	var req downstreamRequest
	if err := json.Unmarshal(payload, &req); err != nil {
		p.logger.Error("failed to unmarshal downstream payload to downstreamRequest struct", "error", err)
		return
	}

	passthrough := true
	var matchedRule *compiledRule

	for _, rule := range compiledDowns {
		val, err := rule.expr.Eval(data)
		if err != nil {
			p.logger.Warn("failed to evaluate downstream rule", "expr", rule.expression, "error", err)
			continue
		}
		if isTruthy(val) {
			p.logger.Info("downstream rule matched", "expr", rule.expression, "action", rule.action)
			matchedRule = &rule
			if rule.action == "reject" {
				passthrough = false
			} else if rule.action == "passthrough" {
				passthrough = true
			} else if rule.action == "match" {
				passthrough = false
			}
			break
		}
	}

	if matchedRule != nil && matchedRule.action == "reject" {
		// 返回错误响应
		p.sendDownstreamError(req.Echo, "rejected by rules")
		return
	}

	if matchedRule != nil && matchedRule.action == "match" {
		// 直接返回 reply_json
		p.sendDownstreamMockReply(req.Echo, matchedRule.replyJSON)
		return
	}

	if passthrough {
		// 呼叫 OneBot Host API 并返回结果
		go p.callOneBotAndReply(req)
	}
}

func (p *OneBotWSClient) sendDownstreamError(echo string, message string) {
	resp := ob11.APIResponse{
		Status:  "failed",
		RetCode: 1403,
		Msg:     message,
		Wording: message,
		Echo:    echo,
	}
	b, err := json.Marshal(resp)
	if err != nil {
		p.logger.Error("failed to marshal error response", "error", err)
		return
	}
	p.queueWrite(b)
}

func (p *OneBotWSClient) sendDownstreamMockReply(echo string, replyJSON string) {
	// 解析 replyJSON 以确保它是合法的，并注入 echo 字段
	var m map[string]interface{}
	if err := json.Unmarshal([]byte(replyJSON), &m); err != nil {
		// 降级：如果不是 JSON 对象，直接作为原始字符串处理，或者尝试拼接
		p.logger.Warn("reply_json is not a valid JSON object, sending as raw string", "error", err)
		p.queueWrite([]byte(replyJSON))
		return
	}

	if echo != "" {
		m["echo"] = echo
	}

	b, err := json.Marshal(m)
	if err != nil {
		p.logger.Error("failed to marshal mock response", "error", err)
		return
	}
	p.queueWrite(b)
}

func (p *OneBotWSClient) callOneBotAndReply(req downstreamRequest) {
	host := transport.Host()
	if host == nil {
		p.logger.Error("host RPC client is not initialized")
		p.sendDownstreamError(req.Echo, "host is unavailable")
		return
	}

	var params map[string]interface{}
	if len(req.Params) > 0 {
		_ = json.Unmarshal(req.Params, &params)
	}

	// 呼叫 OneBot API
	apiResp, err := host.CallOneBot(context.Background(), req.Action, params)
	if err != nil {
		p.logger.Error("failed to call OneBot API", "action", req.Action, "error", err)
		p.sendDownstreamError(req.Echo, err.Error())
		return
	}

	// 将宿主接口返回的 APIResponse 发送至下游（保持相同的 echo）
	apiResp.Echo = req.Echo
	b, err := json.Marshal(apiResp)
	if err != nil {
		p.logger.Error("failed to marshal API response", "error", err)
		return
	}

	p.queueWrite(b)
}

func (p *OneBotWSClient) queueWrite(payload []byte) {
	p.mu.RLock()
	sendChan := p.sendChan
	running := p.running
	p.mu.RUnlock()

	if !running || sendChan == nil {
		return
	}

	select {
	case sendChan <- payload:
		p.logger.Debug("queued message to downstream WS")
	default:
		p.logger.Warn("send queue is full, dropping response message")
	}
}

func isTruthy(v interface{}) bool {
	if v == nil {
		return false
	}
	switch val := v.(type) {
	case bool:
		return val
	case string:
		return val != ""
	case int:
		return val != 0
	case int64:
		return val != 0
	case float64:
		return val != 0.0
	}
	return true
}

func main() {
	pluginLogger := hclog.New(&hclog.LoggerOptions{
		Name:  "onebot-websocket-client",
		Level: hclog.Info,
	})

	pluginImpl := &OneBotWSClient{
		logger:        pluginLogger,
		compiledUps:   make([]compiledRule, 0),
		compiledDowns: make([]compiledRule, 0),
	}

	pluginImpl.startConnection()

	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: pluginImpl},
		},
	})
}
