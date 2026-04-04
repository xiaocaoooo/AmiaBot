package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

// 接口兼容性检查：transport.HostRPCClient 必须实现 util.HostCaller
var _ util.HostCaller = (*transport.HostRPCClient)(nil)

// buildPagesAssetURL 构建页面资源 URL（用于静态资源路径合并）
// 此函数是插件特有逻辑（处理相对路径 URL），留在本地
func buildPagesAssetURL(pagesHost string, assetPath string) string {
	base := util.NormalizeHTTPBase(pagesHost)
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

func buildPixivPageURL(pagesHost string, pid string) string {
	pid = strings.TrimSpace(pid)
	if pid == "" {
		return ""
	}
	return util.BuildPagesURL(pagesHost, "/pixiv/illust/info", map[string]string{"pid": pid})
}

func resolvePixivMediaBase(pagesHost string, downloadBase string) string {
	downloadBase = strings.TrimSpace(downloadBase)
	if downloadBase != "" {
		return downloadBase
	}
	return strings.TrimSpace(pagesHost)
}

func fetchPixivMediaManifest(ctx context.Context, pagesHost string, pid string) (*pixivMediaManifest, error) {
	manifestURL := util.BuildPagesURL(pagesHost, "/pixiv/illust/media", map[string]string{"pid": strings.TrimSpace(pid)})
	if manifestURL == "" {
		return nil, fmt.Errorf("原图清单 URL 构造失败")
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, manifestURL, nil)
	if err != nil {
		return nil, fmt.Errorf("创建原图清单请求失败: %w", err)
	}
	resp, err := pixivHTTPClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("请求原图清单失败: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("读取原图清单失败: %w", err)
	}
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("原图清单请求失败: HTTP %d %s", resp.StatusCode, extractErrorMessage(body))
	}

	var manifest pixivMediaManifest
	if err := json.Unmarshal(body, &manifest); err != nil {
		return nil, fmt.Errorf("解析原图清单失败: %w", err)
	}
	return &manifest, nil
}

func resolvePixivMediaURLs(ctx context.Context, host util.HostCaller, pagesHost string, pid string, items []pixivMediaItem) []string {
	timestamp := time.Now().Unix()
	urls := make([]string, 0, len(items))
	for _, item := range items {
		mediaURL := buildPagesAssetURL(pagesHost, item.Path)
		if mediaURL == "" {
			continue
		}
		blobID := fmt.Sprintf("pixiv-media-%s-%d-%d", pid, item.Index, timestamp)
		if uploaded := util.UploadViaBlobPlugin(ctx, host, mediaURL, blobID, "image"); uploaded != "" {
			mediaURL = uploaded
		}
		urls = append(urls, mediaURL)
	}
	return urls
}
