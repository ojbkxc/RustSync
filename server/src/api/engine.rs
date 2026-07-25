use axum::{
    extract::State,
    Json,
};
use crate::data::models::{Engine, EngineRequest};
use crate::data::response::ApiResponse;
use crate::service::i18n;

/// 验证 AList 引擎连接并获取用户名 - 与 Python AlistClient 一致
async fn verify_alist_connection(url: &str, token: &str) -> Result<Option<String>, String> {
    let api_url = format!("{}/api/me", url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(&api_url)
        .header("Authorization", token)
        .send()
        .await
        .map_err(|e| format!("连接验证失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("引擎验证失败，HTTP 状态码: {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let code = body.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 200 {
        let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("未知错误");
        return Err(format!("引擎验证失败: {}", msg));
    }

    let username = body
        .get("data")
        .and_then(|d| d.get("username"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(username)
}

/// 清除引擎关联的 source snapshot - 与 Python clearSourceSnapshotsByEngine 一致
fn clear_snapshots_by_engine(db: &std::sync::MutexGuard<rusqlite::Connection>, engine_id: i64) {
    let _ = db.execute(
        "DELETE FROM job_source_snapshot WHERE jobId IN (SELECT id FROM job WHERE alistId=?)",
        [engine_id],
    );
    let _ = db.execute(
        "DELETE FROM job_source_snapshot_meta WHERE jobId IN (SELECT id FROM job WHERE alistId=?)",
        [engine_id],
    );
}

/// 检查是否为 RustSync 引擎 - 与 Python _requireRustSync 一致
fn require_rustsync_engine(db: &std::sync::MutexGuard<rusqlite::Connection>, engine_id: i64) -> Result<(), String> {
    let (engine_type, system_key): (Option<String>, Option<String>) = db
        .query_row(
            "SELECT engineType, systemKey FROM alist_list WHERE id=?",
            [engine_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "引擎不存在".to_string())?;

    if engine_type.as_deref() != Some("rustsync") || system_key.as_deref() != Some("rustsync") {
        return Err("存储目录只能为 RustSync 引擎管理".to_string());
    }
    Ok(())
}

/// 存储挂载密钥脱敏 - 与 Python _sanitized 一致
fn sanitize_storage_mount(mount: serde_json::Value) -> serde_json::Value {
    let mut m = mount;
    let driver_type = m.get("driverType").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // 密钥字段列表
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

/// GET /svr/alist - 获取引擎列表或子路径
/// 前端调用: alistGet() 或 alistGetPath(alistId, path)
pub async fn alist_get(
    State(state): State<crate::state::SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = state.db.lock().unwrap();

    // 如果有 alistId 和 path，查询子路径
    if let (Some(alist_id_str), Some(path)) = (params.get("alistId"), params.get("path")) {
        let alist_id: i64 = alist_id_str.parse().unwrap_or(0);
        // 获取引擎信息
        let engine = db.query_row(
            "SELECT id, remark, url, userName, token, engineType, systemKey, protected, createTime
             FROM alist_list WHERE id=?",
            [alist_id],
            |row| {
                Ok(Engine {
                    id: row.get(0)?,
                    remark: row.get(1)?,
                    url: row.get(2)?,
                    user_name: row.get(3)?,
                    token: row.get(4)?,
                    engine_type: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "alist".to_string()),
                    system_key: row.get(6)?,
                    protected: row.get::<_, Option<bool>>(7)?.unwrap_or(false),
                    create_time: row.get(8)?,
                })
            },
        );
        match engine {
            Ok(engine) => {
                // 列出引擎下的挂载目录作为子路径
                let mut stmt = match db.prepare(
                    "SELECT name FROM storage_mount WHERE engineId=? AND enabled=1 ORDER BY name"
                ) {
                    Ok(s) => s,
                    Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
                };
                let children: Vec<serde_json::Value> = match stmt.query_map([alist_id], |row| {
                    Ok(serde_json::json!({
                        "name": row.get::<_, String>(0)?,
                        "path": format!("{}/{}", path, row.get::<_, String>(0)?),
                        "isDir": true
                    }))
                }) {
                    Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
                    Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
                };
                return Json(ApiResponse::ok(serde_json::json!({
                    "engine": engine,
                    "children": children,
                    "path": path
                })));
            }
            Err(_) => Json(ApiResponse::err("引擎不存在")),
        }
    } else {
        // 返回引擎列表（与 Python getClientList 一致：无分页，返回数组，移除 token，添加 displayName/directoryCount）
        let mut stmt = match db.prepare(
            "SELECT id, remark, url, userName, engineType, systemKey, protected, createTime
             FROM alist_list ORDER BY protected DESC, createTime ASC, id ASC",
        ) {
            Ok(s) => s,
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "alist".to_string()),
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<bool>>(6)?.unwrap_or(false),
                row.get::<_, i64>(7)?,
            ))
        });

        match rows {
            Ok(iter) => {
                let list: Vec<serde_json::Value> = iter.filter_map(|r| r.ok()).map(|(id, remark, url, user_name, engine_type, system_key, protected, create_time)| {
                    let is_rustsync = engine_type == "rustsync" && system_key.as_deref() == Some("rustsync");
                    // 计算挂载目录数量（仅 rustsync 引擎）
                    let directory_count = if is_rustsync {
                        db.query_row(
                            "SELECT count(*) FROM storage_mount WHERE engineId=?",
                            [id],
                            |row| row.get::<_, i64>(0),
                        ).unwrap_or(0)
                    } else {
                        0
                    };
                    let display_name = if is_rustsync {
                        "RustSync".to_string()
                    } else {
                        remark.clone().unwrap_or_else(|| url.clone())
                    };
                    serde_json::json!({
                        "id": id,
                        "remark": remark,
                        "url": url,
                        "userName": user_name,
                        "engineType": engine_type,
                        "systemKey": system_key,
                        "protected": protected,
                        "createTime": create_time,
                        "displayName": display_name,
                        "directoryCount": directory_count,
                    })
                }).collect();
                Json(ApiResponse::ok(serde_json::Value::Array(list)))
            }
            Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
        }
    }
}

/// POST /svr/alist - 添加引擎
/// 与 Python addClient 一致：验证连接、获取 userName
pub async fn alist_post(
    State(state): State<crate::state::SharedState>,
    Json(mut req): Json<EngineRequest>,
) -> Json<ApiResponse<serde_json::Value>> {
    // URL 标准化：去除尾部斜杠（与 Python addClient 一致）
    if req.url.ends_with('/') {
        req.url = req.url.trim_end_matches('/').to_string();
    }
    // remark 空字符串转 None
    if req.remark.as_deref().map_or(false, |s| s.trim().is_empty()) {
        req.remark = None;
    }

    // 对外部 AList 引擎进行连接验证（与 Python AlistClient 一致）
    let is_rustsync = req.engine_type == "rustsync";
    let user_name = if !is_rustsync {
        if req.token.is_none() || req.token.as_deref().map_or(true, |s| s.trim().is_empty()) {
            return Json(ApiResponse::err("令牌不能为空"));
        }
        match verify_alist_connection(&req.url, req.token.as_deref().unwrap_or("")).await {
            Ok(username) => username,
            Err(e) => return Json(ApiResponse::err(&format!("引擎连接验证失败: {}", e))),
        }
    } else {
        req.user_name.clone()
    };

    let db = state.db.lock().unwrap();
    match db.execute(
        "INSERT INTO alist_list (remark, url, userName, token, engineType) VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![req.remark, req.url, user_name, req.token, req.engine_type],
    ) {
        Ok(_) => {
            let engine_id = db.last_insert_rowid();
            drop(db);
            tracing::info!("引擎 {} 添加成功 (id={})", req.url, engine_id);
            Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_added")))
        }
        Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
    }
}

/// PUT /svr/alist - 更新引擎（id 在 body 中）
/// 与 Python updateClient 一致：检查 protected、验证连接变更、清除 snapshot
pub async fn alist_put(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    if id == 0 {
        return Json(ApiResponse::err("缺少引擎ID"));
    }

    let old = {
        let db = state.db.lock().unwrap();
        db.query_row(
            "SELECT id, remark, url, userName, token, engineType, systemKey, protected, createTime
             FROM alist_list WHERE id=?",
            [id],
            |row| {
                Ok(Engine {
                    id: row.get(0)?,
                    remark: row.get(1)?,
                    url: row.get(2)?,
                    user_name: row.get(3)?,
                    token: row.get(4)?,
                    engine_type: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "alist".to_string()),
                    system_key: row.get(6)?,
                    protected: row.get::<_, Option<bool>>(7)?.unwrap_or(false),
                    create_time: row.get(8)?,
                })
            },
        )
    };

    let old = match old {
        Ok(e) => e,
        Err(_) => return Json(ApiResponse::err("引擎不存在")),
    };

    // 与 Python updateClient 一致：检查 protected
    if old.protected {
        return Json(ApiResponse::err(&i18n::t("builtin_engine_protected")));
    }

    let mut remark = body.get("remark").and_then(|v| v.as_str()).map(|s| s.to_string());
    let mut url = body.get("url").and_then(|v| v.as_str()).unwrap_or(&old.url).to_string();
    let user_name = body.get("userName").and_then(|v| v.as_str()).map(|s| s.to_string());
    let has_token = body.get("token").is_some();
    let mut token = body.get("token").and_then(|v| v.as_str()).map(|s| s.to_string());
    let engine_type = body.get("engineType").and_then(|v| v.as_str()).unwrap_or(&old.engine_type);

    // URL 标准化：去除尾部斜杠（与 Python updateClient 一致）
    if url.ends_with('/') {
        url = url.trim_end_matches('/').to_string();
    }
    // remark 空字符串转 None
    if remark.as_deref().map_or(false, |s| s.trim().is_empty()) {
        remark = None;
    }
    // token 空字符串处理
    if token.as_deref().map_or(false, |s| s.trim().is_empty()) {
        token = None;
    }

    // 与 Python updateClient 一致：检查连接是否变更
    let connection_changed = old.url != url || has_token;
    if connection_changed {
        // 令牌必填，防止通过修改地址为钓鱼地址的方式窃取令牌
        if !has_token || token.is_none() {
            return Json(ApiResponse::err(&i18n::t("without_token")));
        }
        // 对外部 AList 引擎验证新连接
        if engine_type != "rustsync" {
            if let Err(e) = verify_alist_connection(&url, token.as_deref().unwrap_or("")).await {
                return Json(ApiResponse::err(&format!("引擎连接验证失败: {}", e)));
            }
        }
    }

    // 合并 token：未提供时保留旧 token
    let final_token = if has_token { token } else { old.token.clone() };

    {
        let db = state.db.lock().unwrap();
        match db.execute(
            "UPDATE alist_list SET remark=?, url=?, userName=?, token=?, engineType=? WHERE id=?",
            rusqlite::params![remark, url, user_name, final_token, engine_type, id],
        ) {
            Ok(_) => {
                // 与 Python updateClient 一致：连接变更时清除 source snapshot
                if connection_changed {
                    clear_snapshots_by_engine(&db, id);
                }
            }
            Err(e) => return Json(ApiResponse::err(&format!("更新失败: {}", e))),
        }
    }

    Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_updated")))
}

/// DELETE /svr/alist - 删除引擎（id 在 body 中）
/// 与 Python removeClient 一致：检查 protected 并清除缓存
pub async fn alist_delete(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    if id == 0 {
        return Json(ApiResponse::err("缺少引擎ID"));
    }
    let db = state.db.lock().unwrap();
    // 检查是否内置引擎
    let protected: bool = db
        .query_row("SELECT protected FROM alist_list WHERE id=?", [id], |row| row.get(0))
        .unwrap_or(false);
    if protected {
        return Json(ApiResponse::err(&i18n::t("builtin_engine_protected")));
    }
    // 清除 source snapshot
    clear_snapshots_by_engine(&db, id);
    match db.execute("DELETE FROM alist_list WHERE id=? AND protected=0", [id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}


/// 获取文件系统根目录列表，与 Python _filesystem_roots 一致
fn filesystem_roots() -> Vec<serde_json::Value> {
    let mut roots = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add = |name: &str, path: &str| {
        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());
        if seen.contains(&canonical) {
            return;
        }
        seen.insert(canonical.clone());
        roots.push(serde_json::json!({
            "name": name,
            "path": canonical,
        }));
    };

    #[cfg(target_os = "windows")]
    {
        for letter in 'A'..='Z' {
            let path = format!("{}:\\", letter);
            if std::path::Path::new(&path).is_dir() {
                add(&format!("{}:", letter), &path);
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        add("/", "/");
        if let Ok(home) = std::env::var("HOME") {
            if let Ok(canonical_home) = std::fs::canonicalize(&home) {
                let home_str = canonical_home.to_string_lossy().to_string();
                let root_str = std::fs::canonicalize("/")
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "/".to_string());
                if home_str != root_str {
                    add("home", &home_str);
                }
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_str = cwd.to_string_lossy().to_string();
            add("cwd", &cwd_str);
        }
    }
    roots
}

// ==================== 存储挂载 ====================

/// GET /svr/storage - 获取挂载列表或浏览/发现
/// 前端调用: storageGet(engineId), storageLocalBrowse(path), storageSmbDiscover()
pub async fn storage_get(
    State(state): State<crate::state::SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ApiResponse<serde_json::Value>> {
    let action = params.get("action").map(|s| s.as_str());

    match action {
        Some("localBrowse") => {
            let path = params.get("path").map(|s| s.to_string());
            let result = tokio::task::spawn_blocking(move || {
                let current = if let Some(ref p) = path {
                    if p.is_empty() {
                        std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "/".to_string())
                    } else {
                        p.clone()
                    }
                } else {
                    std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "/".to_string())
                };
                let current_path = std::path::Path::new(&current);
                let current = std::fs::canonicalize(current_path).map(|p| p.to_string_lossy().to_string()).unwrap_or(current);
                if !std::path::Path::new(&current).is_dir() {
                    return Err("local browse path must be an existing directory".to_string());
                }
                let parent_path = std::path::Path::new(&current).parent().map(|p| p.to_string_lossy().to_string());
                let parent = match parent_path {
                    Some(ref p) => {
                        let resolved = std::fs::canonicalize(p).map(|cp| cp.to_string_lossy().to_string()).unwrap_or_else(|_| p.clone());
                        if resolved == current { None } else { Some(resolved) }
                    }
                    None => None,
                };
                let roots = filesystem_roots();
                let mut directories: Vec<serde_json::Value> = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&current) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path.is_symlink() {
                            continue;
                        }
                        if !entry_path.is_dir() {
                            continue;
                        }
                        let entry_name = entry.file_name().to_string_lossy().to_string();
                        let resolved = std::fs::canonicalize(&entry_path).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| entry_path.to_string_lossy().to_string());
                        directories.push(serde_json::json!({
                            "name": entry_name,
                            "path": resolved,
                        }));
                    }
                }
                directories.sort_by(|a, b| {
                    a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
                });
                Ok(serde_json::json!({
                    "path": current,
                    "parent": parent,
                    "roots": roots,
                    "directories": directories,
                }))
            }).await;
            match result {
                Ok(Ok(data)) => Json(ApiResponse::ok(data)),
                Ok(Err(e)) => Json(ApiResponse::err(&e)),
                Err(e) => Json(ApiResponse::err(&format!("任务失败: {}", e))),
            }
        }
        Some("smbDiscover") => {
            // SMB 发现 - 暂返回空列表
            Json(ApiResponse::ok(serde_json::json!([])))
        }
        Some("types") => {
            // 返回支持的驱动类型
            Json(ApiResponse::ok(serde_json::json!(["local", "ftp", "sftp", "smb", "aliyun"])))
        }
        _ => {
            // 默认：获取挂载列表
            let engine_id: i64 = params.get("engineId").and_then(|s| s.parse().ok()).unwrap_or(0);
            let db = state.db.lock().unwrap();

            // 与 Python _requireRustSync 一致：检查引擎类型
            if let Err(e) = require_rustsync_engine(&db, engine_id) {
                return Json(ApiResponse::err(&e));
            }

            let mut stmt = match db.prepare(
                "SELECT id, engineId, name, driverType, config, enabled, configVersion, authVersion, createTime
                 FROM storage_mount WHERE engineId=? AND enabled=1 ORDER BY createTime ASC, id ASC",
            ) {
                Ok(s) => s,
                Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
            };
            let rows = stmt.query_map([engine_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "engineId": row.get::<_, i64>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "driverType": row.get::<_, String>(3)?,
                    "config": row.get::<_, String>(4)?,
                    "enabled": row.get::<_, i32>(5)? != 0,
                    "configVersion": row.get::<_, i32>(6)?.max(1),
                    "authVersion": row.get::<_, i32>(7)?.max(1),
                    "createTime": row.get::<_, i64>(8)?,
                }))
            });
            match rows {
                Ok(iter) => {
                    // 与 Python _sanitized 一致：脱敏密钥
                    let list: Vec<serde_json::Value> = iter
                        .filter_map(|r| r.ok())
                        .map(|mut item| {
                            // 解析 config JSON 字符串
                            if let Some(config_str) = item.get("config").and_then(|v| v.as_str()) {
                                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(config_str) {
                                    if let Some(obj) = item.as_object_mut() {
                                        obj.insert("config".to_string(), config_json);
                                    }
                                }
                            }
                            sanitize_storage_mount(item)
                        })
                        .collect();
                    Json(ApiResponse::ok(serde_json::json!(list)))
                }
                Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
            }
        }
    }
}

