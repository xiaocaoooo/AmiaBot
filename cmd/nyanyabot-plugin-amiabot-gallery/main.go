package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

const (
	defaultGalleryServer = "http://127.0.0.1:25006"
)

const (
	createTagPattern = `(?i)^新建tag(?P<tag>.+)$`
	uploadPattern    = `(?i)^上传(?P<tag>.+)$`
	viewPattern      = `(?i)^看(?P<input>.+)$`
)

type galleryConfig struct {
	GalleryServer     string `json:"gallery_server"`
	GalleryReadToken  string `json:"gallery_read_token"`
	GalleryWriteToken string `json:"gallery_write_token"`
	AmiabotPages      string `json:"amiabot_pages"`
}

type GalleryPlugin struct {
	mu  sync.RWMutex
	cfg galleryConfig
}

type messageContext struct {
	MsgType string
	GroupID any
	UserID  any
	SelfID  any
	Content string
	Payload map[string]any
}

type uploadOutcome struct {
	Index            int
	Uploaded         *galleryImageWithTags
	DuplicateImageID int64
	Failure          string
}

func defaultGalleryConfig() galleryConfig {
	return galleryConfig{
		GalleryServer:     util.NormalizeHTTPBase(defaultGalleryServer),
		GalleryReadToken:  "",
		GalleryWriteToken: "",
		AmiabotPages:      "",
	}
}

