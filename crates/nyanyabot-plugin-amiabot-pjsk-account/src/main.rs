use async_trait::async_trait;
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
}

fn plugin_descriptor() -> Descriptor {
    Descriptor {
        name: "Amiabot PJSK Account".into(),
        plugin_id: "external.amiabot-pjsk-account".into(),
        version: "0.1.0".into(),
        author: "amiabot".into(),
        description: "PJSK 账户存储".into(),
        exports: vec![
            ExportSpec {
                name: "account.add".into(),
                description: "添加账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.get".into(),
                description: "获取账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.list".into(),
                description: "列出账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.set_enabled".into(),
                description: "启用/禁用".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
            ExportSpec {
                name: "account.remove".into(),
                description: "删除账户".into(),
                params_schema: json!({"type":"object"}),
                result_schema: json!({"type":"object"}),
            },
        ],
        config: Some(ConfigSpec {
            version: Some("1".into()),
            description: Some("config".into()),
            schema: Some(json!({"type":"object"})),
            default: Some(json!({"database_url":"","default_server":"jp"})),
        }),
        ..Default::default()
    }
}

#[async_trait]
impl Plugin for Plug {
    async fn descriptor(&self) -> Result<Descriptor, StructuredError> {
        Ok(plugin_descriptor())
    }
    async fn configure(&self, config: Value) -> Result<(), StructuredError> {
        *self.cfg.write() = config.clone();
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
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pjsk_accounts (
                qq_id BIGINT NOT NULL,
                game_server TEXT NOT NULL,
                game_id TEXT NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                PRIMARY KEY (qq_id, game_server, game_id)
            )
        "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pjsk_user_settings (
                qq_id BIGINT PRIMARY KEY,
                default_server TEXT
            )
        "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| StructuredError::internal(e.to_string()))?;
        *self.pool.write().await = Some(pool);
        Ok(())
    }
    async fn invoke(
        &self,
        method: &str,
        params: Value,
        _caller: &str,
    ) -> Result<Value, StructuredError> {
        let pool = self
            .pool
            .read()
            .await
            .clone()
            .ok_or_else(|| StructuredError::internal("database_url not configured"))?;
        match method {
            "account.add" => {
                let qq = params.get("qq_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let server = params
                    .get("game_server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let gid = params
                    .get("game_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if qq == 0 || server.is_empty() || gid.is_empty() {
                    return Err(StructuredError::invalid_params(
                        "qq_id/game_server/game_id required",
                    ));
                }
                sqlx::query("INSERT INTO pjsk_accounts(qq_id, game_server, game_id, enabled) VALUES($1,$2,$3,TRUE) ON CONFLICT DO NOTHING")
                    .bind(qq).bind(&server).bind(&gid).execute(&pool).await
                    .map_err(|e| StructuredError::internal(e.to_string()))?;
                Ok(json!({"ok": true}))
            }
            "account.list" => {
                let qq = params.get("qq_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let rows = sqlx::query_as::<_, (String, String, bool)>(
                    "SELECT game_server, game_id, enabled FROM pjsk_accounts WHERE qq_id=$1",
                )
                .bind(qq)
                .fetch_all(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                let items: Vec<Value> = rows
                    .into_iter()
                    .map(|(s, g, e)| json!({"game_server":s, "game_id":g, "enabled":e}))
                    .collect();
                Ok(json!({"accounts": items}))
            }
            "account.get" => {
                let qq = params.get("qq_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let server = params
                    .get("game_server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let row = sqlx::query_as::<_, (String, bool)>("SELECT game_id, enabled FROM pjsk_accounts WHERE qq_id=$1 AND game_server=$2 AND enabled=TRUE LIMIT 1")
                    .bind(qq).bind(server).fetch_optional(&pool).await.map_err(|e| StructuredError::internal(e.to_string()))?;
                match row {
                    Some((gid, enabled)) => {
                        Ok(json!({"game_id": gid, "enabled": enabled, "game_server": server}))
                    }
                    None => Err(StructuredError::not_found("account not found")),
                }
            }
            "account.set_enabled" => {
                let qq = params.get("qq_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let server = params
                    .get("game_server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let gid = params.get("game_id").and_then(|v| v.as_str()).unwrap_or("");
                let enabled = params
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                sqlx::query("UPDATE pjsk_accounts SET enabled=$1 WHERE qq_id=$2 AND game_server=$3 AND game_id=$4")
                    .bind(enabled).bind(qq).bind(server).bind(gid).execute(&pool).await
                    .map_err(|e| StructuredError::internal(e.to_string()))?;
                Ok(json!({"ok": true}))
            }
            "account.remove" => {
                let qq = params.get("qq_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let server = params
                    .get("game_server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let gid = params.get("game_id").and_then(|v| v.as_str()).unwrap_or("");
                sqlx::query(
                    "DELETE FROM pjsk_accounts WHERE qq_id=$1 AND game_server=$2 AND game_id=$3",
                )
                .bind(qq)
                .bind(server)
                .bind(gid)
                .execute(&pool)
                .await
                .map_err(|e| StructuredError::internal(e.to_string()))?;
                Ok(json!({"ok": true}))
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
        Ok(HandleResult {})
    }
    async fn status(&self) -> Result<String, StructuredError> {
        Ok("OK".into())
    }
    async fn shutdown(&self) -> Result<(), StructuredError> {
        *self.pool.write().await = None;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();
    let host = Arc::new(AsyncRwLock::new(None));
    run_plugin_with_host(
        Arc::new(Plug {
            host: host.clone(),
            cfg: RwLock::new(json!({})),
            pool: AsyncRwLock::new(None),
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
        let mut d = plugin_descriptor();
        nyanyabot_proto::ensure_descriptor_arrays(&mut d);
        let actual = serde_json::to_string_pretty(&d).expect("serialize descriptor");
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("snapshots/descriptor.json");
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("{actual}\n")).unwrap();
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing descriptor snapshot at {}: {e}", path.display()));
        assert_eq!(
            actual.trim(),
            expected.trim(),
            "descriptor snapshot mismatch; run with UPDATE_SNAPSHOTS=1"
        );
    }
}

#[cfg(test)]
mod pg_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock as AsyncRwLock;

    fn pg_uri() -> String {
        std::env::var("NYANYABOT_TEST_DATABASE_URI")
            .unwrap_or_else(|_| "postgres://amiabot:amiabot_password@127.0.0.1:5432/amiabot".into())
    }

    #[tokio::test]
    async fn pjsk_crud_postgres() {
        let uri = pg_uri();
        let ok = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect(&uri)
            .await
            .is_ok();
        if !ok {
            eprintln!("skip pjsk_crud_postgres: no db");
            return;
        }
        let host = Arc::new(AsyncRwLock::new(None));
        let plug = Plug {
            host,
            cfg: RwLock::new(json!({"database_url": uri})),
            pool: AsyncRwLock::new(None),
        };
        plug.configure(json!({"database_url": pg_uri()}))
            .await
            .unwrap();
        let qq = 9_001_001_i64;
        // cleanup
        let _ = plug
            .invoke(
                "account.remove",
                json!({"qq_id": qq, "game_server": "jp", "game_id": "g-test"}),
                "t",
            )
            .await;
        plug.invoke(
            "account.add",
            json!({"qq_id": qq, "game_server": "jp", "game_id": "g-test"}),
            "t",
        )
        .await
        .unwrap();
        let listed = plug
            .invoke("account.list", json!({"qq_id": qq}), "t")
            .await
            .unwrap();
        assert!(
            listed
                .get("accounts")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false),
            "{listed}"
        );
        let got = plug
            .invoke(
                "account.get",
                json!({"qq_id": qq, "game_server": "jp"}),
                "t",
            )
            .await
            .unwrap();
        assert_eq!(got.get("game_id").and_then(|v| v.as_str()), Some("g-test"));
        plug.invoke(
            "account.set_enabled",
            json!({"qq_id": qq, "game_server": "jp", "game_id": "g-test", "enabled": false}),
            "t",
        )
        .await
        .unwrap();
        let err = plug
            .invoke(
                "account.get",
                json!({"qq_id": qq, "game_server": "jp"}),
                "t",
            )
            .await
            .expect_err("disabled hidden");
        assert_eq!(err.code.as_str(), "NOT_FOUND");
        plug.invoke(
            "account.remove",
            json!({"qq_id": qq, "game_server": "jp", "game_id": "g-test"}),
            "t",
        )
        .await
        .unwrap();
    }
}
