// nyanyabot-plugin-amiabot-bilibili
//
// 这是一个「外置插件」示例：插件会被主程序作为独立进程启动，并通过 HashiCorp go-plugin
// （当前仓库使用 net/rpc 协议）与主程序通讯。
//
// 你编写插件时，主要需要做 4 件事：
//  1. 实现 plugin.Plugin 接口（见 sdk 的 plugin/api.go）
//  2. 在 Descriptor() 里声明：插件元信息 + commands/events +（可选）config
//  3. 在 Handle() 里根据 listenerID 分发到你的业务函数
//  4. 需要发消息/调用 OneBot 动作时，使用 transport.Host().CallOneBot(...)
//
// 插件如何被加载：
//   - 主程序启动时扫描 ./plugins 目录
//   - 文件名需以 "nyanyabot-plugin-" 开头且具备可执行权限
//
// 提示：本文件目前保留了 Echo 的最小可运行逻辑，便于你先跑通加载链路。
// 你可以把命令、事件监听、配置结构替换成自己的需求。
package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
)

// AmiabotBilibili 是插件的实现类型。
//
// 注意：主程序可能并发调用 Handle()（事件分发可能是并发的），因此：
//   - 配置（cfg）等共享状态要用锁/原子变量保护
//   - Handle() 内尽量不要做长时间阻塞
type AmiabotBilibili struct {
	mu  sync.RWMutex
	cfg struct {
		// amiabotPages / bilibiliDownloaderServer:
		//
		// 这两个配置用于拼接你需要发送的 URL。
		// - 留空表示不启用对应能力（插件不会发送截图/视频链接，或会提示配置缺失）。
		// - 可以填 "127.0.0.1:8080" 这种 host:port，也可以填 "http(s)://domain"。
		AmiabotPages             string `json:"amiabot_pages"`
		BilibiliDownloaderServer string `json:"bilibili_downloader_server"`
	}
}

// Descriptor 返回插件自描述信息。
//
// 这是插件被宿主加载后，宿主首先会调用的方法之一。
// 宿主会用这里返回的 commands/events 来决定如何把 OneBot 事件分发给你。
func (e *AmiabotBilibili) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx

	// schema/default 是可选项：用于让 WebUI 渲染配置表单，并在未配置时提供默认值。
	// 这里声明 2 个域名/地址字段，默认都为空。
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 域名/地址（用于拼接 /bilibili/video 页面 URL）；为空则无法生成截图 URL"},
			"bilibili_downloader_server":{"type":"string","description":"Bilibili Downloader 服务域名/地址；为空则不发送视频下载链接"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"amiabot_pages":"","bilibili_downloader_server":""}`)
	return papi.Descriptor{
		// Name/PluginID/Version/Author/Description 会展示在 WebUI 插件页面。
		// PluginID 必须全局唯一，否则宿主注册时会报错：plugin already registered。
		Name:        "Amiabot Bilibili",
		PluginID:    "external.amiabot-bilibili",
		Version:     "0.1.0",
		Author:      "nyanyabot",
		Description: "Amiabot bilibili plugin (skeleton)",
		Dependencies: []string{
			"external.screenshot",
			"external.blobserver",
		},
		Exports: []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				// 命令监听器：当 post_type == "message" 且 pattern 命中时触发。
				//
				// 你要求的原始正则（JS 风格，含命名捕获与 /i）：
				//   /\b(?:av(?<aid>\d+))|(?<bvid>bv1[0-9a-zA-Z]+)|(?:(?:https?:\/\/)?b23\.tv\/(?<short>[a-z0-9]+))\b/i
				//
				// Go regexp 不支持 (?<name>)，也没有 /.../i 语法，因此改为等价的 Go 版本：
				//   (?i)\b(?:av(\d+)|(bv1[0-9a-zA-Z]+)|(?:(?:https?://)?b23\.tv/([a-z0-9]+)))\b
				// 捕获组含义：
				//   groups[0] = aid（数字）
				//   groups[1] = bvid（BV1...）
				//   groups[2] = short（b23.tv 短链 code）
				Name:        "bilibili",
				ID:          "cmd.bilibili",
				Description: "识别 av 号 / BV 号 / b23.tv 短链，并发送截图与下载链接",
				Pattern:     `(?i)\b(?:av(\d+)|(bv1[0-9a-zA-Z]+)|(?:(?:https?://)?b23\.tv/([a-z0-9]+)))\b`,
				MatchRaw:    true,
				Handler:     "HandleBilibili",
			},
		},
	}, nil
}