func (g *GalleryPlugin) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"gallery_server":{"type":"string","description":"Gallery Server 的 API / Render 地址，默认 http://127.0.0.1:25006"},
			"gallery_read_token":{"type":"string","description":"Gallery Server 读令牌"},
			"gallery_write_token":{"type":"string","description":"Gallery Server 写令牌"},
			"amiabot_pages":{"type":"string","description":"Amiabot Pages 地址，用于生成重复图片对比卡"}
		},
		"additionalProperties":true
	}`)
	def := json.RawMessage(`{"gallery_server":"http://127.0.0.1:25006","gallery_read_token":"","gallery_write_token":"","amiabot_pages":""}`)

	return papi.Descriptor{
		Name:         "Amiabot Gallery",
		PluginID:     "external.amiabot-gallery",
		Version:      "0.1.0",
		Author:       "nyanyabot",
		Description:  "画廊插件，支持创建标签、上传图片，以及按图片 ID 或标签查看图片",
		Dependencies: []string{"external.screenshot", "external.blobserver"},
		Exports:      []papi.ExportSpec{},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot Gallery 插件配置",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{
			{
				Name:        "gallery-create-tag",
				ID:          "cmd.gallery-create-tag",
				Description: "新建画廊标签",
				Pattern:     createTagPattern,
				MatchRaw:    false,
				Handler:     "HandleCreateTag",
			},
			{
				Name:        "gallery-upload",
				ID:          "cmd.gallery-upload",
				Description: "将当前消息或引用消息中的图片上传到画廊",
				Pattern:     uploadPattern,
				MatchRaw:    false,
				Handler:     "HandleUpload",
			},
			{
				Name:        "gallery-view",
				ID:          "cmd.gallery-view",
				Description: "按图片 ID 或标签查看画廊中的图片",
				Pattern:     viewPattern,
				MatchRaw:    false,
				Handler:     "HandleView",
			},
		},
	}, nil
}

func (g *GalleryPlugin) Configure(ctx context.Context, config json.RawMessage) error {
	cfg := defaultGalleryConfig()
	if len(config) > 0 {
		_ = json.Unmarshal(config, &cfg)
	}
	cfg.GalleryServer = util.NormalizeHTTPBase(strings.TrimSpace(cfg.GalleryServer))
	if cfg.GalleryServer == "" {
		cfg.GalleryServer = util.NormalizeHTTPBase(defaultGalleryServer)
	}
	cfg.GalleryReadToken = strings.TrimSpace(cfg.GalleryReadToken)
	cfg.GalleryWriteToken = strings.TrimSpace(cfg.GalleryWriteToken)
	cfg.AmiabotPages = util.NormalizeHTTPBase(strings.TrimSpace(cfg.AmiabotPages))

	g.mu.Lock()
	g.cfg = cfg
	g.mu.Unlock()
	return nil
}

func (g *GalleryPlugin) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	_ = method
	_ = paramsJSON
	_ = callerPluginID
	return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method is not exported")
}

func (g *GalleryPlugin) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	switch listenerID {
	case "cmd.gallery-create-tag":
		return g.handleCreateTag(ctx, eventRaw, match)
	case "cmd.gallery-upload":
		return g.handleUpload(ctx, eventRaw, match)
	case "cmd.gallery-view":
		return g.handleView(ctx, eventRaw, match)
	default:
		return papi.HandleResult{}, nil
	}
}

func (g *GalleryPlugin) Shutdown(ctx context.Context) error {
	return nil
}

func (g *GalleryPlugin) handleCreateTag(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	host := transport.Host()
	if host == nil {
		return papi.HandleResult{}, nil
	}

	evt, msgCtx, ok := parseMessageEvent(eventRaw)
	if !ok {
		log.Error("[Gallery] 解析事件失败")
		return papi.HandleResult{}, nil
	}
	_ = evt

	tagName := firstGroup(match)
	if strings.TrimSpace(tagName) == "" {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "标签名不能为空")
		return papi.HandleResult{}, nil
	}

	client := g.newClient()
	tag, err := client.createTag(ctx, strings.TrimSpace(tagName))
	if err == nil {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, fmt.Sprintf("标签创建成功：#%s（ID：%d）", tag.Name, tag.ID))
		return papi.HandleResult{}, nil
	}

	if apiErr, ok := err.(*galleryAPIError); ok && apiErr.StatusCode == 409 {
		existing, lookupErr := client.findExactTag(ctx, tagName)
		if lookupErr == nil && existing != nil {
			util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, fmt.Sprintf("标签已存在：#%s（ID：%d）", existing.Name, existing.ID))
			return papi.HandleResult{}, nil
		}
	}

	util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "创建标签失败", err)
	return papi.HandleResult{}, nil
}

func (g *GalleryPlugin) handleUpload(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	log := hclog.L()
	host := transport.Host()
	if host == nil {
		return papi.HandleResult{}, nil
	}

	evt, msgCtx, ok := parseMessageEvent(eventRaw)
	if !ok {
		log.Error("[Gallery] 解析事件失败")
		return papi.HandleResult{}, nil
	}

	tags := parseTags(firstGroup(match))
	if len(tags) == 0 {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "请至少提供一个标签")
		return papi.HandleResult{}, nil
	}

	client := g.newClient()
	missingTags, err := client.findMissingTags(ctx, tags)
	if err != nil {
		util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "标签校验失败", err)
		return papi.HandleResult{}, nil
	}
	if len(missingTags) > 0 {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "以下标签尚未创建，请先创建后再上传："+strings.Join(missingTags, "、"))
		return papi.HandleResult{}, nil
	}

	images, sourceLabel, err := extractImagesFromEvent(ctx, host, evt)
	if err != nil {
		util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "提取图片失败", err)
		return papi.HandleResult{}, nil
	}
	if len(images) == 0 {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "当前消息和引用消息中都没有找到图片")
		return papi.HandleResult{}, nil
	}

	outcomes := make([]uploadOutcome, 0, len(images))
	for index, image := range images {
		filename, data, err := downloadImageData(ctx, image)
		if err != nil {
			outcomes = append(outcomes, uploadOutcome{Index: index + 1, Failure: sanitizeBriefError(err)})
			continue
		}

		uploaded, err := client.uploadImage(ctx, filename, data, tags, false)
		if err == nil {
			outcomes = append(outcomes, uploadOutcome{Index: index + 1, Uploaded: uploaded})
			continue
		}

		apiErr, ok := err.(*galleryAPIError)
		if !ok || apiErr.StatusCode != 409 || apiErr.DuplicateImageID <= 0 {
			outcomes = append(outcomes, uploadOutcome{Index: index + 1, Failure: sanitizeBriefError(err)})
			continue
		}

		outcomes = append(outcomes, uploadOutcome{Index: index + 1, DuplicateImageID: apiErr.DuplicateImageID})
		if compareErr := g.sendDuplicateCompareCard(ctx, host, msgCtx, client, image, tags, apiErr.DuplicateImageID, index+1); compareErr != nil {
			log.Warn("[Gallery] 发送重复对比卡失败", "duplicate_id", apiErr.DuplicateImageID, "error", compareErr)
			util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, fmt.Sprintf("第 %d 张图片已存在于图库中，对应图片 ID：#%d", index+1, apiErr.DuplicateImageID))
		}
	}

	util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, buildUploadSummary(sourceLabel, tags, outcomes))
	return papi.HandleResult{}, nil
}

func (g *GalleryPlugin) handleView(ctx context.Context, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	host := transport.Host()
	if host == nil {
		return papi.HandleResult{}, nil
	}

	_, msgCtx, ok := parseMessageEvent(eventRaw)
	if !ok {
		return papi.HandleResult{}, nil
	}

	input := strings.TrimSpace(firstGroup(match))
	if input == "" {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "请输入图片 ID 或至少一个标签")
		return papi.HandleResult{}, nil
	}

	if mode, tags := parseGalleryPagesViewInput(input); mode != galleryPagesViewNone {
		var err error
		switch mode {
		case galleryPagesViewAllTags:
			err = g.sendGalleryAllTagsCard(ctx, host, msgCtx)
		case galleryPagesViewAllImages:
			err = g.sendGalleryAllImagesCard(ctx, host, msgCtx, tags)
		}
		if err != nil {
			util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "生成画廊页面失败", err)
		}
		return papi.HandleResult{}, nil
	}

	client := g.newClient()
	if looksLikeImageID(input) {
		imageID, _ := strconv.ParseInt(input, 10, 64)
		image, err := client.getImage(ctx, imageID)
		if err == nil {
			if sendErr := g.sendGalleryImage(ctx, host, msgCtx, client, image); sendErr != nil {
				util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "发送图片失败", sendErr)
				return papi.HandleResult{}, nil
			}
			util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, buildImageMetaText(image))
			return papi.HandleResult{}, nil
		}
		if apiErr, ok := err.(*galleryAPIError); ok && apiErr.StatusCode != 404 {
			util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "查询图片失败", err)
			return papi.HandleResult{}, nil
		}
	}

	tags := parseTags(input)
	if len(tags) == 0 {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "请输入图片 ID 或至少一个标签")
		return papi.HandleResult{}, nil
	}

	missingTags, err := client.findMissingTags(ctx, tags)
	if err != nil {
		util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "查询标签失败", err)
		return papi.HandleResult{}, nil
	}
	if len(missingTags) > 0 {
		util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "没有找到匹配的图片或标签")
		return papi.HandleResult{}, nil
	}

	picked, err := client.randomImage(ctx, tags)
	if err != nil {
		if apiErr, ok := err.(*galleryAPIError); ok && apiErr.StatusCode == 404 {
			util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "没有找到匹配的图片或标签")
			return papi.HandleResult{}, nil
		}
		util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "查询图片失败", err)
		return papi.HandleResult{}, nil
	}
	if sendErr := g.sendGalleryImage(ctx, host, msgCtx, client, picked); sendErr != nil {
		util.SendError(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, "发送图片失败", sendErr)
		return papi.HandleResult{}, nil
	}
	util.SendText(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, buildImageMetaText(picked))
	return papi.HandleResult{}, nil
}

func (g *GalleryPlugin) sendDuplicateCompareCard(ctx context.Context, host util.HostCaller, msgCtx messageContext, client *galleryClient, current extractedImage, currentTags []string, duplicateImageID int64, index int) error {
	cfg := g.snapshotConfig()
	if cfg.AmiabotPages == "" {
		return fmt.Errorf("未配置 amiabot_pages")
	}

	existing, err := client.getImage(ctx, duplicateImageID)
	if err != nil {
		return err
	}

	currentImageURL := strings.TrimSpace(current.SourceURL)
	if currentImageURL == "" {
		return fmt.Errorf("当前图片地址为空，无法生成对比卡")
	}

	pageURL := buildDuplicateComparePageURL(cfg.AmiabotPages, duplicateComparePageParams{
		CurrentImageURL:  currentImageURL,
		DuplicateImageID: duplicateImageID,
		CurrentTags:      currentTags,
		ExistingTags:     tagNames(existing.Tags),
	})
	if pageURL == "" {
		return fmt.Errorf("重复图片对比页地址构造失败")
	}

	screenshotURL, err := util.BuildScreenshotViaPlugin(ctx, host, pageURL)
	if err != nil {
		return err
	}
	blobID := fmt.Sprintf("gallery-duplicate-card-%d-%d", duplicateImageID, time.Now().Unix())
	onebotURL, err := uploadOneBotImageViaBlob(ctx, host, screenshotURL, blobID)
	if err != nil {
		return err
	}
	if err := util.SendImage(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, onebotURL); err != nil {
		return err
	}
	return nil
}

func (g *GalleryPlugin) sendGalleryImage(ctx context.Context, host util.HostCaller, msgCtx messageContext, client *galleryClient, image *galleryImageWithTags) error {
	imageURL := buildDisplayRenderURL(client, image)
	if imageURL == "" {
		return fmt.Errorf("图片渲染地址为空，无法发送")
	}
	blobID := fmt.Sprintf("gallery-image-%d-%d", image.ID, time.Now().Unix())
	onebotURL, err := uploadOneBotImageViaBlob(ctx, host, imageURL, blobID)
	if err != nil {
		return err
	}
	return util.SendImage(ctx, host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, onebotURL)
}

func uploadOneBotImageViaBlob(ctx context.Context, host util.HostCaller, sourceURL string, blobID string) (string, error) {
	sourceURL = strings.TrimSpace(sourceURL)
	blobID = strings.TrimSpace(blobID)
	if sourceURL == "" {
		return "", fmt.Errorf("图片源地址为空，无法上传")
	}
	if blobID == "" {
		return "", fmt.Errorf("blob_id 为空，无法上传")
	}
	uploaded := util.UploadViaBlobPlugin(ctx, host, sourceURL, blobID, "image")
	if strings.TrimSpace(uploaded) == "" {
		return "", fmt.Errorf("上传图片到 Blob 服务失败")
	}
	return strings.TrimSpace(uploaded), nil
}

func (g *GalleryPlugin) snapshotConfig() galleryConfig {
	g.mu.RLock()
	defer g.mu.RUnlock()
	return g.cfg
}

func (g *GalleryPlugin) newClient() *galleryClient {
	cfg := g.snapshotConfig()
	return newGalleryClient(cfg)
}

func parseMessageEvent(eventRaw ob11.Event) (map[string]any, messageContext, bool) {
	var evt map[string]any
	if err := json.Unmarshal(eventRaw, &evt); err != nil {
		return nil, messageContext{}, false
	}
	ctx := messageContext{
		MsgType: toString(evt["message_type"]),
		GroupID: evt["group_id"],
		UserID:  evt["user_id"],
		SelfID:  evt["self_id"],
		Content: toString(evt["content"]),
		Payload: evt,
	}
	if ctx.SelfID == nil {
		ctx.SelfID = ctx.UserID
	}
	return evt, ctx, true
}

func parseTags(input string) []string {
	parts := strings.Split(strings.TrimSpace(input), ",")
	result := make([]string, 0, len(parts))
	seen := make(map[string]struct{}, len(parts))
	for _, part := range parts {
		trimmed := strings.TrimSpace(part)
		if trimmed == "" {
			continue
		}
		lowered := strings.ToLower(trimmed)
		if _, ok := seen[lowered]; ok {
			continue
		}
		seen[lowered] = struct{}{}
		result = append(result, trimmed)
	}
	return result
}

func firstGroup(match *papi.CommandMatch) string {
	if match == nil || len(match.Groups) == 0 {
		return ""
	}
	return strings.TrimSpace(match.Groups[0])
}

func looksLikeImageID(input string) bool {
	if strings.TrimSpace(input) == "" {
		return false
	}
	_, err := strconv.ParseInt(strings.TrimSpace(input), 10, 64)
	return err == nil
}

func buildUploadSummary(sourceLabel string, tags []string, outcomes []uploadOutcome) string {
	successIDs := make([]string, 0)
	duplicateIDs := make([]string, 0)
	failures := make([]string, 0)
	for _, outcome := range outcomes {
		switch {
		case outcome.Uploaded != nil:
			successIDs = append(successIDs, fmt.Sprintf("#%d", outcome.Uploaded.ID))
		case outcome.DuplicateImageID > 0:
			duplicateIDs = append(duplicateIDs, fmt.Sprintf("第 %d 张→#%d", outcome.Index, outcome.DuplicateImageID))
		case outcome.Failure != "":
			failures = append(failures, fmt.Sprintf("第 %d 张：%s", outcome.Index, outcome.Failure))
		}
	}

	lines := []string{fmt.Sprintf("画廊上传结果（来源：%s）", nonEmptyOr(sourceLabel, "未知来源"))}
	if len(tags) > 0 {
		lines = append(lines, "目标标签："+strings.Join(tags, "、"))
	}
	lines = append(lines, fmt.Sprintf("成功 %d 张 / 重复 %d 张 / 失败 %d 张", len(successIDs), len(duplicateIDs), len(failures)))
	if len(successIDs) > 0 {
		lines = append(lines, "新上传："+strings.Join(successIDs, "、"))
	}
	if len(duplicateIDs) > 0 {
		lines = append(lines, "重复图片："+strings.Join(duplicateIDs, "、"))
	}
	if len(failures) > 0 {
		lines = append(lines, "失败详情："+strings.Join(failures, "；"))
	}
	return strings.Join(lines, "\n")
}

func buildImageMetaText(image *galleryImageWithTags) string {
	if image == nil {
		return ""
	}
	lines := []string{fmt.Sprintf("图片 #%d", image.ID)}
	if names := tagNames(image.Tags); len(names) > 0 {
		lines = append(lines, "标签："+strings.Join(names, "、"))
	}
	if image.Width > 0 && image.Height > 0 {
		lines = append(lines, fmt.Sprintf("尺寸：%d×%d", image.Width, image.Height))
	}
	if image.FileSize > 0 {
		lines = append(lines, fmt.Sprintf("大小：%s", humanBytes(image.FileSize)))
	}
	if !image.CreatedAt.IsZero() {
		lines = append(lines, "收录时间："+image.CreatedAt.Local().Format("2006-01-02 15:04:05"))
	}
	return strings.Join(lines, "\n")
}

func humanBytes(size int64) string {
	const unit = 1024
	if size < unit {
		return fmt.Sprintf("%d B", size)
	}
	div, exp := int64(unit), 0
	for n := size / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %ciB", float64(size)/float64(div), "KMGTPE"[exp])
}

func sanitizeBriefError(err error) string {
	if err == nil {
		return "未知错误"
	}
	msg := util.SanitizeError(err)
	msg = strings.TrimSpace(msg)
	if msg == "" {
		return "未知错误"
	}
	return msg
}

func tagNames(tags []galleryTag) []string {
	result := make([]string, 0, len(tags))
	for _, tag := range tags {
		if trimmed := strings.TrimSpace(tag.Name); trimmed != "" {
			result = append(result, trimmed)
		}
	}
	return result
}

func nonEmptyOr(value string, fallback string) string {
	value = strings.TrimSpace(value)
	if value != "" {
		return value
	}
	return fallback
}

func buildDisplayRenderURL(client *galleryClient, image *galleryImageWithTags) string {
	if client == nil || image == nil {
		return ""
	}
	return client.buildRenderURL(image.ID)
}

type duplicateComparePageParams struct {
	CurrentImageURL  string
	DuplicateImageID int64
	CurrentTags      []string
	ExistingTags     []string
}

func buildDuplicateComparePageURL(pagesHost string, params duplicateComparePageParams) string {
	query := map[string]string{
		"current_image_url": strings.TrimSpace(params.CurrentImageURL),
		"duplicate_id":      strconv.FormatInt(params.DuplicateImageID, 10),
		"current_tags":      strings.Join(params.CurrentTags, ", "),
		"existing_tags":     strings.Join(params.ExistingTags, ", "),
	}
	return util.BuildPagesURL(pagesHost, "/gallery/duplicate", query)
}

func main() {
	logger := hclog.New(&hclog.LoggerOptions{
		Name:       "nyanyabot-plugin-amiabot-gallery",
		Level:      hclog.Info,
		Output:     os.Stderr,
		JSONFormat: true,
	})
	hclog.SetDefault(logger)
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &GalleryPlugin{cfg: defaultGalleryConfig()}},
		},
		Logger: logger,
	})
}
