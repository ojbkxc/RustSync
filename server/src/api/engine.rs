use axum::{
    extract::{Path, Query, State},
    Json,
};
use crate::data::models::Engine;
use crate::data::response::ApiResponse;
use crate::service::db::now_ts;
use crate::service::i18n;

/// GET /api/engines
pub async fn list_engines(State(state): State<crate::state::SharedState>) -> Json<ApiResponse<Vec<Engine>>> {
    let conn = state.db.get().unwrap();
    let mut stmt = match conn.prepare("SELECT id, name, remark, createTime FROM engine ORDER BY id") {
        Ok(s) => s, Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
    };
    let rows = stmt.query_map([], |row| Ok(Engine {
        id: row.get(0)?, name: row.get(1)?, remark: row.get(2)?, create_time: row.get(3)?,
    }));
    match rows { Ok(iter) => { let list: Vec<Engine> = iter.filter_map(|r| r.ok()).collect(); Json(ApiResponse::ok(list)) } Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))) }
}

/// POST /api/engines
pub async fn add_engine(State(state): State<crate::state::SharedState>, Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let remark = body.get("remark").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.get().unwrap();
    match conn.execute("INSERT INTO engine (name, remark) VALUES (?, ?)", rusqlite::params![name, remark]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_added"))),
        Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
    }
}

/// PUT /api/engines/:id
pub async fn update_engine(State(state): State<crate::state::SharedState>, Path(id): Path<i64>, Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let remark = body.get("remark").and_then(|v| v.as_str()).unwrap_or("");
    let conn = state.db.get().unwrap();
    match conn.execute("UPDATE engine SET name=?, remark=? WHERE id=?", rusqlite::params![name, remark, id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_updated"))),
        Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
    }
}

/// DELETE /api/engines/:id
pub async fn delete_engine(State(state): State<crate::state::SharedState>, Path(id): Path<i64>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    match conn.execute("DELETE FROM engine WHERE id=?", [id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}

/// GET /api/engines/:id/browse
pub async fn browse_engine(State(state): State<crate::state::SharedState>, Path(id): Path<i64>, Query(params): Query<std::collections::HashMap<String, String>>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    let mount = match conn.query_row(
        "SELECT id, engineId, name, driverType, config, enabled FROM storage_mount WHERE id=?",
        [id],
        |row| Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i32>(5)?,
        )),
    ) {
        Ok(m) => m,
        Err(_) => return Json(ApiResponse::err("挂载点不存在")),
    };
    let (_mount_id, _engine_id, _name, driver_type, config_str, _enabled) = mount;
    let config: serde_json::Value = match serde_json::from_str(&config_str) {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("挂载配置解析失败")),
    };
    let base_path = match config.get("root_path").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(ApiResponse::err("挂载路径配置缺失")),
    };
    let current_path = params.get("path").map(|s| s.to_string()).unwrap_or_default();
    let full_path = if current_path.is_empty() {
        base_path.clone()
    } else {
        let sep = if cfg!(windows) { "\\" } else { "/" };
        format!("{}{}{}", base_path.trim_end_matches(&['/', '\\'][..]), sep, current_path.trim_start_matches(&['/', '\\'][..]))
    };
    let path = std::path::Path::new(&full_path);
    let mut children = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') { continue; }
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let child_path = if current_path.is_empty() {
                file_name.clone()
            } else {
                format!("{}/{}", current_path, file_name)
            };
            children.push(serde_json::json!({
                "name": file_name,
                "path": child_path,
                "isLeaf": !is_dir,
                "child": if is_dir { [] } else { [] }
            }));
        }
    }
    children.sort_by(|a, b| {
        let a_leaf = a["isLeaf"].as_bool().unwrap_or(false);
        let b_leaf = b["isLeaf"].as_bool().unwrap_or(false);
        a_leaf.cmp(&b_leaf).then_with(|| a["name"].as_str().cmp(&b["name"].as_str()))
    });
    Json(ApiResponse::ok(serde_json::json!({
        "child": children,
        "current": {
            "name": if current_path.is_empty() { base_path.clone() } else { current_path.clone() },
            "path": current_path
        }
    })))
}

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
            if !home.is_empty() { add(&home, &home); }
        }
        if let Ok(entries) = std::fs::read_dir("/mnt") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let path_str = path.to_string_lossy().to_string();
                    add(&name, &path_str);
                }
            }
        }
    }
    roots
}