// Configure 接收宿主下发的配置。
//
// 调用时机：
//   - 宿主注册插件后会立刻调用一次（有配置则下发配置，没有则下发 {}）
//   - WebUI 保存插件配置后会再次调用（热更新）
func (e *AmiabotBilibili) Configure(ctx context.Context, config json.RawMessage) error {
	_ = ctx
	// Merge with defaults.
	// 推荐做法：先准备带默认值的结构体，然后再 Unmarshal 覆盖。
	cfg := struct {
		AmiabotPages             string `json:"amiabot_pages"`
		BilibiliDownloaderServer string `json:"bilibili_downloader_server"`
	}{}
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}
	e.mu.Lock()
	e.cfg.AmiabotPages = strings.TrimSpace(cfg.AmiabotPages)
	e.cfg.BilibiliDownloaderServer = strings.TrimSpace(cfg.BilibiliDownloaderServer)
	e.mu.Unlock()
	return nil
}

func (e *AmiabotBilibili) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = ctx
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

// Handle 是宿主分发入口。
//
// 参数说明：
//   - listenerID：命中的监听器 ID（即 CommandListener.ID / EventListener.ID）
//   - eventRaw：原始 OneBot/NapCat 事件 JSON（json.RawMessage）
//   - match：仅命令命中时非空，包含正则捕获组
func (e *AmiabotBilibili) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx

	// 典型写法：switch listenerID 分发到不同函数。
	// 你的插件有多个 commands/events 时，这里会变成一个路由表。
	switch listenerID {
	case "cmd.bilibili":
		return e.handleBilibili(ctx, eventRaw, match)
	default:
		return papi.HandleResult{}, nil
	}
}

// Shutdown 在插件被宿主关闭时调用。
//
// 当前示例没有资源需要释放，因此直接返回 nil。
// 如果你有 goroutine、连接池、文件句柄等，请在这里优雅关闭。
func (e *AmiabotBilibili) Shutdown(ctx context.Context) error {
	_ = ctx
	return nil
}

// hostCaller 抽象宿主提供的 OneBot 调用能力。
// transport.Host() 返回的对象满足该接口。
type hostCaller interface {
	CallOneBot(ctx context.Context, action string, params any) (ob11.APIResponse, error)
	CallDependency(ctx context.Context, targetPluginID string, method string, params any) (json.RawMessage, error)
}

