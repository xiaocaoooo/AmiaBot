package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"path"
	"strconv"
	"strings"
	"time"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

const maxMessageExtractDepth = 8

type extractedImage struct {
	SourceURL string
	Name      string
}

type forwardMessagesResponse struct {
	Messages []any `json:"messages"`
}

func extractImagesFromEvent(ctx context.Context, host util.HostCaller, evt map[string]any) ([]extractedImage, string, error) {
	if images, err := collectImagesFromValue(ctx, host, evt["message"], 0); err != nil {
		return nil, "", err
	} else if len(images) > 0 {
		return dedupeImages(images), "当前消息", nil
	}

	replyID := findReplyIDInMessage(evt["message"])
	if replyID == "" {
		return nil, "", nil
	}

	replyMsg, err := callOneBotJSON[map[string]any](ctx, host, "get_msg", map[string]any{"message_id": replyID})
	if err != nil {
		return nil, "", err
	}
	images, err := collectImagesFromValue(ctx, host, replyMsg["message"], 0)
	if err != nil {
		return nil, "", err
	}
	return dedupeImages(images), "引用消息", nil
}

func findReplyIDInMessage(message any) string {
	segments := asSegmentSlice(message)
	for _, seg := range segments {
		if strings.TrimSpace(toString(seg["type"])) != "reply" {
			continue
		}
		data, _ := seg["data"].(map[string]any)
		if data == nil {
			continue
		}
		if replyID := strings.TrimSpace(toString(data["id"])); replyID != "" {
			return replyID
		}
		if replyID := strings.TrimSpace(toString(data["message_id"])); replyID != "" {
			return replyID
		}
	}
	return ""
}

func collectImagesFromValue(ctx context.Context, host util.HostCaller, value any, depth int) ([]extractedImage, error) {
	if depth > maxMessageExtractDepth {
		return nil, fmt.Errorf("消息嵌套层级过深")
	}

	switch v := value.(type) {
	case nil:
		return nil, nil
	case string:
		return nil, nil
	case []any:
		return collectImagesFromSegments(ctx, host, v, depth)
	case []map[string]any:
		items := make([]any, 0, len(v))
		for _, item := range v {
			items = append(items, item)
		}
		return collectImagesFromSegments(ctx, host, items, depth)
	case map[string]any:
		if message, ok := v["message"]; ok {
			return collectImagesFromValue(ctx, host, message, depth+1)
		}
		if content, ok := v["content"]; ok {
			return collectImagesFromValue(ctx, host, content, depth+1)
		}
		if data, ok := v["data"].(map[string]any); ok {
			if content, ok := data["content"]; ok {
				return collectImagesFromValue(ctx, host, content, depth+1)
			}
			if strings.TrimSpace(toString(v["type"])) == "forward" {
				forwardID := strings.TrimSpace(firstNonEmpty(toString(data["id"]), toString(data["forward_id"]), toString(v["id"])))
				if forwardID == "" {
					return nil, nil
				}
				return collectImagesFromForwardID(ctx, host, forwardID, depth+1)
			}
		}
	}
	return nil, nil
}

func collectImagesFromSegments(ctx context.Context, host util.HostCaller, segments []any, depth int) ([]extractedImage, error) {
	images := make([]extractedImage, 0)
	for _, rawSeg := range segments {
		seg, ok := rawSeg.(map[string]any)
		if !ok {
			continue
		}
		segType := strings.TrimSpace(toString(seg["type"]))
		data, _ := seg["data"].(map[string]any)
		switch segType {
		case "image":
			if image, ok := parseImageSegment(data); ok {
				images = append(images, image)
			}
		case "forward":
			if data == nil {
				continue
			}
			forwardID := strings.TrimSpace(firstNonEmpty(toString(data["id"]), toString(data["forward_id"]), toString(seg["id"])))
			if forwardID == "" {
				continue
			}
			forwardImages, err := collectImagesFromForwardID(ctx, host, forwardID, depth+1)
			if err != nil {
				return nil, err
			}
			images = append(images, forwardImages...)
		case "node":
			if data == nil {
				continue
			}
			content, ok := data["content"]
			if !ok {
				continue
			}
			nodeImages, err := collectImagesFromValue(ctx, host, content, depth+1)
			if err != nil {
				return nil, err
			}
			images = append(images, nodeImages...)
		}
	}
	return images, nil
}

func collectImagesFromForwardID(ctx context.Context, host util.HostCaller, forwardID string, depth int) ([]extractedImage, error) {
	resp, err := callOneBotJSON[forwardMessagesResponse](ctx, host, "get_forward_msg", map[string]any{"message_id": forwardID})
	if err != nil {
		resp, err = callOneBotJSON[forwardMessagesResponse](ctx, host, "get_forward_msg", map[string]any{"id": forwardID})
		if err != nil {
			return nil, err
		}
	}

	images := make([]extractedImage, 0)
	for _, node := range resp.Messages {
		nodeImages, err := collectImagesFromValue(ctx, host, node, depth+1)
		if err != nil {
			return nil, err
		}
		images = append(images, nodeImages...)
	}
	return images, nil
}

