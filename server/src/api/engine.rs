use axum::{
    extract::{Path, Query, State},
    Json,
};
use crate::data::models::{Engine, EngineRequest};
use crate::data::response::ApiResponse;
use crate::service::i18n;

async fn verify_alist_connection(url: &str, token: &str) -> Result<Option<String>, String> {
    let api_url = format!("{}/api/me", url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let resp = client.get(&api_url).header("Authorization", token).send().await
        .map_err(|e| format!("连接验证失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("引擎验证失败，HTTP 状态码: {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("解析响应失败: {}", e))?;
    let code = body.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 200 {
        let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("未知错误");
        return Err(format!("引擎验证失败: {}", msg));
    }
    Ok(body.get("data").and_then(|d| d.get("username")).and_then(|v| v.as_str()).map(|s| s.to_string()))
}

fn clear_snapshots_by_engine(conn: &rusqlite::Connection, engine_id: i64) {
    let _ = conn.execute("DELETE FROM job_source_snapshot WHERE jobId IN (SELECT id FROM job WHERE alistId=?)", [engine_id]);
    let _ = conn.execute("DELETE FROM job_source_snapshot_meta WHERE jobId IN (SELECT id FROM job WHERE alistId=?)", [engine_id]);
}

fn require_rustsync_engine(conn: &rusqlite::Connection, engine_id: i64) -> Result<(), String> {
    let (engine_type, system_key): (Option<String>, Option<String>) = conn
        .query_row("SELECT engineType, systemKey FROM alist_list WHERE id=?", [engine_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|_| "引擎不存在".to_string())?;
    if engine_type.as_deref() != Some("rustsync") || system_key.as_deref() != Some("rustsync") {
        return Err("存储目录只能为 RustSync 引擎管理".to_string());
    }
    Ok(())
}

fn sanitize_storage_mount(mount: serde_json::Value) -> serde_json::Value {
    let mut m = mount;
    let driver_type = m.get("driverType").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let secret_fields: &[&str] = match driver_type.as_str() {
        "sftp" => &["password", "private_key", "private_key_passphrase"],
        "smb" => &["password"],
        "ftp" => &["password"],
        "aliyun" => &["refresh_token", "client_secret", "access_token"],
        _ => &[],
    };
    let mut secret_state = serde_json::Map::new();
    if let Some(config) = m.get_mut("config").and_then(|c| c.as_object_mut()) {
        for field in secret_fields {
            let has_value = config.get(*field).and_then(|v| v.as_str()).map_or(false, |s| !s.is_empty());
            secret_state.insert(field.to_string(), serde_json::Value::Bool(has_value));
            config.insert(field.to_string(), serde_json::Value::String("".to_string()));
        }
    }
    if let Some(obj) = m.as_object_mut() {
        obj.insert("secretState".to_string(), serde_json::Value::Object(secret_state));
    }
    m
}

// ==================== 引擎 CRUD ====================

/// GET /api/engines
pub async fn list_engines(State(state): State<crate::state::SharedState>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    let mut stmt = match conn.prepare(
        "SELECT id, remark, url, userName, engineType, systemKey, protected, createTime FROM alist_list ORDER BY protected DESC, createTime ASC, id ASC"
    ) {
        Ok(s) => s,
        Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
    };
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "alist".to_string()),
            row.get::<_, Option<String>>(5)?, row.get::<_, Option<bool>>(6)?.unwrap_or(false), row.get::<_, i64>(7)?))
    });
    match rows {
        Ok(iter) => {
            let list: Vec<serde_json::Value> = iter.filter_map(|r| r.ok()).map(|(id, remark, url, user_name, engine_type, system_key, protected, create_time)| {
                let is_rustsync = engine_type == "rustsync" && system_key.as_deref() == Some("rustsync");
                let directory_count = if is_rustsync {
                    conn.query_row("SELECT count(*) FROM storage_mount WHERE engineId=?", [id], |row| row.get::<_, i64>(0)).unwrap_or(0)
                } else { 0 };
                serde_json::json!({
                    "id": id, "remark": remark, "url": url, "userName": user_name,
                    "engineType": engine_type, "systemKey": system_key, "protected": protected,
                    "createTime": create_time, "displayName": if is_rustsync { "RustSync" } else { remark.as_deref().unwrap_or(&url) },
                    "directoryCount": directory_count,
                })
            }).collect();
            Json(ApiResponse::ok(serde_json::Value::Array(list)))
        }
        Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
    }
}