/// POST /svr/storage - 添加挂载或 SFTP 测试/浏览
/// 前端: storagePost(data), storageSftpTest(data), storageSftpBrowse(data)
pub async fn storage_post(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let action = body.get("action").and_then(|v| v.as_str());

    match action {
        Some("sftpTest") => {
            let host = body.get("host").and_then(|v| v.as_str()).unwrap_or("");
            let port = body.get("port").and_then(|v| v.as_u64()).unwrap_or(22) as u16;
            let addr = format!("{}:{}", host, port);
            match tokio::net::TcpStream::connect(&addr).await {
                Ok(_) => Json(ApiResponse::ok(serde_json::json!({
                    "connected": true,
                    "message": "SSH 服务器可连接"
                }))),
                Err(e) => Json(ApiResponse::err(&format!("连接失败: {}", e))),
            }
        }
        Some("sftpBrowse") => {
            // SFTP 浏览 - 暂返回空
            let path = body.get("path").and_then(|v| v.as_str()).unwrap_or("/");
            Json(ApiResponse::ok(serde_json::json!({
                "path": path,
                "files": []
            })))
        }
        _ => {
            // 添加挂载 - 与 Python addMount 一致
            let engine_id = body.get("engineId").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let driver_type = body.get("driverType").and_then(|v| v.as_str()).unwrap_or("");
            let config = body.get("config").cloned().unwrap_or_default();
            let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            let config_str = serde_json::to_string(&config).unwrap_or_default();

            let db = state.db.lock().unwrap();
            // 与 Python _requireRustSync 一致：检查引擎类型
            if let Err(e) = require_rustsync_engine(&db, engine_id) {
                return Json(ApiResponse::err(&e));
            }
            match db.execute(
                "INSERT INTO storage_mount (engineId, name, driverType, config, enabled) VALUES (?, ?, ?, ?, ?)",
                rusqlite::params![engine_id, name, driver_type, config_str, enabled as i32],
            ) {
                Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("mount_added"))),
                Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
            }
        }
    }
}

/// PUT /svr/storage - 更新挂载（id 在 body 中）
pub async fn storage_put(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    if id == 0 {
        return Json(ApiResponse::err("缺少挂载ID"));
    }
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let driver_type = body.get("driverType").and_then(|v| v.as_str()).unwrap_or("");
    let config = body.get("config").cloned().unwrap_or_default();
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let config_str = serde_json::to_string(&config).unwrap_or_default();

    let db = state.db.lock().unwrap();
    match db.execute(
        "UPDATE storage_mount SET name=?, driverType=?, config=?, enabled=?, configVersion=configVersion+1 WHERE id=?",
        rusqlite::params![name, driver_type, config_str, enabled as i32, id],
    ) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("mount_updated"))),
        Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
    }
}

/// DELETE /svr/storage - 删除挂载（id 在 body 中）
pub async fn storage_delete(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let db = state.db.lock().unwrap();
    match db.execute("DELETE FROM storage_mount WHERE id=?", [id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("mount_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
}