func parseImageSegment(data map[string]any) (extractedImage, bool) {
	if data == nil {
		return extractedImage{}, false
	}
	sourceURL := strings.TrimSpace(toString(data["url"]))
	if sourceURL == "" {
		for _, candidate := range []string{toString(data["file"]), toString(data["path"])} {
			candidate = strings.TrimSpace(candidate)
			if !looksDownloadableResource(candidate) {
				continue
			}
			sourceURL = candidate
			break
		}
	}
	if sourceURL == "" {
		return extractedImage{}, false
	}
	name := strings.TrimSpace(firstNonEmpty(toString(data["name"]), filenameFromURL(sourceURL)))
	if name == "" {
		name = "image"
	}
	return extractedImage{SourceURL: sourceURL, Name: name}, true
}

func downloadImageData(ctx context.Context, image extractedImage) (string, []byte, error) {
	sourceURL := strings.TrimSpace(image.SourceURL)
	if sourceURL == "" {
		return "", nil, fmt.Errorf("图片地址为空，无法下载")
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, sourceURL, nil)
	if err != nil {
		return "", nil, err
	}
	req.Header.Set("User-Agent", "nyanyabot-plugin-amiabot-gallery/0.1")
	client := &http.Client{Timeout: 60 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return "", nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 64*1024))
		return "", nil, fmt.Errorf("下载图片失败（HTTP %d）", resp.StatusCode)
	}
	data, err := io.ReadAll(io.LimitReader(resp.Body, 32*1024*1024+1))
	if err != nil {
		return "", nil, err
	}
	if len(data) == 0 {
		return "", nil, fmt.Errorf("下载到的图片内容为空")
	}
	if len(data) > 32*1024*1024 {
		return "", nil, fmt.Errorf("图片大小超过 32 MiB 限制")
	}
	filename := strings.TrimSpace(image.Name)
	if filename == "" {
		filename = filenameFromURL(sourceURL)
	}
	if filename == "" {
		filename = "image"
	}
	return filename, data, nil
}

func filenameFromURL(raw string) string {
	parsed, err := url.Parse(strings.TrimSpace(raw))
	if err != nil {
		return ""
	}
	name := path.Base(parsed.Path)
	if name == "." || name == "/" {
		return ""
	}
	return name
}

func looksDownloadableResource(raw string) bool {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return false
	}
	return strings.HasPrefix(raw, "http://") || strings.HasPrefix(raw, "https://") || strings.HasPrefix(raw, "file://") || strings.HasPrefix(raw, "/")
}

func dedupeImages(images []extractedImage) []extractedImage {
	result := make([]extractedImage, 0, len(images))
	seen := make(map[string]struct{}, len(images))
	for _, image := range images {
		key := strings.TrimSpace(image.SourceURL)
		if key == "" {
			continue
		}
		if _, ok := seen[key]; ok {
			continue
		}
		seen[key] = struct{}{}
		result = append(result, image)
	}
	return result
}

func asSegmentSlice(value any) []map[string]any {
	segments := make([]map[string]any, 0)
	switch v := value.(type) {
	case []any:
		for _, item := range v {
			if seg, ok := item.(map[string]any); ok {
				segments = append(segments, seg)
			}
		}
	case []map[string]any:
		segments = append(segments, v...)
	}
	return segments
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}

func toString(value any) string {
	switch v := value.(type) {
	case nil:
		return ""
	case string:
		return v
	case json.Number:
		return v.String()
	case float64:
		return strconv.FormatInt(int64(v), 10)
	case float32:
		return strconv.FormatInt(int64(v), 10)
	case int:
		return strconv.Itoa(v)
	case int64:
		return strconv.FormatInt(v, 10)
	case int32:
		return strconv.FormatInt(int64(v), 10)
	case int16:
		return strconv.FormatInt(int64(v), 10)
	case int8:
		return strconv.FormatInt(int64(v), 10)
	case uint:
		return strconv.FormatUint(uint64(v), 10)
	case uint64:
		return strconv.FormatUint(v, 10)
	case uint32:
		return strconv.FormatUint(uint64(v), 10)
	case uint16:
		return strconv.FormatUint(uint64(v), 10)
	case uint8:
		return strconv.FormatUint(uint64(v), 10)
	default:
		return fmt.Sprintf("%v", value)
	}
}

func callOneBotJSON[T any](ctx context.Context, host util.HostCaller, action string, params any) (T, error) {
	var zero T
	if host == nil {
		return zero, fmt.Errorf("host 不可用")
	}
	resp, err := host.CallOneBot(ctx, action, params)
	if err != nil {
		return zero, err
	}
	if resp.RetCode != 0 || (resp.Status != "" && !strings.EqualFold(resp.Status, "ok")) {
		return zero, fmt.Errorf("调用 %s 失败：%s", action, firstNonEmpty(resp.Wording, resp.Msg, resp.Status, strconv.Itoa(resp.RetCode)))
	}
	if len(resp.Data) == 0 {
		return zero, fmt.Errorf("%s 返回了空数据", action)
	}
	var out T
	decoder := json.NewDecoder(bytes.NewReader(resp.Data))
	decoder.UseNumber()
	if err := decoder.Decode(&out); err != nil {
		return zero, fmt.Errorf("解析 %s 响应失败：%w", action, err)
	}
	return out, nil
}