/// POST /api/engines
pub async fn add_engine(State(state): State<crate::state::SharedState>, Json(req): Json<EngineRequest>) -> Json<ApiResponse<serde_json::Value>> {
    let mut req = req;
    if req.url.ends_with('/') { req.url = req.url.trim_end_matches('/').to_string(); }
    if req.remark.as_deref().map_or(false, |s| s.trim().is_empty()) { req.remark = None; }
    let is_rustsync = req.engine_type == "rustsync";
    let user_name = if !is_rustsync {
        if req.token.is_none() || req.token.as_deref().map_or(true, |s| s.trim().is_empty()) {
            return Json(ApiResponse::bad_request("令牌不能为空"));
        }
        match verify_alist_connection(&req.url, req.token.as_deref().unwrap_or("")).await {
            Ok(username) => username,
            Err(e) => return Json(ApiResponse::err(&format!("引擎连接验证失败: {}", e))),
        }
    } else { req.user_name.clone() };
    let conn = state.db.get().unwrap();
    match conn.execute(
        "INSERT INTO alist_list (remark, url, userName, token, engineType) VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![req.remark, req.url, user_name, req.token, req.engine_type],
    ) {
        Ok(_) => { let id = conn.last_insert_rowid(); tracing::info!("引擎 {} 添加成功 (id={})", req.url, id); Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_added"))) }
        Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
    }
}

/// PUT /api/engines/:id
pub async fn update_engine(State(state): State<crate::state::SharedState>, Path(id): Path<i64>, Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    let old = match conn.query_row(
        "SELECT id, remark, url, userName, token, engineType, systemKey, protected, createTime FROM alist_list WHERE id=?",
        [id], |row| Ok(Engine { id: row.get(0)?, remark: row.get(1)?, url: row.get(2)?, user_name: row.get(3)?, token: row.get(4)?, engine_type: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "alist".to_string()), system_key: row.get(6)?, protected: row.get::<_, Option<bool>>(7)?.unwrap_or(false), create_time: row.get(8)? }),
    ) { Ok(e) => e, Err(_) => return Json(ApiResponse::not_found("引擎不存在")) };
    if old.protected { return Json(ApiResponse::forbidden(&i18n::t("builtin_engine_protected"))); }

    let mut remark = body.get("remark").and_then(|v| v.as_str()).map(|s| s.to_string());
    let mut url = body.get("url").and_then(|v| v.as_str()).unwrap_or(&old.url).to_string();
    let user_name = body.get("userName").and_then(|v| v.as_str()).map(|s| s.to_string());
    let has_token = body.get("token").is_some();
    let mut token = body.get("token").and_then(|v| v.as_str()).map(|s| s.to_string());
    let engine_type = body.get("engineType").and_then(|v| v.as_str()).unwrap_or(&old.engine_type);
    if url.ends_with('/') { url = url.trim_end_matches('/').to_string(); }
    if remark.as_deref().map_or(false, |s| s.trim().is_empty()) { remark = None; }
    if token.as_deref().map_or(false, |s| s.trim().is_empty()) { token = None; }

    let connection_changed = old.url != url || has_token;
    if connection_changed {
        if !has_token || token.is_none() { return Json(ApiResponse::bad_request(&i18n::t("without_token"))); }
        if engine_type != "rustsync" {
            if let Err(e) = verify_alist_connection(&url, token.as_deref().unwrap_or("")).await {
                return Json(ApiResponse::err(&format!("引擎连接验证失败: {}", e)));
            }
        }
    }
    let final_token = if has_token { token } else { old.token.clone() };
    drop(conn);
    let conn = state.db.get().unwrap();
    match conn.execute("UPDATE alist_list SET remark=?, url=?, userName=?, token=?, engineType=? WHERE id=?", rusqlite::params![remark, url, user_name, final_token, engine_type, id]) {
        Ok(_) => { if connection_changed { clear_snapshots_by_engine(&conn, id); } Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_updated"))) }
        Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
    }
}

/// DELETE /api/engines/:id
pub async fn delete_engine(State(state): State<crate::state::SharedState>, Path(id): Path<i64>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    let protected: bool = conn.query_row("SELECT protected FROM alist_list WHERE id=?", [id], |row| row.get(0)).unwrap_or(false);
    if protected { return Json(ApiResponse::forbidden(&i18n::t("builtin_engine_protected"))); }
    clear_snapshots_by_engine(&conn, id);
    match conn.execute("DELETE FROM alist_list WHERE id=? AND protected=0", [id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}

/// GET /api/engines/:id/browse
pub async fn browse_engine(State(state): State<crate::state::SharedState>, Path(id): Path<i64>, Query(params): Query<std::collections::HashMap<String, String>>) -> Json<ApiResponse<serde_json::Value>> {
    let path = params.get("path").cloned().unwrap_or_default();

    let (engine, _mount_children, mount_config) = {
        let conn = state.db.get().unwrap();
        let engine = match conn.query_row(
            "SELECT id, remark, url, userName, token, engineType, systemKey, protected, createTime FROM alist_list WHERE id=?",
            [id], |row| Ok(Engine { id: row.get(0)?, remark: row.get(1)?, url: row.get(2)?, user_name: row.get(3)?, token: row.get(4)?, engine_type: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "alist".to_string()), system_key: row.get(6)?, protected: row.get::<_, Option<bool>>(7)?.unwrap_or(false), create_time: row.get(8)? }),
        ) { Ok(e) => e, Err(_) => return Json(ApiResponse::not_found("引擎不存在")) };

        let mut stmt = match conn.prepare("SELECT name FROM storage_mount WHERE engineId=? AND enabled=1 ORDER BY name") {
            Ok(s) => s, Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };
        let children: Vec<serde_json::Value> = match stmt.query_map([id], |row| {
            let name = row.get::<_, String>(0)?;
            Ok(serde_json::json!({"path": name}))
        }) { Ok(iter) => iter.filter_map(|r| r.ok()).collect(), Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))) };

        // Root path: return mount names directly
        if path.is_empty() || path == "/" {
            return Json(ApiResponse::ok(serde_json::json!({"engine": engine, "child": children, "path": path})));
        }

        // Non-root path: resolve mount name to get filesystem base path
        let path_trimmed = path.trim_start_matches('/');
        let mount_name = path_trimmed.split('/').next().unwrap_or("");
        let config_str = match conn.query_row(
            "SELECT config FROM storage_mount WHERE engineId=? AND name=? AND enabled=1",
            rusqlite::params![id, mount_name],
            |row| row.get::<_, String>(0),
        ) {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::not_found("挂载目录不存在")),
        };
        (engine, children, Some(config_str))
    };

    // Resolve filesystem path from mount config
    let config_str = mount_config.unwrap();
    let config: serde_json::Value = match serde_json::from_str(&config_str) {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("挂载配置解析失败")),
    };
    let base_path = match config.get("path").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(ApiResponse::err("挂载路径配置缺失")),
    };

    let path_trimmed = path.trim_start_matches('/');
    let relative: String = {
        let parts: Vec<&str> = path_trimmed.splitn(2, '/').collect();
        if parts.len() > 1 { format!("/{}", parts[1]) } else { "/".to_string() }
    };
    let full_path = if relative == "/" {
        base_path
    } else {
        format!("{}{}", base_path.trim_end_matches('/'), relative)
    };

    let result = tokio::task::spawn_blocking(move || {
        let dir_path = std::path::Path::new(&full_path);
        if !dir_path.is_dir() {
            return Err("目录不存在".to_string());
        }
        let entries = match std::fs::read_dir(dir_path) {
            Ok(e) => e,
            Err(e) => return Err(format!("读取目录失败: {}", e)),
        };
        let mut children: Vec<serde_json::Value> = Vec::new();
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if !entry_path.is_dir() { continue; }
            let name = entry.file_name().to_string_lossy().to_string();
            children.push(serde_json::json!({"path": name}));
        }
        children.sort_by(|a, b| a["path"].as_str().unwrap_or("").to_lowercase().cmp(&b["path"].as_str().unwrap_or("").to_lowercase()));
        Ok(children)
    }).await;

    match result {
        Ok(Ok(children)) => Json(ApiResponse::ok(serde_json::json!({"engine": engine, "child": children, "path": path}))),
        Ok(Err(e)) => Json(ApiResponse::err(&e)),
        Err(e) => Json(ApiResponse::err(&format!("任务失败: {}", e))),
    }
}