// handleBilibili：
//   - 识别消息中的 aid / bvid / b23.tv short
//   - short 需要先拼接 https://b23.tv/{short} 并跟随重定向，得到最终 URL，再解析 aid 或 bvid
//   - 发送：
//     图片：通过 external.screenshot 构造截图 URL（输入为 amiabot pages URL）
//     视频： http://{bilibili-downloader-server}/bilibili/download/{id}
//     其中 aid 与 bvid 只需提供一个；id 为 aid 或 bvid。
func (e *AmiabotBilibili) handleBilibili(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	host := transport.Host()
	if host == nil {
		return papi.HandleResult{}, nil
	}

	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		return papi.HandleResult{}, nil
	}

	msgType, _ := evt["message_type"].(string)
	groupID := evt["group_id"]
	userID := evt["user_id"]
	rawMessage, _ := evt["raw_message"].(string)

	// 解析匹配结果：aid / bvid / short（三选一，short 需要继续解析）。
	aid, bvid, short := "", "", ""
	if match != nil && len(match.Groups) >= 3 {
		// groups[0]=aid, groups[1]=bvid, groups[2]=short
		aid = strings.TrimSpace(match.Groups[0])
		bvid = strings.TrimSpace(match.Groups[1])
		short = strings.TrimSpace(match.Groups[2])
	}

	// 兜底：如果宿主没有传 match，就自己跑一次正则（与 Descriptor.Pattern 保持一致）。
	if aid == "" && bvid == "" && short == "" {
		re := regexp.MustCompile(`(?i)\b(?:av(\d+)|(bv1[0-9a-zA-Z]+)|(?:(?:https?://)?b23\.tv/([a-z0-9]+)))\b`)
		m := re.FindStringSubmatch(rawMessage)
		if len(m) >= 4 {
			aid = strings.TrimSpace(m[1])
			bvid = strings.TrimSpace(m[2])
			short = strings.TrimSpace(m[3])
		}
	}

	if short != "" && aid == "" && bvid == "" {
		a2, b2, err := resolveB23(ctx, short)
		if err != nil {
			return papi.HandleResult{}, nil
		}
		aid, bvid = a2, b2
	}

	if aid == "" && bvid == "" {
		return papi.HandleResult{}, nil
	}
	if bvid != "" {
		bvid = strings.TrimSpace(bvid)
	}

	// 读取配置（可能会被 Configure 热更新）。
	e.mu.RLock()
	pagesHost := e.cfg.AmiabotPages
	downloaderServer := e.cfg.BilibiliDownloaderServer
	e.mu.RUnlock()

	// 生成截图 URL（需要 pagesHost + external.screenshot 插件）。
	screenshotURL := ""
	if pagesHost != "" {
		pagesURL := buildAmiabotBilibiliPagesURL(pagesHost, aid, bvid)
		screenshotURL = buildScreenshotViaPlugin(host, pagesURL)
	}

	// 生成下载 URL（需要 downloaderServer）。
	// id 为 aid 或 bvid。
	id := ""
	if aid != "" {
		id = aid
	} else {
		id = bvid
	}
	videoURL := ""
	if downloaderServer != "" {
		downloadURL := buildDownloaderURL(downloaderServer, id)
		videoURL = downloadURL
	}

	if screenshotURL == "" && videoURL == "" {
		return papi.HandleResult{}, nil
	}

	// 图片与视频分开发送，若 external.blobserver 可用则优先转为 Blob URL，不可用则回退原始 URL。
	if screenshotURL != "" {
		imageID := fmt.Sprintf("%s-image-%d", id, time.Now().Unix())
		if uploadedURL := uploadViaBlobPlugin(ctx, host, screenshotURL, imageID, "image"); uploadedURL != "" {
			screenshotURL = uploadedURL
		}
		_ = sendImage(host, msgType, groupID, userID, screenshotURL)
	}
	if videoURL != "" {
		videoID := id + "-video"
		if uploadedURL := uploadViaBlobPlugin(ctx, host, videoURL, videoID, "video"); uploadedURL != "" {
			videoURL = uploadedURL
		}
		_ = sendVideo(host, msgType, groupID, userID, videoURL)
	}

	return papi.HandleResult{}, nil
}

func sendImage(host hostCaller, msgType string, groupID any, userID any, url string) error {
	if host == nil {
		return nil
	}
	if msgType == "group" {
		_, err := host.CallOneBot(context.Background(), "send_group_msg", map[string]any{
			"group_id": groupID,
			"message": []map[string]any{
				{"type": "image", "data": map[string]any{"file": url}},
			},
		})
		return err
	}
	_, err := host.CallOneBot(context.Background(), "send_private_msg", map[string]any{
		"user_id": userID,
		"message": []map[string]any{
			{"type": "image", "data": map[string]any{"file": url}},
		},
	})
	return err
}

