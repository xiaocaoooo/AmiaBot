package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

type galleryClient struct {
	base       string
	readToken  string
	writeToken string
	httpClient *http.Client
}

type galleryAPIError struct {
	StatusCode       int
	Message          string
	DuplicateImageID int64
}

func (e *galleryAPIError) Error() string {
	if e == nil {
		return "画廊服务发生错误"
	}
	if strings.TrimSpace(e.Message) != "" {
		return e.Message
	}
	if e.StatusCode > 0 {
		return fmt.Sprintf("画廊服务返回 HTTP %d", e.StatusCode)
	}
	return "画廊服务发生错误"
}

type galleryTag struct {
	ID        int64     `json:"id"`
	Name      string    `json:"name"`
	CreatedAt time.Time `json:"created_at"`
}

type galleryImage struct {
	ID          int64     `json:"id"`
	Filename    string    `json:"filename"`
	FID         string    `json:"fid"`
	FileSize    int64     `json:"file_size"`
	Width       int       `json:"width"`
	Height      int       `json:"height"`
	MimeType    string    `json:"mime_type"`
	PHash       int64     `json:"phash"`
	IsAnimated  bool      `json:"is_animated"`
	Description string    `json:"description"`
	CreatedAt   time.Time `json:"created_at"`
}

type galleryImageWithTags struct {
	galleryImage
	Tags []galleryTag `json:"tags"`
}

type galleryImageListResponse struct {
	Items    []galleryImageWithTags `json:"items"`
	Page     int                    `json:"page"`
	PageSize int                    `json:"page_size"`
	Total    int64                  `json:"total"`
}

func newGalleryClient(cfg galleryConfig) *galleryClient {
	return &galleryClient{
		base:       util.NormalizeHTTPBase(cfg.GalleryServer),
		readToken:  strings.TrimSpace(cfg.GalleryReadToken),
		writeToken: strings.TrimSpace(cfg.GalleryWriteToken),
		httpClient: &http.Client{Timeout: 60 * time.Second},
	}
}

