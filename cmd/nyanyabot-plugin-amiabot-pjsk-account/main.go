package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"sync"
	"time"

	hclog "github.com/hashicorp/go-hclog"
	"github.com/hashicorp/go-plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/onebot/ob11"
	papi "github.com/xiaocaoooo/amiabot-plugin-sdk/plugin"
	"github.com/xiaocaoooo/amiabot-plugin-sdk/plugin/transport"

	_ "github.com/lib/pq"
)

// PJSKAccount 插件主结构
type PJSKAccount struct {
	mu   sync.RWMutex
	cfg  config
	db   *sql.DB
}

type config struct {
	DatabaseURL  string `json:"database_url"`
	DefaultServer string `json:"default_server"`
}

// Account 账户信息结构
type Account struct {
	QQID      int64     `json:"qq_id"`
	GameServer string    `json:"game_server"`
	GameID    string    `json:"game_id"`
	CreatedAt time.Time `json:"created_at"`
	Enabled   bool      `json:"enabled"`
}

var validServers = map[string]bool{
	"jp": true,
	"cn": true,
	"en": true,
	"tw": true,
	"kr": true,
}

func (p *PJSKAccount) Descriptor(ctx context.Context) (papi.Descriptor, error) {
	_ = ctx
	schema := json.RawMessage(`{
		"type":"object",
		"properties":{
			"database_url":{"type":"string","description":"PostgreSQL 数据库连接字符串"},
			"default_server":{"type":"string","description":"默认服务器 (jp/cn/en/tw/kr)，不填时为 jp"}
		},
		"required":["database_url"],
		"additionalProperties":false
	}`)
	def := json.RawMessage(`{"database_url":"","default_server":"jp"}`)
	return papi.Descriptor{
		Name:        "Amiabot PJSK Account",
		PluginID:    "external.amiabot-pjsk-account",
		Version:     "0.1.0",
		Author:      "nyanyabot",
		Description: "PJSK 游戏账户管理插件，提供账户的添加、获取、设置启用状态等功能，供其他插件调用",
		Dependencies: []string{},
		Exports: []papi.ExportSpec{
			{
				Name:        "account.add",
				Description: "添加账户，如果已存在则设为启用",
				ParamsSchema: json.RawMessage(`{"type":"object","properties":{"qq_id":{"type":"integer"},"game_server":{"type":"string"},"game_id":{"type":"string"}},"required":["qq_id","game_server","game_id"]}`),
				ResultSchema: json.RawMessage(`{"type":"object","properties":{"success":{"type":"boolean"},"account":{"type":"object"},"message":{"type":"string"}}}`),
			},
			{
				Name:        "account.get",
				Description: "获取单个账户",
				ParamsSchema: json.RawMessage(`{"type":"object","properties":{"qq_id":{"type":"integer"},"game_server":{"type":"string"},"game_id":{"type":"string"}},"required":["qq_id","game_server","game_id"]}`),
				ResultSchema: json.RawMessage(`{"type":"object","properties":{"success":{"type":"boolean"},"account":{"type":"object"},"message":{"type":"string"}}}`),
			},
			{
				Name:        "account.list_by_qq",
				Description: "根据 QQ 号获取所有账户",
				ParamsSchema: json.RawMessage(`{"type":"object","properties":{"qq_id":{"type":"integer"},"enabled_only":{"type":"boolean"}},"required":["qq_id"]}`),
				ResultSchema: json.RawMessage(`{"type":"object","properties":{"success":{"type":"boolean"},"accounts":{"type":"array"}}}`),
			},
			{
				Name:        "account.list_by_game_id",
				Description: "根据游戏 ID 获取所有账户",
				ParamsSchema: json.RawMessage(`{"type":"object","properties":{"game_server":{"type":"string"},"game_id":{"type":"string"},"enabled_only":{"type":"boolean"}},"required":["game_server","game_id"]}`),
				ResultSchema: json.RawMessage(`{"type":"object","properties":{"success":{"type":"boolean"},"accounts":{"type":"array"}}}`),
			},
			{
				Name:        "account.set_enabled",
				Description: "设置账户启用状态",
				ParamsSchema: json.RawMessage(`{"type":"object","properties":{"qq_id":{"type":"integer"},"game_server":{"type":"string"},"game_id":{"type":"string"},"enabled":{"type":"boolean"}},"required":["qq_id","game_server","game_id","enabled"]}`),
				ResultSchema: json.RawMessage(`{"type":"object","properties":{"success":{"type":"boolean"},"message":{"type":"string"}}}`),
			},
			{
				Name:        "account.remove",
				Description: "删除账户",
				ParamsSchema: json.RawMessage(`{"type":"object","properties":{"qq_id":{"type":"integer"},"game_server":{"type":"string"},"game_id":{"type":"string"}},"required":["qq_id","game_server","game_id"]}`),
				ResultSchema: json.RawMessage(`{"type":"object","properties":{"success":{"type":"boolean"},"message":{"type":"string"}}}`),
			},
		},
		Config: &papi.ConfigSpec{
			Version:     "1",
			Description: "Amiabot PJSK Account plugin config",
			Schema:      schema,
			Default:     def,
		},
		Commands: []papi.CommandListener{},
		Events:   []papi.EventListener{},
	}, nil
}

