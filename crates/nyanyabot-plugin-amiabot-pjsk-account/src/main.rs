use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nyanyabot_proto::{
    CommandMatch, ConfigSpec, Descriptor, ExportSpec, HandleResult, HostClient, Plugin,
    StructuredError, run_plugin_with_host,
};
use parking_lot::RwLock;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing_subscriber::EnvFilter;

struct Plug {
    #[allow(dead_code)]
    host: Arc<AsyncRwLock<Option<HostClient>>>,
    cfg: RwLock<Value>,
    pool: AsyncRwLock<Option<PgPool>>,
    default_server: RwLock<String>,
}

fn valid_server(s: &str) -> bool {
    matches!(s, "jp" | "cn" | "en" | "tw" | "kr")
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot PJSK Account".into(),
        plugin_id: "external.amiabot-pjsk-account".into(),
        version: "0.1.0".into(),
        author: "nyanyabot".into(),
        description:
            "PJSK 游戏账户管理插件，提供账户的添加、获取、设置启用状态等功能，供其他插件调用".into(),
        exports: vec![
            ExportSpec {
                name: "account.add".into(),
                description: "添加账户，如果已存在则设为启用".into(),
                params_schema: json!({"type":"object","properties":{"qq_id":{"type":"integer"},"game_server":{"type":"string"},"game_id":{"type":"string"}},"required":["qq_id","game_server","game_id"]}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.get".into(),
                description: "获取单个账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.list_by_qq".into(),
                description: "根据 QQ 号获取所有账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.list_by_game_id".into(),
                description: "根据游戏 ID 获取所有账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.set_enabled".into(),
                description: "设置账户启用状态".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.remove".into(),
                description: "删除账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.get_preferred_server".into(),
                description: "获取用户默认服务器".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.set_preferred_server".into(),
                description: "设置用户默认服务器".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
        ],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("Amiabot PJSK Account plugin config".into()),
            schema: Some(json!({
                "type":"object",
                "properties":{
                    "database_url":{"type":"string"},
                    "default_server":{"type":"string"}
                },
                "required":["database_url"],
                "additionalProperties": false
            })),
            default: Some(json!({"database_url":"","default_server":"jp"})),
        }),
        ..Default::default()
    }
}

fn param_i64(params: &Value, key: &str) -> i64 {
    params.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}
