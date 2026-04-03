package main

import (
	"regexp"
	"strings"
)

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

// normalizeServer 标准化服务器名称
func normalizeServer(server string) string {
	server = strings.ToLower(strings.TrimSpace(server))
	if validServers[server] {
		return server
	}
	return ""
}

// isValidServer 检查服务器是否有效
func isValidServer(server string) bool {
	return validServers[strings.ToLower(strings.TrimSpace(server))]
}
