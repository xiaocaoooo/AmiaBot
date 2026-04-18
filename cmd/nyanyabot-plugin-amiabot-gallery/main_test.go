package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
)

func TestParseTags(t *testing.T) {
	got := parseTags(" cat, cover ,Cat,  ,角色 ")
	want := []string{"cat", "cover", "角色"}
	if len(got) != len(want) {
		t.Fatalf("parseTags() len = %d, want %d, got=%v", len(got), len(want), got)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("parseTags()[%d] = %q, want %q", i, got[i], want[i])
		}
	}
}

func TestFindReplyIDInMessage(t *testing.T) {
	message := []any{
		map[string]any{"type": "text", "data": map[string]any{"text": "上传cat"}},
		map[string]any{"type": "reply", "data": map[string]any{"id": "12345"}},
	}
	if got := findReplyIDInMessage(message); got != "12345" {
		t.Fatalf("findReplyIDInMessage() = %q, want %q", got, "12345")
	}
}

func TestCollectImagesFromCurrentMessage(t *testing.T) {
	message := []any{
		map[string]any{"type": "text", "data": map[string]any{"text": "上传cat"}},
		map[string]any{"type": "image", "data": map[string]any{"url": "https://example.com/a.png", "name": "a.png"}},
		map[string]any{"type": "image", "data": map[string]any{"url": "https://example.com/b.png"}},
	}
	images, err := collectImagesFromValue(context.Background(), nil, message, 0)
	if err != nil {
		t.Fatalf("collectImagesFromValue() error = %v", err)
	}
	if len(images) != 2 {
		t.Fatalf("collectImagesFromValue() len = %d, want 2", len(images))
	}
	if images[0].SourceURL != "https://example.com/a.png" {
		t.Fatalf("unexpected first image url: %q", images[0].SourceURL)
	}
	if images[1].Name != "b.png" {
		t.Fatalf("unexpected fallback filename: %q", images[1].Name)
	}
}

func TestBuildDuplicateComparePageURL(t *testing.T) {
	url := buildDuplicateComparePageURL("http://amiabot-pages:8080", duplicateComparePageParams{
		CurrentImageURL:  "http://blob-server/current.png",
		DuplicateImageID: 123,
		CurrentTags:      []string{"cat", "cover"},
		ExistingTags:     []string{"cat"},
	})
	for _, fragment := range []string{"/gallery/duplicate", "duplicate_id=123", "current_tags=cat%2C+cover", "existing_tags=cat"} {
		if !strings.Contains(url, fragment) {
			t.Fatalf("buildDuplicateComparePageURL() missing %q: %s", fragment, url)
		}
	}
}

func TestExtractImagesFromReplyForward(t *testing.T) {
	host := &stubHostCaller{
		responses: map[string]ob11.APIResponse{
			"get_msg": {
				Status:  "ok",
				RetCode: 0,
				Data: mustJSON(t, map[string]any{
					"message": []any{
						map[string]any{"type": "forward", "data": map[string]any{"id": "forward-1"}},
					},
				}),
			},
			"get_forward_msg": {
				Status:  "ok",
				RetCode: 0,
				Data: mustJSON(t, map[string]any{
					"messages": []any{
						map[string]any{
							"type": "node",
							"data": map[string]any{
								"content": []any{
									map[string]any{"type": "image", "data": map[string]any{"url": "https://example.com/fwd.png", "name": "fwd.png"}},
								},
							},
						},
					},
				}),
			},
		},
	}
	evt := map[string]any{
		"message": []any{
			map[string]any{"type": "text", "data": map[string]any{"text": "上传cat"}},
			map[string]any{"type": "reply", "data": map[string]any{"id": "reply-1"}},
		},
	}
	images, source, err := extractImagesFromEvent(context.Background(), host, evt)
	if err != nil {
		t.Fatalf("extractImagesFromEvent() error = %v", err)
	}
	if source != "引用消息" {
		t.Fatalf("extractImagesFromEvent() source = %q, want %q", source, "引用消息")
	}
	if len(images) != 1 || images[0].SourceURL != "https://example.com/fwd.png" {
		t.Fatalf("unexpected images: %#v", images)
	}
}

