package main

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/cookiejar"
	"strings"
	"time"
)

// askDeepWiki 同步调用 DeepWiki MCP：initialize → notifications/initialized → tools/call ask_question。
func askDeepWiki(ctx context.Context, endpoint, repo, question string, timeout time.Duration) (string, error) {
	endpoint = strings.TrimSpace(endpoint)
	if endpoint == "" {
		return "", fmt.Errorf("deepwiki url is empty")
	}
	repo = strings.TrimSpace(repo)
	question = strings.TrimSpace(question)
	if repo == "" || question == "" {
		return "", fmt.Errorf("repo and question are required")
	}
	if timeout <= 0 {
		timeout = time.Duration(defaultDeepWikiTimeout) * time.Second
	}

	callCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	// 每次会话使用独立 cookie jar，保持 MCP session 一致性。
	jar, err := cookiejar.New(nil)
	if err != nil {
		return "", fmt.Errorf("create cookie jar: %w", err)
	}
	client := &http.Client{Jar: jar, Timeout: 0}
	session := &mcpSession{client: client, endpoint: endpoint}

	// ① initialize
	initPayload := map[string]any{
		"jsonrpc": "2.0",
		"id":      newRequestID(),
		"method":  "initialize",
		"params": map[string]any{
			"protocolVersion": "2024-11-05",
			"capabilities":    map[string]any{},
			"clientInfo": map[string]any{
				"name":    "amiabot-wiki-plugin",
				"version": "1.0.0",
			},
		},
	}
	initBody, err := session.post(callCtx, initPayload, 30*time.Second)
	if err != nil {
		return "", fmt.Errorf("initialize request failed: %w", err)
	}
	if !hasProtocolVersion(initBody) {
		return "", fmt.Errorf("MCP initialize failed: server did not return valid protocol version")
	}

	// ② notifications/initialized
	_, _ = session.post(callCtx, map[string]any{
		"jsonrpc": "2.0",
		"method":  "notifications/initialized",
	}, 10*time.Second)

	// ③ tools/call ask_question（使用剩余超时）
	askPayload := map[string]any{
		"jsonrpc": "2.0",
		"id":      newRequestID(),
		"method":  "tools/call",
		"params": map[string]any{
			"name": "ask_question",
			"arguments": map[string]any{
				"repoName": repo,
				"question": question,
			},
		},
	}
	askBody, err := session.post(callCtx, askPayload, 0)
	if err != nil {
		return "", fmt.Errorf("ask_question request failed: %w", err)
	}

	texts := extractSSETextContents(askBody)
	answer := strings.TrimSpace(strings.Join(texts, "\n"))
	if answer == "" {
		if errMsg := extractSSEError(askBody); errMsg != "" {
			return "", fmt.Errorf("deepwiki error: %s", errMsg)
		}
		return "", fmt.Errorf("deepwiki returned empty answer")
	}
	return answer, nil
}

type mcpSession struct {
	client    *http.Client
	endpoint  string
	sessionID string
}

func (s *mcpSession) post(ctx context.Context, payload any, hardTimeout time.Duration) ([]byte, error) {
	body, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}

	reqCtx := ctx
	var cancel context.CancelFunc
	if hardTimeout > 0 {
		reqCtx, cancel = context.WithTimeout(ctx, hardTimeout)
		defer cancel()
	}

	req, err := http.NewRequestWithContext(reqCtx, http.MethodPost, s.endpoint, bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json, text/event-stream")
	if s.sessionID != "" {
		req.Header.Set("Mcp-Session-Id", s.sessionID)
	}

	resp, err := s.client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if sid := strings.TrimSpace(resp.Header.Get("Mcp-Session-Id")); sid != "" {
		s.sessionID = sid
	}

	raw, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		snippet := string(raw)
		if len(snippet) > 200 {
			snippet = snippet[:200] + "..."
		}
		return nil, fmt.Errorf("http %d: %s", resp.StatusCode, snippet)
	}
	return raw, nil
}

func hasProtocolVersion(body []byte) bool {
	for _, data := range iterSSEData(body) {
		var msg map[string]any
		if err := json.Unmarshal(data, &msg); err != nil {
			continue
		}
		result, _ := msg["result"].(map[string]any)
		if result == nil {
			continue
		}
		if pv, ok := result["protocolVersion"].(string); ok && strings.TrimSpace(pv) != "" {
			return true
		}
	}
	// 兼容非 SSE 的纯 JSON 响应
	var msg map[string]any
	if err := json.Unmarshal(body, &msg); err == nil {
		if result, ok := msg["result"].(map[string]any); ok {
			if pv, ok := result["protocolVersion"].(string); ok && strings.TrimSpace(pv) != "" {
				return true
			}
		}
	}
	return false
}

// extractSSETextContents 从 MCP tools/call 的 SSE 响应中提取 text 内容。
func extractSSETextContents(body []byte) []string {
	var texts []string
	for _, data := range iterSSEData(body) {
		texts = append(texts, extractTextFromJSON(data)...)
	}
	// 兼容纯 JSON
	if len(texts) == 0 {
		texts = append(texts, extractTextFromJSON(body)...)
	}
	return texts
}

func extractTextFromJSON(data []byte) []string {
	var msg map[string]any
	if err := json.Unmarshal(data, &msg); err != nil {
		return nil
	}
	result, _ := msg["result"].(map[string]any)
	if result == nil {
		return nil
	}
	content, _ := result["content"].([]any)
	if content == nil {
		return nil
	}
	var texts []string
	for _, item := range content {
		obj, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if typ, _ := obj["type"].(string); typ != "text" {
			continue
		}
		if text, ok := obj["text"].(string); ok && text != "" {
			texts = append(texts, text)
		}
	}
	return texts
}

func extractSSEError(body []byte) string {
	for _, data := range iterSSEData(body) {
		var msg map[string]any
		if err := json.Unmarshal(data, &msg); err != nil {
			continue
		}
		if errObj, ok := msg["error"].(map[string]any); ok {
			if m, ok := errObj["message"].(string); ok && strings.TrimSpace(m) != "" {
				return strings.TrimSpace(m)
			}
		}
	}
	var msg map[string]any
	if err := json.Unmarshal(body, &msg); err == nil {
		if errObj, ok := msg["error"].(map[string]any); ok {
			if m, ok := errObj["message"].(string); ok {
				return strings.TrimSpace(m)
			}
		}
	}
	return ""
}

func iterSSEData(body []byte) [][]byte {
	lines := bytes.Split(body, []byte("\n"))
	out := make([][]byte, 0, len(lines))
	for _, line := range lines {
		line = bytes.TrimSpace(line)
		if !bytes.HasPrefix(line, []byte("data:")) {
			continue
		}
		data := bytes.TrimSpace(bytes.TrimPrefix(line, []byte("data:")))
		if len(data) == 0 || bytes.Equal(data, []byte("[DONE]")) {
			continue
		}
		out = append(out, data)
	}
	return out
}

func newRequestID() string {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		return fmt.Sprintf("%d", time.Now().UnixNano())
	}
	return hex.EncodeToString(b[:])
}
