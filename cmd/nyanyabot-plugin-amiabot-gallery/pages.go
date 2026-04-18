package main

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

type galleryPagesViewMode int

const (
	galleryPagesViewNone galleryPagesViewMode = iota
	galleryPagesViewAllTags
	galleryPagesViewAllImages
)

func parseGalleryPagesViewInput(input string) (galleryPagesViewMode, []string) {
	input = strings.TrimSpace(input)
	if input == "所有" {
		return galleryPagesViewAllTags, nil
	}
	if !strings.HasPrefix(input, "所有") {
		return galleryPagesViewNone, nil
	}
	tags := parseTags(strings.TrimSpace(strings.TrimPrefix(input, "所有")))
	if len(tags) == 0 {
		return galleryPagesViewNone, nil
	}
	return galleryPagesViewAllImages, tags
}

func buildGalleryAllTagsPageURL(pagesHost string) string {
	return util.BuildPagesURL(pagesHost, "/gallery/tags", nil)
}

func buildGalleryAllImagesPageURL(pagesHost string, tags []string) string {
	if len(tags) == 0 {
		return ""
	}
	return util.BuildPagesURL(pagesHost, "/gallery/images", map[string]string{
		"tags": strings.Join(tags, ","),
	})
}

func (g *GalleryPlugin) sendGalleryPagesCard(ctx context.Context, host util.HostCaller, msgCtx messageContext, pageURL string, blobPrefix string) error {
	pageURL = strings.TrimSpace(pageURL)
	blobPrefix = strings.TrimSpace(blobPrefix)
	if pageURL == "" {
		return fmt.Errorf("页面地址为空，无法生成截图")
	}
	if blobPrefix == "" {
		blobPrefix = "gallery-pages"
	}

	screenshotURL, err := util.BuildScreenshotViaPlugin(host, pageURL)
	if err != nil {
		return err
	}
	blobID := fmt.Sprintf("%s-%d", blobPrefix, time.Now().Unix())
	onebotURL, err := uploadOneBotImageViaBlob(ctx, host, screenshotURL, blobID)
	if err != nil {
		return err
	}
	return util.SendImage(host, msgCtx.MsgType, msgCtx.GroupID, msgCtx.UserID, onebotURL)
}

func (g *GalleryPlugin) sendGalleryAllTagsCard(ctx context.Context, host util.HostCaller, msgCtx messageContext) error {
	cfg := g.snapshotConfig()
	if strings.TrimSpace(cfg.AmiabotPages) == "" {
		return fmt.Errorf("未配置 amiabot_pages")
	}
	pageURL := buildGalleryAllTagsPageURL(cfg.AmiabotPages)
	if pageURL == "" {
		return fmt.Errorf("标签总览页地址构造失败")
	}
	return g.sendGalleryPagesCard(ctx, host, msgCtx, pageURL, "gallery-tags")
}

func (g *GalleryPlugin) sendGalleryAllImagesCard(ctx context.Context, host util.HostCaller, msgCtx messageContext, tags []string) error {
	cfg := g.snapshotConfig()
	if strings.TrimSpace(cfg.AmiabotPages) == "" {
		return fmt.Errorf("未配置 amiabot_pages")
	}
	pageURL := buildGalleryAllImagesPageURL(cfg.AmiabotPages, tags)
	if pageURL == "" {
		return fmt.Errorf("标签图片页地址构造失败")
	}
	return g.sendGalleryPagesCard(ctx, host, msgCtx, pageURL, "gallery-images")
}