func TestParseGalleryPagesViewInput(t *testing.T) {
	tests := []struct {
		name  string
		input string
		mode  galleryPagesViewMode
		tags  []string
	}{
		{name: "all tags", input: "所有", mode: galleryPagesViewAllTags},
		{name: "all images by tag", input: "所有cat, cover", mode: galleryPagesViewAllImages, tags: []string{"cat", "cover"}},
		{name: "normal view", input: "cat", mode: galleryPagesViewNone},
		{name: "invalid all view", input: "所有,", mode: galleryPagesViewNone},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			mode, tags := parseGalleryPagesViewInput(tc.input)
			if mode != tc.mode {
				t.Fatalf("parseGalleryPagesViewInput() mode = %v, want %v", mode, tc.mode)
			}
			if len(tags) != len(tc.tags) {
				t.Fatalf("parseGalleryPagesViewInput() tags len = %d, want %d, got=%v", len(tags), len(tc.tags), tags)
			}
			for i := range tc.tags {
				if tags[i] != tc.tags[i] {
					t.Fatalf("parseGalleryPagesViewInput() tags[%d] = %q, want %q", i, tags[i], tc.tags[i])
				}
			}
		})
	}
}

func TestBuildGalleryPagesURL(t *testing.T) {
	allTagsURL := buildGalleryAllTagsPageURL("http://amiabot-pages:8080")
	if !strings.Contains(allTagsURL, "/gallery/tags") {
		t.Fatalf("buildGalleryAllTagsPageURL() unexpected url: %s", allTagsURL)
	}

	allImagesURL := buildGalleryAllImagesPageURL("http://amiabot-pages:8080", []string{"cat", "cover"})
	for _, fragment := range []string{"/gallery/images", "tags=cat%2Ccover"} {
		if !strings.Contains(allImagesURL, fragment) {
			t.Fatalf("buildGalleryAllImagesPageURL() missing %q: %s", fragment, allImagesURL)
		}
	}
}

func TestGalleryClientRandomImage(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/v1/images/random" {
			t.Fatalf("unexpected request path: %s", r.URL.Path)
		}
		if got := r.URL.Query().Get("tags"); got != "cat,cover" {
			t.Fatalf("unexpected tags query: %q", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(mustJSON(t, map[string]any{
			"id":          42,
			"filename":    "picked.webp",
			"fid":         "fid",
			"file_size":   123,
			"width":       100,
			"height":      200,
			"mime_type":   "image/webp",
			"phash":       1,
			"is_animated": false,
			"description": "",
			"created_at":  "2025-01-01T00:00:00Z",
			"tags":        []map[string]any{{"id": 1, "name": "cat", "created_at": "2025-01-01T00:00:00Z"}},
		}))
	}))
	defer server.Close()

	client := newGalleryClient(galleryConfig{GalleryServer: server.URL})
	image, err := client.randomImage(context.Background(), []string{"cat", "cover"})
	if err != nil {
		t.Fatalf("randomImage() error = %v", err)
	}
	if image.ID != 42 {
		t.Fatalf("randomImage() id = %d, want 42", image.ID)
	}
}

type stubHostCaller struct {
	responses map[string]ob11.APIResponse
}

func (s *stubHostCaller) CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error) {
	_ = ctx
	_ = params
	if resp, ok := s.responses[action]; ok {
		return resp, nil
	}
	return ob11.APIResponse{}, nil
}

func (s *stubHostCaller) CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error) {
	_ = ctx
	_ = targetPluginID
	_ = method
	_ = params
	return nil, nil
}

func mustJSON(t *testing.T, value any) json.RawMessage {
	t.Helper()
	data, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("json.Marshal() error = %v", err)
	}
	return data
}