fn param_str(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}
fn param_bool(params: &Value, key: &str, default: bool) -> bool {
    params.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

#[derive(Debug, Clone)]
struct AccountRow {
    qq_id: i64,
    game_server: String,
    game_id: String,
    created_at: DateTime<Utc>,
    enabled: bool,
}

fn account_json(a: &AccountRow) -> Value {
    json!({
        "qq_id": a.qq_id,
        "game_server": a.game_server,
        "game_id": a.game_id,
        "created_at": a.created_at.to_rfc3339(),
        "enabled": a.enabled,
    })
}

fn map_account(row: (i64, String, String, DateTime<Utc>, bool)) -> AccountRow {
    AccountRow {
        qq_id: row.0,
        game_server: row.1,
        game_id: row.2,
        created_at: row.3,
        enabled: row.4,
    }
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        *self.cfg.write() = config.clone();
        let default_server = config
            .get("default_server")
            .and_then(|v| v.as_str())
            .unwrap_or("jp")
            .trim()
            .to_lowercase();
        *self.default_server.write() = if valid_server(&default_server) {
            default_server
        } else {
            "jp".into()
        };
        let url = config
            .get("database_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            *self.pool.write().await = None;
            return Ok(());
        }
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|e| StructuredError::internal(e.to_string()))?;
        // Align with Go production schema.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pjsk_accounts (
                qq_id BIGINT NOT NULL,
                game_server TEXT NOT NULL,
                game_id TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                PRIMARY KEY (qq_id, game_server, game_id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
        let _ = sqlx::query(
            r#"ALTER TABLE pjsk_accounts ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP"#,
        )
        .execute(&pool)
        .await;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pjsk_user_settings (
                qq_id BIGINT PRIMARY KEY,
                preferred_server TEXT NOT NULL DEFAULT '',
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
        // migrate old default_server column name if present
        let _ = sqlx::query(
            r#"
            DO $$
            BEGIN
              IF EXISTS (
                SELECT 1 FROM information_schema.columns
                WHERE table_name='pjsk_user_settings' AND column_name='default_server'
              ) AND NOT EXISTS (
                SELECT 1 FROM information_schema.columns
                WHERE table_name='pjsk_user_settings' AND column_name='preferred_server'
              ) THEN
                ALTER TABLE pjsk_user_settings RENAME COLUMN default_server TO preferred_server;
              END IF;
            END $$;
            "#,
        )
        .execute(&pool)
        .await;
        let _ = sqlx::query(
            r#"ALTER TABLE pjsk_user_settings ADD COLUMN IF NOT EXISTS preferred_server TEXT NOT NULL DEFAULT ''"#,
        )
        .execute(&pool)
        .await;
        let _ = sqlx::query(
            r#"ALTER TABLE pjsk_user_settings ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP"#,
        )
        .execute(&pool)
        .await;
        *self.pool.write().await = Some(pool);
        Ok(())
    }
    async fn invoke(
        &self,
        method: &str,
        params: Value,
        _caller: &str,
    ) -> Result<Value, StructuredError> {
        // Keep alias for older thin rust callers.
        let method = if method == "account.list" {
            "account.list_by_qq"
        } else {
            method
        };
        let pool = self.pool.read().await.clone();
        let Some(pool) = pool else {
            return Ok(json!({"success": false, "message": "数据库未连接"}));
        };

        match method {
            "account.add" => {
                let qq = param_i64(&params, "qq_id");
                let server = param_str(&params, "game_server").to_lowercase();
                let gid = param_str(&params, "game_id");
                if qq == 0 {
                    return Ok(json!({"success": false, "message": "QQ号不能为空"}));
                }
                if !valid_server(&server) {
                    return Ok(
                        json!({"success": false, "message": "无效的游戏服务器，只支持 jp/cn/en/tw/kr"}),
                    );
                }
                if gid.is_empty() {
                    return Ok(json!({"success": false, "message": "游戏ID不能为空"}));
                }
                sqlx::query(
                    r#"
                    INSERT INTO pjsk_accounts (qq_id, game_server, game_id, enabled)
                    VALUES ($1,$2,$3,TRUE)
                    ON CONFLICT (qq_id, game_server, game_id)
                    DO UPDATE SET enabled = TRUE
                    "#,
                )
                .bind(qq)
                .bind(&server)
                .bind(&gid)
                .execute(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                let row = map_account(
                    sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, bool)>(
                        r#"SELECT qq_id, game_server, game_id, created_at, enabled
                           FROM pjsk_accounts WHERE qq_id=$1 AND game_server=$2 AND game_id=$3"#,
                    )
                    .bind(qq)
                    .bind(&server)
                    .bind(&gid)
                    .fetch_one(&pool)
                    .await
                    .map_err(|e| StructuredError::internal(e.to_string()))?,
                );
                Ok(json!({"success": true, "account": account_json(&row), "message": "ok"}))
            }
            "account.get" => {
                let qq = param_i64(&params, "qq_id");
                let server = param_str(&params, "game_server").to_lowercase();
                let gid = param_str(&params, "game_id");
                let row = sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, bool)>(
                    r#"SELECT qq_id, game_server, game_id, created_at, enabled
                       FROM pjsk_accounts WHERE qq_id=$1 AND game_server=$2 AND game_id=$3"#,
                )
                .bind(qq)
                .bind(&server)
                .bind(&gid)
                .fetch_optional(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?
                .map(map_account);
                match row {
                    Some(a) => Ok(json!({"success": true, "account": account_json(&a)})),
                    None => Ok(json!({"success": false, "message": "账户不存在"})),
                }
            }
            "account.list_by_qq" => {
                let qq = param_i64(&params, "qq_id");
                let enabled_only = param_bool(&params, "enabled_only", false);
                let rows: Vec<AccountRow> = if enabled_only {
                    sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, bool)>(
                        r#"SELECT qq_id, game_server, game_id, created_at, enabled
                           FROM pjsk_accounts WHERE qq_id=$1 AND enabled=TRUE
                           ORDER BY created_at ASC"#,
                    )
                    .bind(qq)
                    .fetch_all(&pool)
                    .await
                } else {
                    sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, bool)>(
                        r#"SELECT qq_id, game_server, game_id, created_at, enabled
                           FROM pjsk_accounts WHERE qq_id=$1
                           ORDER BY created_at ASC"#,
                    )
                    .bind(qq)
                    .fetch_all(&pool)
                    .await
                }
                .map_err(|e| StructuredError::internal(e.to_string()))?
                .into_iter()
                .map(map_account)
                .collect();
                let accounts: Vec<Value> = rows.iter().map(account_json).collect();
                Ok(json!({"success": true, "accounts": accounts}))
            }
            "account.list_by_game_id" => {
                let server = param_str(&params, "game_server").to_lowercase();
                let gid = param_str(&params, "game_id");
                let enabled_only = param_bool(&params, "enabled_only", false);
                let rows: Vec<AccountRow> = if enabled_only {
                    sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, bool)>(
                        r#"SELECT qq_id, game_server, game_id, created_at, enabled
                           FROM pjsk_accounts WHERE game_server=$1 AND game_id=$2 AND enabled=TRUE
                           ORDER BY created_at ASC"#,
                    )
                    .bind(&server)
                    .bind(&gid)
                    .fetch_all(&pool)
                    .await
                } else {
                    sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, bool)>(
                        r#"SELECT qq_id, game_server, game_id, created_at, enabled
                           FROM pjsk_accounts WHERE game_server=$1 AND game_id=$2
                           ORDER BY created_at ASC"#,
                    )
                    .bind(&server)
                    .bind(&gid)
                    .fetch_all(&pool)
                    .await
                }
                .map_err(|e| StructuredError::internal(e.to_string()))?
                .into_iter()
                .map(map_account)
                .collect();
                let accounts: Vec<Value> = rows.iter().map(account_json).collect();
                Ok(json!({"success": true, "accounts": accounts}))
            }
            "account.set_enabled" => {
                let qq = param_i64(&params, "qq_id");
                let server = param_str(&params, "game_server").to_lowercase();
                let gid = param_str(&params, "game_id");
                let enabled = param_bool(&params, "enabled", true);
                let res = sqlx::query(
                    r#"UPDATE pjsk_accounts SET enabled=$4
                       WHERE qq_id=$1 AND game_server=$2 AND game_id=$3"#,
                )
                .bind(qq)
                .bind(&server)
                .bind(&gid)
                .bind(enabled)
                .execute(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                if res.rows_affected() == 0 {
                    Ok(json!({"success": false, "message": "账户不存在"}))
                } else {
                    Ok(json!({"success": true, "message": "ok"}))
                }
            }
            "account.remove" => {
                let qq = param_i64(&params, "qq_id");
                let server = param_str(&params, "game_server").to_lowercase();
                let gid = param_str(&params, "game_id");
                let res = sqlx::query(
                    r#"DELETE FROM pjsk_accounts
                       WHERE qq_id=$1 AND game_server=$2 AND game_id=$3"#,
                )
                .bind(qq)
                .bind(&server)
                .bind(&gid)
                .execute(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                if res.rows_affected() == 0 {
                    Ok(json!({"success": false, "message": "账户不存在"}))
                } else {
                    Ok(json!({"success": true, "message": "ok"}))
                }
            }
            "account.get_preferred_server" => {
                let qq = param_i64(&params, "qq_id");
                if qq == 0 {
                    return Ok(json!({"success": false, "message": "QQ号不能为空"}));
                }
                let row: Option<(String,)> = sqlx::query_as(
                    r#"SELECT preferred_server FROM pjsk_user_settings WHERE qq_id=$1"#,
                )
                .bind(qq)
                .fetch_optional(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                match row {
                    Some((server,)) => Ok(json!({"success": true, "server": server})),
                    None => Ok(json!({
                        "success": true,
                        "server": "",
                        "message": "用户未设置默认服务器"
                    })),
                }
            }
            "account.set_preferred_server" => {
                let qq = param_i64(&params, "qq_id");
                let server = param_str(&params, "server").to_lowercase();
                if qq == 0 {
                    return Ok(json!({"success": false, "message": "QQ号不能为空"}));
                }
                if !valid_server(&server) {
                    return Ok(
                        json!({"success": false, "message": "无效的服务器，只支持 jp/cn/en/tw/kr"}),
                    );
                }
                sqlx::query(
                    r#"
                    INSERT INTO pjsk_user_settings (qq_id, preferred_server, updated_at)
                    VALUES ($1, $2, CURRENT_TIMESTAMP)
                    ON CONFLICT (qq_id) DO UPDATE
                      SET preferred_server = $2, updated_at = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(qq)
                .bind(&server)
                .execute(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                Ok(json!({
                    "success": true,
                    "message": format!("默认服务器已设置为 [{}]", server.to_uppercase())
                }))
            }
            _ => Err(StructuredError::not_found("method is not exported")),
        }
    }
    async fn handle(
        &self,
        _: &str,
        _: Value,
        _: Option<CommandMatch>,
        _: &str,
    ) -> Result<HandleResult, StructuredError> {
        Ok(HandleResult::handled())
    }
    async fn status(&self) -> Result<String, StructuredError> {
        Ok(if self.pool.read().await.is_some() {
            "OK".into()
        } else {
            "NO_DB".into()
        })
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        *self.pool.write().await = None;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();
    let host = Arc::new(AsyncRwLock::new(None));
    run_plugin_with_host(
        Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(json!({})),
            pool: AsyncRwLock::new(None),
            default_server: RwLock::new("jp".into()),
        }),
        host,
    )
    .await
}

#[cfg(test)]
mod descriptor_snapshot_tests {
    use super::plugin_descriptor;
    #[test]
    fn descriptor_matches_snapshot() {
        let desc = plugin_descriptor();
        let got = serde_json::to_value(&desc).unwrap();
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots/descriptor.json");
        if std::env::var("UPDATE_SNAPSHOTS").ok().as_deref() == Some("1") {
            std::fs::create_dir_all(path.parent().unwrap()).ok();
            std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap()).unwrap();
            return;
        }
        let raw = std::fs::read_to_string(&path).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(got, expected, "run UPDATE_SNAPSHOTS=1");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn pg_uri() -> Option<String> {
        std::env::var("DATABASE_URL")
            .ok()
            .or_else(|| std::env::var("POSTGRES_URL").ok())
            .filter(|s| !s.trim().is_empty())
    }

    #[tokio::test]
    async fn pjsk_crud_postgres() {
        let Some(uri) = pg_uri() else {
            eprintln!("skip pjsk_crud_postgres: no DATABASE_URL");
            return;
        };
        let host = Arc::new(AsyncRwLock::new(None));
        let plug = Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(json!({})),
            pool: AsyncRwLock::new(None),
            default_server: RwLock::new("jp".into()),
        });
        plug.configure(json!({"database_url": uri, "default_server":"jp"}))
            .await
            .unwrap();
        let qq = 9_001_001_i64;
        let server = "jp";
        let gid = "test-gid-9001001";
        let _ = plug
            .invoke(
                "account.remove",
                json!({"qq_id": qq, "game_server": server, "game_id": gid}),
                "t",
            )
            .await;
        let add = plug
            .invoke(
                "account.add",
                json!({"qq_id": qq, "game_server": server, "game_id": gid}),
                "t",
            )
            .await
            .unwrap();
        assert_eq!(add["success"], true);
        let list = plug
            .invoke("account.list_by_qq", json!({"qq_id": qq}), "t")
            .await
            .unwrap();
        assert_eq!(list["success"], true);
        assert!(
            list["accounts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["game_id"] == gid)
        );
        let pref = plug
            .invoke(
                "account.set_preferred_server",
                json!({"qq_id": qq, "server": "cn"}),
                "t",
            )
            .await
            .unwrap();
        assert_eq!(pref["success"], true);
        let got = plug
            .invoke("account.get_preferred_server", json!({"qq_id": qq}), "t")
            .await
            .unwrap();
        assert_eq!(got["server"], "cn");
        // alias
        let list2 = plug
            .invoke("account.list", json!({"qq_id": qq}), "t")
            .await
            .unwrap();
        assert_eq!(list2["success"], true);
        let _ = plug
            .invoke(
                "account.remove",
                json!({"qq_id": qq, "game_server": server, "game_id": gid}),
                "t",
            )
            .await;
    }
}