// ==================== 存储挂载 ====================

fn filesystem_roots() -> Vec<serde_json::Value> {
    let mut roots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut add = |name: &str, path: &str| {
        let canonical = std::fs::canonicalize(path).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| path.to_string());
        if seen.contains(&canonical) { return; }
        seen.insert(canonical.clone());
        roots.push(serde_json::json!({"name": name, "path": canonical}));
    };
    #[cfg(target_os = "windows")] {
        for letter in 'A'..='Z' {
let path = format!("{}:\\", letter);
            if std::path::Path::new(&path).is_dir() { add(&format!("{}:", letter), &path); }
        }
    }
    #[cfg(not(target_os = "windows"))] {
        add("/", "/");
        if let Ok(home) = std::env::var("HOME") {
            if let Ok(canonical_home) = std::fs::canonicalize(&home) {
                let home_str = canonical_home.to_string_lossy().to_string();
                let root_str = std::fs::canonicalize("/").map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "/".to_string());
                if home_str != root_str { add("home", &home_str); }
            }
        }
        if let Ok(cwd) = std::env::current_dir() { let cwd_str = cwd.to_string_lossy().to_string(); add("cwd", &cwd_str); }
    }
    roots
}

/// GET /api/storage
pub async fn list_storage(State(state): State<crate::state::SharedState>, Query(params): Query<std::collections::HashMap<String, String>>) -> Json<ApiResponse<serde_json::Value>> {
    let engine_id: i64 = params.get("engineId").and_then(|s| s.parse().ok()).unwrap_or(0);
    let conn = state.db.get().unwrap();
    if let Err(e) = require_rustsync_engine(&conn, engine_id) { return Json(ApiResponse::bad_request(&e)); }
    let mut stmt = match conn.prepare(
        "SELECT id, engineId, name, driverType, config, enabled, configVersion, authVersion, createTime FROM storage_mount WHERE engineId=? AND enabled=1 ORDER BY createTime ASC, id ASC"
    ) { Ok(s) => s, Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))) };
    let rows = stmt.query_map([engine_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, i64>(0)?, "engineId": row.get::<_, i64>(1)?, "name": row.get::<_, String>(2)?,
            "driverType": row.get::<_, String>(3)?, "config": row.get::<_, String>(4)?,
            "enabled": row.get::<_, i32>(5)? != 0, "configVersion": row.get::<_, i32>(6)?.max(1),
            "authVersion": row.get::<_, i32>(7)?.max(1), "createTime": row.get::<_, i64>(8)?,
        }))
    });
    match rows {
        Ok(iter) => {
            let list: Vec<serde_json::Value> = iter.filter_map(|r| r.ok()).map(|mut item| {
                if let Some(config_str) = item.get("config").and_then(|v| v.as_str()) {
                    if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(config_str) {
                        if let Some(obj) = item.as_object_mut() { obj.insert("config".to_string(), config_json); }
                    }
                }
                sanitize_storage_mount(item)
            }).collect();
            Json(ApiResponse::ok(serde_json::json!(list)))
        }
        Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
    }
}

