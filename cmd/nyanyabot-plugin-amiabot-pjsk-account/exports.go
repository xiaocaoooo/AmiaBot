package main

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	hclog "github.com/hashicorp/go-hclog"
)

// AddParams 添加账户参数
type AddParams struct {
	QQID      int64  `json:"qq_id"`
	GameServer string `json:"game_server"`
	GameID    string `json:"game_id"`
}

// GetParams 获取账户参数
type GetParams struct {
	QQID      int64  `json:"qq_id"`
	GameServer string `json:"game_server"`
	GameID    string `json:"game_id"`
}

// ListByQQParams 根据 QQ 号列出账户参数
type ListByQQParams struct {
	QQID        int64 `json:"qq_id"`
	EnabledOnly bool  `json:"enabled_only"`
}

// ListByGameIDParams 根据游戏 ID 列出账户参数
type ListByGameIDParams struct {
	GameServer  string `json:"game_server"`
	GameID      string `json:"game_id"`
	EnabledOnly bool   `json:"enabled_only"`
}

// SetEnabledParams 设置启用状态参数
type SetEnabledParams struct {
	QQID      int64  `json:"qq_id"`
	GameServer string `json:"game_server"`
	GameID    string `json:"game_id"`
	Enabled   bool   `json:"enabled"`
}

// RemoveParams 删除账户参数
type RemoveParams struct {
	QQID      int64  `json:"qq_id"`
	GameServer string `json:"game_server"`
	GameID    string `json:"game_id"`
}

// GetPreferredServerParams 获取默认服务器参数
type GetPreferredServerParams struct {
	QQID int64 `json:"qq_id"`
}

// SetPreferredServerParams 设置默认服务器参数
type SetPreferredServerParams struct {
	QQID   int64  `json:"qq_id"`
	Server string `json:"server"`
}

// handleAdd 处理添加账户请求
func (p *PJSKAccount) handleAdd(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params AddParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	// 验证参数
	if params.QQID == 0 {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "QQ号不能为空",
		}), nil
	}
	if !validServers[params.GameServer] {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "无效的游戏服务器，只支持 jp/cn/en/tw/kr",
		}), nil
	}
	if params.GameID == "" {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "游戏ID不能为空",
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	// 检查是否已存在，如果存在则更新为启用
	var existingAccount Account
	err := db.QueryRowContext(ctx,
		"SELECT qq_id, game_server, game_id, created_at, enabled FROM pjsk_accounts WHERE qq_id = $1 AND game_server = $2 AND game_id = $3",
		params.QQID, params.GameServer, params.GameID,
	).Scan(&existingAccount.QQID, &existingAccount.GameServer, &existingAccount.GameID, &existingAccount.CreatedAt, &existingAccount.Enabled)

	now := time.Now()
	var account Account

	if err == sql.ErrNoRows {
		// 插入新记录
		_, err = db.ExecContext(ctx,
			"INSERT INTO pjsk_accounts (qq_id, game_server, game_id, created_at, enabled) VALUES ($1, $2, $3, $4, TRUE)",
			params.QQID, params.GameServer, params.GameID, now,
		)
		if err != nil {
			hclog.L().Error("[Account] 添加账户失败", "error", err)
			return jsonResult(map[string]interface{}{
				"success": false,
				"message": "添加账户失败: " + err.Error(),
			}), nil
		}
		account = Account{
			QQID:      params.QQID,
			GameServer: params.GameServer,
			GameID:    params.GameID,
			CreatedAt: now,
			Enabled:   true,
		}
		hclog.L().Info("[Account] 添加账户成功", "qq_id", params.QQID, "server", params.GameServer, "game_id", params.GameID)
	} else if err != nil {
		hclog.L().Error("[Account] 查询账户失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "查询账户失败: " + err.Error(),
		}), nil
	} else {
		// 已存在，更新为启用
		_, err = db.ExecContext(ctx,
			"UPDATE pjsk_accounts SET enabled = TRUE WHERE qq_id = $1 AND game_server = $2 AND game_id = $3",
			params.QQID, params.GameServer, params.GameID,
		)
		if err != nil {
			hclog.L().Error("[Account] 更新账户失败", "error", err)
			return jsonResult(map[string]interface{}{
				"success": false,
				"message": "更新账户失败: " + err.Error(),
			}), nil
		}
		account = Account{
			QQID:      existingAccount.QQID,
			GameServer: existingAccount.GameServer,
			GameID:    existingAccount.GameID,
			CreatedAt: existingAccount.CreatedAt,
			Enabled:   true,
		}
		hclog.L().Info("[Account] 账户已存在，已启用", "qq_id", params.QQID, "server", params.GameServer, "game_id", params.GameID)
	}

	return jsonResult(map[string]interface{}{
		"success": true,
		"account": account,
		"message": "账户添加成功",
	}), nil
}

