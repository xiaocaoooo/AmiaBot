package main

import (
	"testing"

	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
)

func TestExtractPixivArtworkID(t *testing.T) {
	tests := []struct {
		name  string
		raw   string
		match *papi.CommandMatch
		want  string
	}{
		{
			name: "match groups 优先",
			raw:  "",
			match: &papi.CommandMatch{
				Groups: []string{"125547965"},
			},
			want: "125547965",
		},
		{
			name: "标准 artworks 链接",
			raw:  "https://www.pixiv.net/artworks/123456",
			want: "123456",
		},
		{
			name: "无 scheme 链接",
			raw:  "pixiv.net/artworks/42",
			want: "42",
		},
		{
			name: "带语言前缀",
			raw:  "https://www.pixiv.net/en/artworks/987654",
			want: "987654",
		},
		{
			name: "带 query 和 fragment",
			raw:  "看看这个 https://pixiv.net/artworks/24680?foo=bar#baz",
			want: "24680",
		},
		{
			name: "尾部带斜杠",
			raw:  "https://www.pixiv.net/artworks/13579/",
			want: "13579",
		},
		{
			name: "非 artworks 链接",
			raw:  "https://www.pixiv.net/users/123456",
			want: "",
		},
		{
			name: "非法 pid",
			raw:  "https://www.pixiv.net/artworks/abc",
			want: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractPixivArtworkID(tt.raw, tt.match)
			if got != tt.want {
				t.Fatalf("extractPixivArtworkID() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestBuildPixivPageURL(t *testing.T) {
	tests := []struct {
		name      string
		pagesHost string
		pid       string
		want      string
	}{
		{
			name:      "裸 host 自动补 http",
			pagesHost: "pages.example.com",
			pid:       "123",
			want:      "http://pages.example.com/pixiv/illust/info?pid=123",
		},
		{
			name:      "保留 https",
			pagesHost: "https://pages.example.com/",
			pid:       "456",
			want:      "https://pages.example.com/pixiv/illust/info?pid=456",
		},
		{
			name:      "保留 base path",
			pagesHost: "https://pages.example.com/base/",
			pid:       "789",
			want:      "https://pages.example.com/base/pixiv/illust/info?pid=789",
		},
		{
			name:      "空 pid 返回空",
			pagesHost: "https://pages.example.com",
			pid:       "",
			want:      "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := buildPixivPageURL(tt.pagesHost, tt.pid)
			if got != tt.want {
				t.Fatalf("buildPixivPageURL() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestResolvePixivMediaBase(t *testing.T) {
	tests := []struct {
		name         string
		pagesHost    string
		downloadBase string
		want         string
	}{
		{
			name:         "优先使用下载基地址",
			pagesHost:    "http://amiabot-pages:8080",
			downloadBase: "http://127.0.0.1:25000",
			want:         "http://127.0.0.1:25000",
		},
		{
			name:         "为空时回退页面地址",
			pagesHost:    "http://amiabot-pages:8080",
			downloadBase: "",
			want:         "http://amiabot-pages:8080",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := resolvePixivMediaBase(tt.pagesHost, tt.downloadBase)
			if got != tt.want {
				t.Fatalf("resolvePixivMediaBase() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestBuildPagesAssetURL(t *testing.T) {
	tests := []struct {
		name      string
		pagesHost string
		assetPath string
		want      string
	}{
		{
			name:      "拼接相对资源路径",
			pagesHost: "https://pages.example.com/base",
			assetPath: "/pixiv/image?url=https%3A%2F%2Fi.pximg.net%2Fa.jpg",
			want:      "https://pages.example.com/base/pixiv/image?url=https%3A%2F%2Fi.pximg.net%2Fa.jpg",
		},
		{
			name:      "绝对路径直接返回",
			pagesHost: "https://pages.example.com/base",
			assetPath: "https://cdn.example.com/a.gif",
			want:      "https://cdn.example.com/a.gif",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := buildPagesAssetURL(tt.pagesHost, tt.assetPath)
			if got != tt.want {
				t.Fatalf("buildPagesAssetURL() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestBuildPixivForwardNodes(t *testing.T) {
	nodes := buildPixivForwardNodes([]string{"https://example.com/p0.jpg", "https://example.com/p1.jpg"}, int64(123456), "AmiaBot Pixiv")
	if len(nodes) != 2 {
		t.Fatalf("unexpected node count: got=%d", len(nodes))
	}
	firstData, ok := nodes[0]["data"].(map[string]any)
	if !ok {
		t.Fatalf("unexpected first node data: %#v", nodes[0])
	}
	if firstData["user_id"] != int64(123456) {
		t.Fatalf("unexpected user_id: %#v", firstData["user_id"])
	}
	if firstData["nickname"] != "AmiaBot Pixiv" {
		t.Fatalf("unexpected nickname: %#v", firstData["nickname"])
	}
	content, ok := firstData["content"].([]map[string]any)
	if !ok {
		t.Fatalf("unexpected content type: %#v", firstData["content"])
	}
	if text := content[0]["data"].(map[string]any)["text"]; text != "P1 / 2" {
		t.Fatalf("unexpected first label: %#v", text)
	}
	if file := content[1]["data"].(map[string]any)["file"]; file != "https://example.com/p0.jpg" {
		t.Fatalf("unexpected first file: %#v", file)
	}
}