/// POST /api/storage
pub async fn add_storage(State(state): State<crate::state::SharedState>, Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let engine_id = body.get("engineId").and_then(|v| v.as_i64()).unwrap_or(0);
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let driver_type = body.get("driverType").and_then(|v| v.as_str()).unwrap_or("");
    let config = body.get("config").cloned().unwrap_or_default();
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let config_str = serde_json::to_string(&config).unwrap_or_default();
    let conn = state.db.get().unwrap();
    if let Err(e) = require_rustsync_engine(&conn, engine_id) { return Json(ApiResponse::bad_request(&e)); }
    match conn.execute("INSERT INTO storage_mount (engineId, name, driverType, config, enabled) VALUES (?, ?, ?, ?, ?)", rusqlite::params![engine_id, name, driver_type, config_str, enabled as i32]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("mount_added"))),
        Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
    }
}

/// PUT /api/storage/:id
pub async fn update_storage(State(state): State<crate::state::SharedState>, Path(id): Path<i64>, Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let driver_type = body.get("driverType").and_then(|v| v.as_str()).unwrap_or("");
    let config = body.get("config").cloned().unwrap_or_default();
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let config_str = serde_json::to_string(&config).unwrap_or_default();
    let conn = state.db.get().unwrap();
    match conn.execute("UPDATE storage_mount SET name=?, driverType=?, config=?, enabled=?, configVersion=configVersion+1 WHERE id=?", rusqlite::params![name, driver_type, config_str, enabled as i32, id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("mount_updated"))),
        Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
    }
}

