package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/url"
	"regexp"
	"strings"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
)

type hostCaller interface {
	CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error)
	CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error)
}

func sanitizeError(err error) string {
	if err == nil {
		return "未知错误"
	}
	msg := err.Error()
	re := regexp.MustCompile(`https?://[^\s]+`)
	msg = re.ReplaceAllString(msg, "[服务地址]")
	re2 := regexp.MustCompile(`\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}:\d+`)
	msg = re2.ReplaceAllString(msg, "[内部地址]")
	re3 := regexp.MustCompile(`/[a-zA-Z0-9_./-]+`)
	msg = re3.ReplaceAllString(msg, "[路径]")
	if len(msg) > 100 {
		msg = msg[:100] + "..."
	}
	return msg
}

func sendError(host hostCaller, msgType string, groupID any, userID any, prefix string, err error) {
	sendText(host, msgType, groupID, userID, prefix+": "+sanitizeError(err))
}

func buildPagesURL(pagesHost string, path string, params map[string]string) string {
	base := normalizeHTTPBase(pagesHost)
	if base == "" {
		return ""
	}
	u, err := url.Parse(base)
	if err != nil {
		return ""
	}
	u.Path = strings.TrimRight(u.Path, "/") + path
	q := u.Query()
	for k, v := range params {
		q.Set(k, v)
	}
	u.RawQuery = q.Encode()
	return u.String()
}

func buildPagesAssetURL(pagesHost string, assetPath string) string {
	base := normalizeHTTPBase(pagesHost)
	assetPath = strings.TrimSpace(assetPath)
	if base == "" || assetPath == "" {
		return ""
	}

	baseURL, err := url.Parse(base)
	if err != nil {
		return ""
	}
	assetURL, err := url.Parse(assetPath)
	if err != nil {
		return ""
	}
	if assetURL.IsAbs() {
		return assetURL.String()
	}

	if assetURL.Path != "" {
		baseURL.Path = strings.TrimRight(baseURL.Path, "/") + "/" + strings.TrimLeft(assetURL.Path, "/")
	}
	baseURL.RawQuery = assetURL.RawQuery
	baseURL.Fragment = assetURL.Fragment
	return baseURL.String()
}

func buildScreenshotViaPlugin(host hostCaller, pageURL string) (string, error) {
	if host == nil {
		return "", fmt.Errorf("screenshot 调用失败: host 为 nil")
	}
	if strings.TrimSpace(pageURL) == "" {
		return "", fmt.Errorf("screenshot 调用失败: pageURL 为空")
	}
	result, err := host.CallDependency(context.Background(), "external.screenshot", "screenshot.build_url", map[string]any{
		"page_url": pageURL,
		"selector": "#screenshot-wrapper",
	})
	if err != nil {
		return "", fmt.Errorf("screenshot 插件调用失败: %w", err)
	}
	var out struct {
		URL string `json:"url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		return "", fmt.Errorf("screenshot 返回解析失败: %w", err)
	}
	return strings.TrimSpace(out.URL), nil
}

func uploadViaBlobPlugin(ctx context.Context, host hostCaller, downloadURL string, blobID string, kind string) string {
	if host == nil || strings.TrimSpace(downloadURL) == "" || strings.TrimSpace(blobID) == "" {
		return ""
	}
	result, err := host.CallDependency(ctx, "external.blobserver", "blob.upload_remote", map[string]any{
		"download_url": downloadURL,
		"blob_id":      blobID,
		"kind":         kind,
	})
	if err != nil {
		return ""
	}
	var out struct {
		BlobURL   string `json:"blob_url"`
		OneBotURL string `json:"onebot_url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		return ""
	}
	if strings.TrimSpace(out.OneBotURL) != "" {
		return strings.TrimSpace(out.OneBotURL)
	}
	return strings.TrimSpace(out.BlobURL)
}

func sendImage(host hostCaller, msgType string, groupID any, userID any, urlStr string) error {
	if host == nil {
		return nil
	}
	if msgType == "group" {
		_, err := host.CallOneBot(context.Background(), "send_group_msg", map[string]any{
			"group_id": groupID,
			"message": []map[string]any{
				{"type": "image", "data": map[string]any{"file": urlStr}},
			},
		})
		return err
	}
	_, err := host.CallOneBot(context.Background(), "send_private_msg", map[string]any{
		"user_id": userID,
		"message": []map[string]any{
			{"type": "image", "data": map[string]any{"file": urlStr}},
		},
	})
	return err
}

func sendForward(host hostCaller, msgType string, groupID any, userID any, nodes []map[string]any) error {
	if host == nil || len(nodes) == 0 {
		return nil
	}
	if msgType == "group" {
		_, err := host.CallOneBot(context.Background(), "send_group_forward_msg", map[string]any{
			"group_id": groupID,
			"messages": nodes,
		})
		return err
	}
	_, err := host.CallOneBot(context.Background(), "send_private_forward_msg", map[string]any{
		"user_id":  userID,
		"messages": nodes,
	})
	return err
}

func sendText(host hostCaller, msgType string, groupID any, userID any, text string) {
	if host == nil || text == "" {
		return
	}
	if msgType == "group" {
		_, _ = host.CallOneBot(context.Background(), "send_group_msg", map[string]any{
			"group_id": groupID,
			"message":  text,
		})
	} else {
		_, _ = host.CallOneBot(context.Background(), "send_private_msg", map[string]any{
			"user_id": userID,
			"message": text,
		})
	}
}

func normalizeHTTPBase(hostOrURL string) string {
	hostOrURL = strings.TrimSpace(hostOrURL)
	if hostOrURL == "" {
		return ""
	}
	if strings.HasPrefix(hostOrURL, "http://") || strings.HasPrefix(hostOrURL, "https://") {
		return strings.TrimRight(hostOrURL, "/")
	}
	return "http://" + strings.TrimRight(hostOrURL, "/")
}

var _ hostCaller = (*transport.HostRPCClient)(nil)
