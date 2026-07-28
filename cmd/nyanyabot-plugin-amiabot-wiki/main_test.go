package main

import (
	"strings"
	"testing"

	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
)

func TestParseSetWikiArgs(t *testing.T) {
	owner, repo := parseSetWikiArgs("setwiki xiaocaoooo/AmiaBot", nil)
	if owner != "xiaocaoooo" || repo != "AmiaBot" {
		t.Fatalf("got %q/%q", owner, repo)
	}

	owner, repo = parseSetWikiArgs("SETWIKI owner/repo-name", &papi.CommandMatch{
		Groups: []string{"owner", "repo-name"},
	})
	if owner != "owner" || repo != "repo-name" {
		t.Fatalf("match groups: got %q/%q", owner, repo)
	}

	owner, repo = parseSetWikiArgs("setwiki bad", nil)
	if owner != "" || repo != "" {
		t.Fatalf("expected empty for invalid input, got %q/%q", owner, repo)
	}
}

func TestParseWikiArgs(t *testing.T) {
	owner, repo, q := parseWikiArgs("wiki 这个项目是做什么的？", nil)
	if owner != "" || repo != "" {
		t.Fatalf("expected empty repo, got %s/%s", owner, repo)
	}
	if q != "这个项目是做什么的？" {
		t.Fatalf("question = %q", q)
	}

	owner, repo, q = parseWikiArgs("wiki xiaocaoooo/AmiaBot 插件如何添加？", nil)
	if owner != "xiaocaoooo" || repo != "AmiaBot" {
		t.Fatalf("repo = %s/%s", owner, repo)
	}
	if q != "插件如何添加？" {
		t.Fatalf("question = %q", q)
	}

	owner, repo, q = parseWikiArgs("WIKI owner/repo hello world with spaces", nil)
	if owner != "owner" || repo != "repo" || q != "hello world with spaces" {
		t.Fatalf("got %s/%s %q", owner, repo, q)
	}

	// 可选组为空时 match.Groups 仍可能有 3 个槽位
	owner, repo, q = parseWikiArgs("wiki 仅问题", &papi.CommandMatch{
		Groups: []string{"", "", "仅问题"},
	})
	if owner != "" || repo != "" || q != "仅问题" {
		t.Fatalf("groups: got %s/%s %q", owner, repo, q)
	}
}

func TestExtractSSETextContents(t *testing.T) {
	body := []byte(strings.Join([]string{
		"event: message",
		`data: {"jsonrpc":"2.0","id":"1","result":{"content":[{"type":"text","text":"hello"},{"type":"text","text":"world"}]}}`,
		"",
		`data: {"jsonrpc":"2.0","id":"2","result":{"content":[{"type":"image","data":"x"}]}}`,
	}, "\n"))

	texts := extractSSETextContents(body)
	if len(texts) != 2 || texts[0] != "hello" || texts[1] != "world" {
		t.Fatalf("texts = %#v", texts)
	}
}

func TestExtractSSETextContentsPlainJSON(t *testing.T) {
	body := []byte(`{"jsonrpc":"2.0","result":{"content":[{"type":"text","text":"plain"}]}}`)
	texts := extractSSETextContents(body)
	if len(texts) != 1 || texts[0] != "plain" {
		t.Fatalf("texts = %#v", texts)
	}
}

func TestHasProtocolVersion(t *testing.T) {
	ok := hasProtocolVersion([]byte(`data: {"result":{"protocolVersion":"2024-11-05"}}` + "\n"))
	if !ok {
		t.Fatal("expected protocol version from SSE")
	}
	ok = hasProtocolVersion([]byte(`{"result":{"protocolVersion":"2024-11-05"}}`))
	if !ok {
		t.Fatal("expected protocol version from JSON")
	}
	ok = hasProtocolVersion([]byte(`data: {"result":{}}` + "\n"))
	if ok {
		t.Fatal("expected false without protocol version")
	}
}

func TestSplitByRunes(t *testing.T) {
	chunks := splitByRunes("abc", 10)
	if len(chunks) != 1 || chunks[0] != "abc" {
		t.Fatalf("short: %#v", chunks)
	}
	// 优先在换行处分片
	text := strings.Repeat("a", 20) + "\n" + strings.Repeat("b", 20)
	chunks = splitByRunes(text, 25)
	if len(chunks) < 2 {
		t.Fatalf("expected multiple chunks, got %#v", chunks)
	}
	joined := strings.Join(chunks, "")
	if !strings.Contains(joined, "a") || !strings.Contains(joined, "b") {
		t.Fatalf("lost content: %#v", chunks)
	}
}