func sendVideo(host hostCaller, msgType string, groupID any, userID any, url string) error {
	if host == nil {
		return nil
	}
	if msgType == "group" {
		_, err := host.CallOneBot(context.Background(), "send_group_msg", map[string]any{
			"group_id": groupID,
			"message": []map[string]any{
				{"type": "video", "data": map[string]any{"file": url}},
			},
		})
		return err
	}
	_, err := host.CallOneBot(context.Background(), "send_private_msg", map[string]any{
		"user_id": userID,
		"message": []map[string]any{
			{"type": "video", "data": map[string]any{"file": url}},
		},
	})
	return err
}

// resolveB23 将 b23.tv 的 short code 解析为 aid 或 bvid。
//
// 逻辑：GET https://b23.tv/{short} 并跟随重定向；从最终 URL 中提取 av123 或 BV1...。
func resolveB23(ctx context.Context, short string) (aid string, bvid string, err error) {
	short = strings.TrimSpace(short)
	if short == "" {
		return "", "", errors.New("short is empty")
	}

	u := "https://b23.tv/" + short
	client := &http.Client{Timeout: 10 * time.Second}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, u, nil)
	if err != nil {
		return "", "", err
	}
	req.Header.Set("User-Agent", "nyanyabot-plugin-amiabot-bilibili/0.1")

	resp, err := client.Do(req)
	if err != nil {
		return "", "", err
	}
	defer resp.Body.Close()
	_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 64*1024))

	finalURL := ""
	if resp.Request != nil && resp.Request.URL != nil {
		finalURL = resp.Request.URL.String()
	}
	if finalURL == "" {
		return "", "", errors.New("empty final url")
	}

	aid = extractAID(finalURL)
	bvid = extractBVID(finalURL)
	if aid == "" && bvid == "" {
		return "", "", fmt.Errorf("cannot extract aid/bvid from redirect url: %s", finalURL)
	}
	return aid, bvid, nil
}

func extractAID(s string) string {
	re := regexp.MustCompile(`(?i)\bav(\d+)\b`)
	m := re.FindStringSubmatch(s)
	if len(m) >= 2 {
		return m[1]
	}
	return ""
}

func extractBVID(s string) string {
	re := regexp.MustCompile(`(?i)\b(bv1[0-9a-zA-Z]+)\b`)
	m := re.FindStringSubmatch(s)
	if len(m) >= 2 {
		return strings.ToUpper(m[1])
	}
	return ""
}

func buildAmiabotBilibiliPagesURL(amiabotPages string, aid string, bvid string) string {
	base := normalizeHTTPBase(amiabotPages)
	u, err := url.Parse(base)
	if err != nil {
		return ""
	}
	u.Path = strings.TrimRight(u.Path, "/") + "/bilibili/video"
	q := u.Query()
	if aid != "" {
		q.Set("aid", aid)
	}
	if bvid != "" {
		q.Set("bvid", bvid)
	}
	u.RawQuery = q.Encode()
	return u.String()
}

func buildDownloaderURL(downloaderServer string, id string) string {
	base := normalizeHTTPBase(downloaderServer)
	u, err := url.Parse(base)
	if err != nil {
		return ""
	}
	u.Path = strings.TrimRight(u.Path, "/") + "/bilibili/download/" + url.PathEscape(id)
	u.RawQuery = ""
	return u.String()
}

func buildScreenshotViaPlugin(host hostCaller, pageURL string) string {
	if host == nil || strings.TrimSpace(pageURL) == "" {
		return ""
	}
	result, err := host.CallDependency(context.Background(), "external.screenshot", "screenshot.build_url", map[string]any{
		"page_url": pageURL,
		"selector": "#screenshot-wrapper",
	})
	if err != nil {
		return ""
	}
	var out struct {
		URL string `json:"url"`
	}
	if err := json.Unmarshal(result, &out); err != nil {
		return ""
	}
	return strings.TrimSpace(out.URL)
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

func main() {
	// 这里是 go-plugin 的标准启动方式：把你的实现类型挂到 transport.Map{PluginImpl: ...}。
	// 宿主端会使用相同的 Handshake() 与 PluginName 来发现/连接。
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &AmiabotBilibili{}},
		},
	})
}