func (p *PJSKAccount) Configure(ctx context.Context, configJSON json.RawMessage) error {
	_ = ctx
	cfg := config{
		DefaultServer: "jp",
	}
	if len(configJSON) > 0 {
		_ = json.Unmarshal(configJSON, &cfg)
	}

	// 验证默认服务器
	if !validServers[cfg.DefaultServer] {
		cfg.DefaultServer = "jp"
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	// 如果数据库连接字符串变化，重新连接
	if p.cfg.DatabaseURL != cfg.DatabaseURL || p.db == nil {
		// 关闭旧连接
		if p.db != nil {
			_ = p.db.Close()
		}

		// 建立新连接
		if cfg.DatabaseURL != "" {
			db, err := sql.Open("postgres", cfg.DatabaseURL)
			if err != nil {
				hclog.L().Error("[Account] 数据库连接失败", "error", err)
				p.db = nil
			} else {
				db.SetMaxOpenConns(10)
				db.SetMaxIdleConns(5)
				db.SetConnMaxLifetime(time.Hour)

				// 创建表
				if err := p.createTable(db); err != nil {
					hclog.L().Error("[Account] 创建表失败", "error", err)
					_ = db.Close()
					p.db = nil
				} else {
					p.db = db
					hclog.L().Info("[Account] 数据库连接成功")
				}
			}
		}
	}

	p.cfg = cfg
	return nil
}

func (p *PJSKAccount) createTable(db *sql.DB) error {
	query := `
	CREATE TABLE IF NOT EXISTS pjsk_accounts (
		qq_id BIGINT NOT NULL,
		game_server VARCHAR(4) NOT NULL,
		game_id VARCHAR(64) NOT NULL,
		created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
		enabled BOOLEAN DEFAULT TRUE,
		PRIMARY KEY (qq_id, game_server, game_id)
	);
	CREATE INDEX IF NOT EXISTS idx_pjsk_accounts_qq_id ON pjsk_accounts(qq_id);
	CREATE INDEX IF NOT EXISTS idx_pjsk_accounts_game_id ON pjsk_accounts(game_id);
	`
	_, err := db.Exec(query)
	return err
}

func (p *PJSKAccount) Invoke(ctx context.Context, method string, paramsJSON json.RawMessage, callerPluginID string) (json.RawMessage, error) {
	hclog.L().Info("[Account] Invoke called", "method", method, "caller", callerPluginID)

	switch method {
	case "account.add":
		return p.handleAdd(ctx, paramsJSON)
	case "account.get":
		return p.handleGet(ctx, paramsJSON)
	case "account.list_by_qq":
		return p.handleListByQQ(ctx, paramsJSON)
	case "account.list_by_game_id":
		return p.handleListByGameID(ctx, paramsJSON)
	case "account.set_enabled":
		return p.handleSetEnabled(ctx, paramsJSON)
	case "account.remove":
		return p.handleRemove(ctx, paramsJSON)
	default:
		return nil, papi.NewStructuredError(papi.ErrorCodeNotFound, "method not found: "+method)
	}
}

func (p *PJSKAccount) Handle(ctx context.Context, listenerID string, eventRaw ob11.Event, match *papi.CommandMatch) (papi.HandleResult, error) {
	_ = ctx
	_ = listenerID
	_ = eventRaw
	_ = match
	return papi.HandleResult{}, nil
}

func (p *PJSKAccount) Shutdown(ctx context.Context) error {
	_ = ctx
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.db != nil {
		return p.db.Close()
	}
	return nil
}

func main() {
	logger := hclog.New(&hclog.LoggerOptions{Name: "nyanyabot-plugin-amiabot-pjsk-account", Level: hclog.Info})

	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: transport.Handshake(),
		Plugins: plugin.PluginSet{
			transport.PluginName: &transport.Map{PluginImpl: &PJSKAccount{}},
		},
		Logger: logger,
	})
}