// handleGet 处理获取账户请求
func (p *PJSKAccount) handleGet(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params GetParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	var account Account
	err := db.QueryRowContext(ctx,
		"SELECT qq_id, game_server, game_id, created_at, enabled FROM pjsk_accounts WHERE qq_id = $1 AND game_server = $2 AND game_id = $3",
		params.QQID, params.GameServer, params.GameID,
	).Scan(&account.QQID, &account.GameServer, &account.GameID, &account.CreatedAt, &account.Enabled)

	if err == sql.ErrNoRows {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "账户不存在",
		}), nil
	} else if err != nil {
		hclog.L().Error("[Account] 查询账户失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "查询账户失败: " + err.Error(),
		}), nil
	}

	return jsonResult(map[string]interface{}{
		"success": true,
		"account": account,
	}), nil
}

// handleListByQQ 处理根据 QQ 号列出账户请求
func (p *PJSKAccount) handleListByQQ(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params ListByQQParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	var query string
	var rows *sql.Rows
	var err error

	if params.EnabledOnly {
		query = "SELECT qq_id, game_server, game_id, created_at, enabled FROM pjsk_accounts WHERE qq_id = $1 AND enabled = TRUE ORDER BY created_at"
		rows, err = db.QueryContext(ctx, query, params.QQID)
	} else {
		query = "SELECT qq_id, game_server, game_id, created_at, enabled FROM pjsk_accounts WHERE qq_id = $1 ORDER BY created_at"
		rows, err = db.QueryContext(ctx, query, params.QQID)
	}

	if err != nil {
		hclog.L().Error("[Account] 查询账户列表失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "查询账户列表失败: " + err.Error(),
		}), nil
	}
	defer rows.Close()

	accounts := []Account{}
	for rows.Next() {
		var account Account
		if err := rows.Scan(&account.QQID, &account.GameServer, &account.GameID, &account.CreatedAt, &account.Enabled); err != nil {
			hclog.L().Error("[Account] 扫描账户失败", "error", err)
			continue
		}
		accounts = append(accounts, account)
	}

	return jsonResult(map[string]interface{}{
		"success":  true,
		"accounts": accounts,
	}), nil
}

// handleListByGameID 处理根据游戏 ID 列出账户请求
func (p *PJSKAccount) handleListByGameID(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params ListByGameIDParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	var query string
	var rows *sql.Rows
	var err error

	if params.EnabledOnly {
		query = "SELECT qq_id, game_server, game_id, created_at, enabled FROM pjsk_accounts WHERE game_server = $1 AND game_id = $2 AND enabled = TRUE ORDER BY created_at"
		rows, err = db.QueryContext(ctx, query, params.GameServer, params.GameID)
	} else {
		query = "SELECT qq_id, game_server, game_id, created_at, enabled FROM pjsk_accounts WHERE game_server = $1 AND game_id = $2 ORDER BY created_at"
		rows, err = db.QueryContext(ctx, query, params.GameServer, params.GameID)
	}

	if err != nil {
		hclog.L().Error("[Account] 查询账户列表失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "查询账户列表失败: " + err.Error(),
		}), nil
	}
	defer rows.Close()

	accounts := []Account{}
	for rows.Next() {
		var account Account
		if err := rows.Scan(&account.QQID, &account.GameServer, &account.GameID, &account.CreatedAt, &account.Enabled); err != nil {
			hclog.L().Error("[Account] 扫描账户失败", "error", err)
			continue
		}
		accounts = append(accounts, account)
	}

	return jsonResult(map[string]interface{}{
		"success":  true,
		"accounts": accounts,
	}), nil
}

