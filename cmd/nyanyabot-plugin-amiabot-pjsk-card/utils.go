package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/url"
	"regexp"
	"strings"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
)

// hostCaller 抽象宿主提供的 OneBot / 跨插件调用能力
type hostCaller interface {
	CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error)
	CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error)
}

// sanitizeError 脱敏错误信息，移除内部地址、路径等敏感信息
func sanitizeError(err error) string {
	if err == nil {
		return "未知错误"
	}
	msg := err.Error()
	// 移除 http(s):// 地址
	re := regexp.MustCompile(`https?://[^\s]+`)
	msg = re.ReplaceAllString(msg, "[服务地址]")
	// 移除 ip:port
	re2 := regexp.MustCompile(`\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}:\d+`)
	msg = re2.ReplaceAllString(msg, "[内部地址]")
	// 移除文件路径
	re3 := regexp.MustCompile(`/[a-zA-Z0-9_./-]+`)
	msg = re3.ReplaceAllString(msg, "[路径]")
	// 限制长度
	if len(msg) > 100 {
		msg = msg[:100] + "..."
	}
	return msg
}

// sendError 发送脱敏后的错误消息给用户
func sendError(host hostCaller, msgType string, groupID any, userID any, prefix string, err error) {
	sanitized := sanitizeError(err)
	hclog.L().Error("[Utils] sendError", "prefix", prefix, "raw_error", err, "sanitized", sanitized)
	sendText(host, msgType, groupID, userID, prefix+": "+sanitized)
}

func buildPagesURL(pagesHost string, path string, params map[string]string) string {
	base := normalizeHTTPBase(pagesHost)
	if base == "" {
		hclog.L().Warn("[Utils] normalizeHTTPBase 返回空")
		return ""
	}
	u, err := url.Parse(base)
	if err != nil {
		hclog.L().Error("[Utils] url.Parse 失败", "base", base, "error", err)
		return ""
	}
	u.Path = strings.TrimRight(u.Path, "/") + path
	q := u.Query()
	for k, v := range params {
		q.Set(k, v)
	}
	u.RawQuery = q.Encode()
	result := u.String()
	hclog.L().Info("[Utils] buildPagesURL", "host", pagesHost, "path", path, "params", params, "result", result)
	return result
}

func buildScreenshotViaPlugin(host hostCaller, pageURL string) (string, error) {
	if host == nil {
		return "", fmt.Errorf("screenshot 调用失败: host 为 nil")
	}
	if strings.TrimSpace(pageURL) == "" {
		return "", fmt.Errorf("screenshot 调用失败: pageURL 为空")
	}
	hclog.L().Info("[Utils] 调用 external.screenshot.build_url", "page_url", pageURL)
	result, err := host.CallDependency(context.Background(), "external.screenshot", "screenshot.build_url", map[string]any{
		"page_url": pageURL,
		"selector": "#screenshot-wrapper",
	})
	if err != nil {
		return "", fmt.Errorf("screenshot 插件调用失败: %w", err)
	}
	hclog.L().Info("[Utils] external.screenshot 返回", "result", string(result))
	var out struct {
		URL string `json:"url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		return "", fmt.Errorf("screenshot 返回解析失败: %w", err)
	}
	hclog.L().Info("[Utils] 截图 URL 构建成功", "url", out.URL)
	return strings.TrimSpace(out.URL), nil
}

func uploadViaBlobPlugin(ctx context.Context, host hostCaller, downloadURL string, blobID string, kind string) string {
	if host == nil {
		hclog.L().Warn("[Utils] uploadViaBlobPlugin: host 为 nil")
		return ""
	}
	if strings.TrimSpace(downloadURL) == "" || strings.TrimSpace(blobID) == "" {
		hclog.L().Warn("[Utils] uploadViaBlobPlugin: downloadURL 或 blobID 为空")
		return ""
	}
	hclog.L().Info("[Utils] 调用 external.blobserver.upload_remote",
		"download_url", downloadURL, "blob_id", blobID, "kind", kind,
	)
	result, err := host.CallDependency(ctx, "external.blobserver", "blob.upload_remote", map[string]any{
		"download_url": downloadURL,
		"blob_id":      blobID,
		"kind":         kind,
	})
	if err != nil {
		hclog.L().Error("[Utils] external.blobserver 调用失败", "error", err)
		return ""
	}
	hclog.L().Info("[Utils] external.blobserver 返回", "result", string(result))
	var out struct {
		BlobURL   string `json:"blob_url"`
		OneBotURL string `json:"onebot_url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		hclog.L().Error("[Utils] 解析 blobserver 返回失败", "error", err)
		return ""
	}
	finalURL := strings.TrimSpace(out.BlobURL)
	if strings.TrimSpace(out.OneBotURL) != "" {
		finalURL = strings.TrimSpace(out.OneBotURL)
	}
	hclog.L().Info("[Utils] 上传成功", "final_url", finalURL)
	return finalURL
}

func sendImage(host hostCaller, msgType string, groupID any, userID any, urlStr string) error {
	if host == nil {
		hclog.L().Warn("[Utils] sendImage: host 为 nil")
		return nil
	}
	hclog.L().Info("[Utils] 发送图片消息", "msg_type", msgType, "group_id", groupID, "user_id", userID, "url", urlStr)
	if msgType == "group" {
		_, err := host.CallOneBot(context.Background(), "send_group_msg", map[string]any{
			"group_id": groupID,
			"message": []map[string]any{
				{"type": "image", "data": map[string]any{"file": urlStr}},
			},
		})
		if err != nil {
			hclog.L().Error("[Utils] send_group_msg 失败", "error", err)
		} else {
			hclog.L().Info("[Utils] send_group_msg 成功")
		}
		return err
	}
	_, err := host.CallOneBot(context.Background(), "send_private_msg", map[string]any{
		"user_id": userID,
		"message": []map[string]any{
			{"type": "image", "data": map[string]any{"file": urlStr}},
		},
	})
	if err != nil {
		hclog.L().Error("[Utils] send_private_msg 失败", "error", err)
	} else {
		hclog.L().Info("[Utils] send_private_msg 成功")
	}
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