/// GET /api/storage
pub async fn list_storage(State(state): State<crate::state::SharedState>) -> Json<ApiResponse<serde_json::Value>> {
    let conn = state.db.get().unwrap();
    let mut stmt = match conn.prepare("SELECT id, engineId, name, driverType, config, enabled, configVersion, createTime FROM storage_mount ORDER BY id") {
        Ok(s) => s, Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
    };
    let rows = stmt.query_map([], |row| Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "engineId": row.get::<_, i64>(1)?,
        "name": row.get::<_, String>(2)?,
        "driverType": row.get::<_, String>(3)?,
        "config": row.get::<_, String>(4)?,
        "enabled": row.get::<_, i32>(5)?,
        "configVersion": row.get::<_, i64>(6)?,
        "createTime": row.get::<_, i64>(7)?,
    })));
    match rows {
        Ok(iter) => {
            let list: Vec<serde_json::Value> = iter.filter_map(|r| r.ok()).map(|mut item| {
                if let Some(config_str) = item.get("config").and_then(|v| v.as_str()) {
                    if let Ok(config) = serde_json::from_str::<serde_json::Value>(config_str) {
                        item["config"] = config;
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
    let enabled = body.get("enabled").and_then(|v| crate::data::json_bool(v)).unwrap_or(true);
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
    let enabled = body.get("enabled").and_then(|v| crate::data::json_bool(v)).unwrap_or(true);
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
    let roots = if path.is_none() || path.as_deref() == Some("") {
        filesystem_roots()
    } else {
        let p = std::path::Path::new(path.as_deref().unwrap());
        let mut children = Vec::new();
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with('.') { continue; }
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                if !is_dir { continue; }
                let child_path = entry.path().to_string_lossy().to_string();
                children.push(serde_json::json!({
                    "name": file_name,
                    "path": child_path
                }));
            }
        }
        children.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        children
    };
    Json(ApiResponse::ok(serde_json::json!(roots)))
}

/// GET /api/storage/smb-discover
pub async fn smb_discover(Query(params): Query<std::collections::HashMap<String, String>>) -> Json<ApiResponse<serde_json::Value>> {
    let host = params.get("host").map(|s| s.to_string()).unwrap_or_default();
    if host.is_empty() {
        return Json(ApiResponse::bad_request("缺少 host 参数"));
    }
    let shares = crate::driver::smb::SmbDriver::discover_shares(&host);
    Json(ApiResponse::ok(serde_json::json!(shares)))
}

/// GET /api/storage/types
pub async fn storage_types() -> Json<ApiResponse<serde_json::Value>> {
    let types = vec![
        serde_json::json!({"value": "local", "label": "本地存储"}),
        serde_json::json!({"value": "sftp", "label": "SFTP"}),
        serde_json::json!({"value": "ftp", "label": "FTP"}),
        serde_json::json!({"value": "smb", "label": "SMB/CIFS"}),
        serde_json::json!({"value": "aliyun", "label": "阿里云盘"}),
    ];
    Json(ApiResponse::ok(serde_json::json!(types)))
}

/// POST /api/storage/sftp-test
pub async fn sftp_test(Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let config = body.get("config").cloned().unwrap_or_default();
    match crate::driver::sftp::SftpDriver::test_connection(&config) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), "SFTP 连接测试成功")),
        Err(e) => Json(ApiResponse::err(&format!("SFTP 连接测试失败: {}", e))),
    }
}

/// POST /api/storage/sftp-browse
pub async fn sftp_browse(Json(body): Json<serde_json::Value>) -> Json<ApiResponse<serde_json::Value>> {
    let config = body.get("config").cloned().unwrap_or_default();
    let path = body.get("path").and_then(|v| v.as_str()).unwrap_or("/").to_string();
    match crate::driver::sftp::SftpDriver::browse(&config, &path) {
        Ok(entries) => Json(ApiResponse::ok(serde_json::json!(entries))),
        Err(e) => Json(ApiResponse::err(&format!("SFTP 浏览失败: {}", e))),
    }
}

fn require_rustsync_engine(conn: &rusqlite::Connection, engine_id: i64) -> Result<(), String> {
    let name: String = conn.query_row("SELECT name FROM engine WHERE id=?", [engine_id], |row| row.get(0))
        .map_err(|_| "引擎不存在".to_string())?;
    if name != "RustSync" {
        return Err("不允许在非 RustSync 引擎下添加挂载".to_string());
    }
    Ok(())
}

fn sanitize_storage_mount(mut item: serde_json::Value) -> serde_json::Value {
    if let Some(config) = item.get("config") {
        if let Some(obj) = config.as_object() {
            let mut cleaned = serde_json::Map::new();
            for (k, v) in obj {
                if k == "password" || k == "client_secret" || k == "refresh_token" {
                    cleaned.insert(k.clone(), serde_json::Value::String("***".to_string()));
                } else {
                    cleaned.insert(k.clone(), v.clone());
                }
            }
            item["config"] = serde_json::Value::Object(cleaned);
        }
    }
    item
}
