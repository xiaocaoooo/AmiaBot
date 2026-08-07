package main

import (
	"context"
	"encoding/json"
	"testing"

	"github.com/hashicorp/go-hclog"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
)

func TestCoreWSClientRules(t *testing.T) {
	// 初始化客户端
	client := &OneBotWSClient{
		logger:        hclog.NewNullLogger(),
		compiledUps:   make([]compiledRule, 0),
		compiledDowns: make([]compiledRule, 0),
	}

	// 配置过滤规则
	config := wsConfig{
		WSURL:   "ws://127.0.0.1:12345/ws",
		WSToken: "mysecret",
		UpstreamRules: []rule{
			{Expression: "post_type = 'message' and message_type = 'private'", Action: "reject"},
			{Expression: "post_type = 'message'", Action: "passthrough"},
		},
		DownstreamRules: []rule{
			{Expression: "action = 'get_status'", Action: "match", ReplyJSON: `{"status": "ok", "retcode": 0}`},
			{Expression: "action = 'private_api'", Action: "reject"},
		},
	}

	configBytes, _ := json.Marshal(config)
	err := client.Configure(context.Background(), configBytes)
	if err != nil {
		t.Fatalf("failed to configure client: %v", err)
	}

	// 1. 测试上游过滤
	// a. 应该被 reject
	privateEvent := ob11.Event(`{"post_type": "message", "message_type": "private", "message": "hello"}`)
	client.sendChan = make(chan []byte, 10)
	client.running = true
	_, err = client.Handle(context.Background(), "evt.onebot-ws.message", privateEvent, nil)
	if err != nil {
		t.Fatalf("handle returned error: %v", err)
	}
	if len(client.sendChan) != 0 {
		t.Errorf("expected private message event to be rejected/dropped, but it was queued")
	}

	// b. 应该被 passthrough
	groupEvent := ob11.Event(`{"post_type": "message", "message_type": "group", "message": "hello"}`)
	_, err = client.Handle(context.Background(), "evt.onebot-ws.message", groupEvent, nil)
	if err != nil {
		t.Fatalf("handle returned error: %v", err)
	}
	if len(client.sendChan) != 1 {
		t.Errorf("expected group message event to be passed through, but it was not queued")
	}

	// 2. 测试下游消息过滤
	// a. match (返回指定 json 并注入 echo)
	mockWSRecv := []byte(`{"action": "get_status", "echo": "xyz"}`)
	
	// 由于 WebSocket 连接没有真实建立，我们可以直接测试 handleDownstreamMessage 发送 mock 响应和 reject 响应
	client.sendChan = make(chan []byte, 10) // 充当 queueWrite 队列

	client.handleDownstreamMessage(mockWSRecv)

	if len(client.sendChan) != 1 {
		t.Fatalf("expected 1 mock response queued, got %d", len(client.sendChan))
	}
	resp := <-client.sendChan
	var respObj map[string]interface{}
	_ = json.Unmarshal(resp, &respObj)

	if respObj["status"] != "ok" || respObj["retcode"] != 0.0 || respObj["echo"] != "xyz" {
		t.Errorf("unexpected mock reply content: %s", string(resp))
	}

	// b. reject
	rejectWSRecv := []byte(`{"action": "private_api", "echo": "999"}`)
	client.handleDownstreamMessage(rejectWSRecv)

	if len(client.sendChan) != 1 {
		t.Fatalf("expected 1 error response queued, got %d", len(client.sendChan))
	}
	resp = <-client.sendChan
	_ = json.Unmarshal(resp, &respObj)
	if respObj["status"] != "failed" || respObj["echo"] != "999" {
		t.Errorf("unexpected reject reply content: %s", string(resp))
	}
}
