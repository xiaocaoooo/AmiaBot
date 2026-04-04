package main

import (
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/util"
)

// 接口兼容性检查：transport.HostRPCClient 必须实现 util.HostCaller
var _ util.HostCaller = (*transport.HostRPCClient)(nil)