// handleSetEnabled 处理设置启用状态请求
func (p *PJSKAccount) handleSetEnabled(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params SetEnabledParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	result, err := db.ExecContext(ctx,
		"UPDATE pjsk_accounts SET enabled = $1 WHERE qq_id = $2 AND game_server = $3 AND game_id = $4",
		params.Enabled, params.QQID, params.GameServer, params.GameID,
	)
	if err != nil {
		hclog.L().Error("[Account] 更新账户状态失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "更新账户状态失败: " + err.Error(),
		}), nil
	}

	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "账户不存在",
		}), nil
	}

	hclog.L().Info("[Account] 更新账户状态成功", "qq_id", params.QQID, "server", params.GameServer, "game_id", params.GameID, "enabled", params.Enabled)
	return jsonResult(map[string]interface{}{
		"success": true,
		"message": "状态更新成功",
	}), nil
}

// handleRemove 处理删除账户请求
func (p *PJSKAccount) handleRemove(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params RemoveParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	result, err := db.ExecContext(ctx,
		"DELETE FROM pjsk_accounts WHERE qq_id = $1 AND game_server = $2 AND game_id = $3",
		params.QQID, params.GameServer, params.GameID,
	)
	if err != nil {
		hclog.L().Error("[Account] 删除账户失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "删除账户失败: " + err.Error(),
		}), nil
	}

	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "账户不存在",
		}), nil
	}

	hclog.L().Info("[Account] 删除账户成功", "qq_id", params.QQID, "server", params.GameServer, "game_id", params.GameID)
	return jsonResult(map[string]interface{}{
		"success": true,
		"message": "账户删除成功",
	}), nil
}

// jsonResult 辅助函数，将结果转换为 JSON
func jsonResult(data interface{}) json.RawMessage {
	b, _ := json.Marshal(data)
	return b
}

// handleGetPreferredServer 处理获取用户默认服务器请求
func (p *PJSKAccount) handleGetPreferredServer(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params GetPreferredServerParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	if params.QQID == 0 {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "QQ号不能为空",
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	var server string
	err := db.QueryRowContext(ctx,
		"SELECT preferred_server FROM pjsk_user_settings WHERE qq_id = $1",
		params.QQID,
	).Scan(&server)

	if err == sql.ErrNoRows {
		// 没有记录，返回默认值
		return jsonResult(map[string]interface{}{
			"success": true,
			"server":  "",
			"message": "用户未设置默认服务器",
		}), nil
	} else if err != nil {
		hclog.L().Error("[Account] 查询默认服务器失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "查询默认服务器失败: " + err.Error(),
		}), nil
	}

	return jsonResult(map[string]interface{}{
		"success": true,
		"server":  server,
	}), nil
}

// handleSetPreferredServer 处理设置用户默认服务器请求
func (p *PJSKAccount) handleSetPreferredServer(ctx context.Context, paramsJSON json.RawMessage) (json.RawMessage, error) {
	var params SetPreferredServerParams
	if err := json.Unmarshal(paramsJSON, &params); err != nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "参数解析失败: " + err.Error(),
		}), nil
	}

	if params.QQID == 0 {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "QQ号不能为空",
		}), nil
	}
	if !validServers[params.Server] {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "无效的服务器，只支持 jp/cn/en/tw/kr",
		}), nil
	}

	p.mu.RLock()
	db := p.db
	p.mu.RUnlock()

	if db == nil {
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "数据库未连接",
		}), nil
	}

	// UPSERT: 如果存在则更新，不存在则插入
	_, err := db.ExecContext(ctx,
		`INSERT INTO pjsk_user_settings (qq_id, preferred_server, updated_at)
		 VALUES ($1, $2, CURRENT_TIMESTAMP)
		 ON CONFLICT (qq_id) DO UPDATE SET preferred_server = $2, updated_at = CURRENT_TIMESTAMP`,
		params.QQID, params.Server,
	)
	if err != nil {
		hclog.L().Error("[Account] 设置默认服务器失败", "error", err)
		return jsonResult(map[string]interface{}{
			"success": false,
			"message": "设置默认服务器失败: " + err.Error(),
		}), nil
	}

	serverUpper := strings.ToUpper(params.Server)
	hclog.L().Info("[Account] 设置默认服务器成功", "qq_id", params.QQID, "server", serverUpper)
	return jsonResult(map[string]interface{}{
		"success": true,
		"message": fmt.Sprintf("默认服务器已设置为 [%s]", serverUpper),
	}), nil
}