func (c *galleryClient) createTag(ctx context.Context, name string) (*galleryTag, error) {
	body, err := json.Marshal(map[string]string{"name": strings.TrimSpace(name)})
	if err != nil {
		return nil, err
	}
	req, err := c.newJSONRequest(ctx, http.MethodPost, "/v1/tags", bytes.NewReader(body), true)
	if err != nil {
		return nil, err
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusCreated {
		return nil, c.decodeAPIError(resp)
	}
	var tag galleryTag
	if err := json.NewDecoder(resp.Body).Decode(&tag); err != nil {
		return nil, err
	}
	return &tag, nil
}

func (c *galleryClient) listTags(ctx context.Context, q string, limit int) ([]galleryTag, error) {
	path := "/v1/tags"
	params := map[string]string{}
	if strings.TrimSpace(q) != "" {
		params["q"] = strings.TrimSpace(q)
	}
	if limit > 0 {
		params["limit"] = strconv.Itoa(limit)
	}
	req, err := c.newJSONRequest(ctx, http.MethodGet, c.buildPath(path, params), nil, false)
	if err != nil {
		return nil, err
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, c.decodeAPIError(resp)
	}
	var payload struct {
		Items []galleryTag `json:"items"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&payload); err != nil {
		return nil, err
	}
	return payload.Items, nil
}

func (c *galleryClient) findExactTag(ctx context.Context, name string) (*galleryTag, error) {
	items, err := c.listTags(ctx, strings.TrimSpace(name), 100)
	if err != nil {
		return nil, err
	}
	for _, item := range items {
		if strings.EqualFold(strings.TrimSpace(item.Name), strings.TrimSpace(name)) {
			copied := item
			return &copied, nil
		}
	}
	return nil, nil
}

func (c *galleryClient) findMissingTags(ctx context.Context, tags []string) ([]string, error) {
	missing := make([]string, 0)
	for _, tag := range tags {
		item, err := c.findExactTag(ctx, tag)
		if err != nil {
			return nil, err
		}
		if item == nil {
			missing = append(missing, tag)
		}
	}
	return missing, nil
}

func (c *galleryClient) uploadImage(ctx context.Context, filename string, data []byte, tags []string, force bool) (*galleryImageWithTags, error) {
	var body bytes.Buffer
	writer := multipart.NewWriter(&body)
	fileWriter, err := writer.CreateFormFile("file", filename)
	if err != nil {
		return nil, err
	}
	if _, err := fileWriter.Write(data); err != nil {
		return nil, err
	}
	for _, tag := range tags {
		if err := writer.WriteField("tags", tag); err != nil {
			return nil, err
		}
	}
	if force {
		if err := writer.WriteField("force", "true"); err != nil {
			return nil, err
		}
	}
	if err := writer.Close(); err != nil {
		return nil, err
	}

	req, err := c.newRequest(ctx, http.MethodPost, "/v1/images/upload", &body, true)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", writer.FormDataContentType())
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusCreated {
		return nil, c.decodeAPIError(resp)
	}
	var image galleryImageWithTags
	if err := json.NewDecoder(resp.Body).Decode(&image); err != nil {
		return nil, err
	}
	return &image, nil
}

func (c *galleryClient) getImage(ctx context.Context, imageID int64) (*galleryImageWithTags, error) {
	req, err := c.newJSONRequest(ctx, http.MethodGet, fmt.Sprintf("/v1/images/%d", imageID), nil, false)
	if err != nil {
		return nil, err
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, c.decodeAPIError(resp)
	}
	var image galleryImageWithTags
	if err := json.NewDecoder(resp.Body).Decode(&image); err != nil {
		return nil, err
	}
	return &image, nil
}

func (c *galleryClient) listImages(ctx context.Context, tags []string, page int, pageSize int) ([]galleryImageWithTags, error) {
	params := map[string]string{}
	if len(tags) > 0 {
		params["tags"] = strings.Join(tags, ",")
	}
	if page > 0 {
		params["page"] = strconv.Itoa(page)
	}
	if pageSize > 0 {
		params["page_size"] = strconv.Itoa(pageSize)
	}
	req, err := c.newJSONRequest(ctx, http.MethodGet, c.buildPath("/v1/images", params), nil, false)
	if err != nil {
		return nil, err
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, c.decodeAPIError(resp)
	}
	var payload galleryImageListResponse
	if err := json.NewDecoder(resp.Body).Decode(&payload); err != nil {
		return nil, err
	}
	return payload.Items, nil
}

func (c *galleryClient) randomImage(ctx context.Context, tags []string) (*galleryImageWithTags, error) {
	params := map[string]string{}
	if len(tags) > 0 {
		params["tags"] = strings.Join(tags, ",")
	}
	req, err := c.newJSONRequest(ctx, http.MethodGet, c.buildPath("/v1/images/random", params), nil, false)
	if err != nil {
		return nil, err
	}
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, c.decodeAPIError(resp)
	}
	var image galleryImageWithTags
	if err := json.NewDecoder(resp.Body).Decode(&image); err != nil {
		return nil, err
	}
	return &image, nil
}

func (c *galleryClient) buildRenderURL(imageID int64) string {
	base := util.NormalizeHTTPBase(c.base)
	if base == "" {
		return ""
	}
	parsed, err := url.Parse(base)
	if err != nil {
		return ""
	}
	parsed.Path = strings.TrimRight(parsed.Path, "/") + "/v1/images/" + url.PathEscape(strconv.FormatInt(imageID, 10)) + "/render"
	parsed.RawQuery = ""
	return parsed.String()
}

func (c *galleryClient) readAccessToken() string {
	if strings.TrimSpace(c.readToken) != "" {
		return strings.TrimSpace(c.readToken)
	}
	return strings.TrimSpace(c.writeToken)
}

func (c *galleryClient) newJSONRequest(ctx context.Context, method string, path string, body io.Reader, write bool) (*http.Request, error) {
	req, err := c.newRequest(ctx, method, path, body, write)
	if err != nil {
		return nil, err
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.Header.Set("Accept", "application/json")
	return req, nil
}

func (c *galleryClient) newRequest(ctx context.Context, method string, path string, body io.Reader, write bool) (*http.Request, error) {
	if c == nil || strings.TrimSpace(c.base) == "" {
		return nil, fmt.Errorf("未配置 gallery_server")
	}
	target := path
	if !strings.HasPrefix(path, "http://") && !strings.HasPrefix(path, "https://") {
		target = strings.TrimRight(c.base, "/") + path
	}
	req, err := http.NewRequestWithContext(ctx, method, target, body)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", "nyanyabot-plugin-amiabot-gallery/0.1")
	if write {
		if token := strings.TrimSpace(c.writeToken); token != "" {
			req.Header.Set("Authorization", "Bearer "+token)
		}
	} else if token := c.readAccessToken(); token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	return req, nil
}

func (c *galleryClient) buildPath(path string, params map[string]string) string {
	if len(params) == 0 {
		return path
	}
	parsed, err := url.Parse(path)
	if err != nil {
		return path
	}
	query := parsed.Query()
	for key, value := range params {
		if strings.TrimSpace(value) == "" {
			continue
		}
		query.Set(key, value)
	}
	parsed.RawQuery = query.Encode()
	return parsed.String()
}

func (c *galleryClient) decodeAPIError(resp *http.Response) error {
	if resp == nil {
		return &galleryAPIError{Message: "画廊服务响应为空"}
	}
	body, _ := io.ReadAll(io.LimitReader(resp.Body, 256*1024))
	var payload struct {
		Error            string `json:"error"`
		DuplicateImageID int64  `json:"duplicate_image_id"`
	}
	_ = json.Unmarshal(body, &payload)
	message := strings.TrimSpace(payload.Error)
	if message == "" {
		message = strings.TrimSpace(string(body))
	}
	if message == "" {
		message = fmt.Sprintf("画廊服务返回 HTTP %d", resp.StatusCode)
	}
	return &galleryAPIError{
		StatusCode:       resp.StatusCode,
		Message:          message,
		DuplicateImageID: payload.DuplicateImageID,
	}
}