/// DELETE /api/storage/:id
pub async fn delete_storage(State(state): State<crate::state::SharedState>, Path(id): Path<i64>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    match conn.execute("DELETE FROM storage_mount WHERE id=?", [id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("mount_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}

/// GET /api/storage/local-browse
pub async fn local_browse(Query(params): Query<std::collections::HashMap<String, String>>) -> Json<ApiResponse<serde_json::Value>> {
    let path = params.get("path").map(|s| s.to_string());
    let result = tokio::task::spawn_blocking(move || {
        let current = if let Some(ref p) = path {
            if p.is_empty() { std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "/".to_string()) } else { p.clone() }
        } else { std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "/".to_string()) };
        let current = std::fs::canonicalize(std::path::Path::new(&current)).map(|p| p.to_string_lossy().to_string()).unwrap_or(current);
        if !std::path::Path::new(&current).is_dir() { return Err("local browse path must be an existing directory".to_string()); }
        let parent = std::path::Path::new(&current).parent().and_then(|p| {
            let resolved = std::fs::canonicalize(p).map(|cp| cp.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string());
            if resolved == current { None } else { Some(resolved) }
        });
        let roots = filesystem_roots();
        let mut directories: Vec<serde_json::Value> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_symlink() || !entry_path.is_dir() { continue; }
                let entry_name = entry.file_name().to_string_lossy().to_string();
                let resolved = std::fs::canonicalize(&entry_path).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| entry_path.to_string_lossy().to_string());
                directories.push(serde_json::json!({"name": entry_name, "path": resolved}));
            }
        }
        directories.sort_by(|a, b| a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase()));
        Ok(serde_json::json!({"path": current, "parent": parent, "roots": roots, "directories": directories}))
    }).await;
    match result { Ok(Ok(data)) => Json(ApiResponse::ok(data)), Ok(Err(e)) => Json(ApiResponse::err(&e)), Err(e) => Json(ApiResponse::err(&format!("任务失败: {}", e))) }
}

/// GET /api/storage/smb-discover
pub async fn smb_discover() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(serde_json::json!([])))
}

/// GET /api/storage/types
pub async fn storage_types() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(serde_json::json!(["local", "sftp", "smb", "ftp", "aliyun"])))
}

/// POST /api/storage/sftp-test
pub async fn sftp_test(Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let host = body.get("host").and_then(|v| v.as_str()).unwrap_or("");
    let port = body.get("port").and_then(|v| v.as_u64()).unwrap_or(22) as u16;
    match tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await {
        Ok(_) => Json(ApiResponse::ok(serde_json::json!({"connected": true, "message": "SSH 服务器可连接"}))),
        Err(e) => Json(ApiResponse::err(&format!("连接失败: {}", e))),
    }
}

/// POST /api/storage/sftp-browse
pub async fn sftp_browse(Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let path = body.get("path").and_then(|v| v.as_str()).unwrap_or("/");
    Json(ApiResponse::ok(serde_json::json!({"path": path, "files": []})))
}