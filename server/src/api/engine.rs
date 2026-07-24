use axum::{
    extract::State,
    Json,
};
use crate::data::models::{Engine, EngineRequest, StorageMount};
use crate::data::response::ApiResponse;
use crate::service::i18n;

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
        // 返回引擎列表（分页）
        let page_num: i32 = params.get("pageNum").and_then(|s| s.parse().ok()).unwrap_or(1);
        let page_size: i32 = params.get("pageSize").and_then(|s| s.parse().ok()).unwrap_or(10);
        let offset = (page_num - 1) * page_size;

        let total: i64 = db
            .query_row("SELECT count(*) FROM alist_list", [], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = match db.prepare(
            "SELECT id, remark, url, userName, token, engineType, systemKey, protected, createTime
             FROM alist_list ORDER BY id DESC LIMIT ? OFFSET ?",
        ) {
            Ok(s) => s,
            Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
        };

        let rows = stmt.query_map([page_size, offset], |row| {
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
        });

        match rows {
            Ok(iter) => {
                let list: Vec<Engine> = iter.filter_map(|r| r.ok()).collect();
                Json(ApiResponse::ok(serde_json::json!({
                    "list": list,
                    "total": total,
                    "pageNum": page_num,
                    "pageSize": page_size
                })))
            }
            Err(e) => Json(ApiResponse::err(&format!("查询失败: {}", e))),
        }
    }
}

/// POST /svr/alist - 添加引擎
pub async fn alist_post(
    State(state): State<crate::state::SharedState>,
    Json(req): Json<EngineRequest>,
) -> Json<ApiResponse<serde_json::Value>> {
    let db = state.db.lock().unwrap();
    match db.execute(
        "INSERT INTO alist_list (remark, url, userName, token, engineType) VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![req.remark, req.url, req.user_name, req.token, req.engine_type],
    ) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_added"))),
        Err(e) => Json(ApiResponse::err(&format!("添加失败: {}", e))),
    }
}

/// PUT /svr/alist - 更新引擎（id 在 body 中）
pub async fn alist_put(
    State(state): State<crate::state::SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let id = body.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    if id == 0 {
        return Json(ApiResponse::err("缺少引擎ID"));
    }
    let db = state.db.lock().unwrap();
    let remark = body.get("remark").and_then(|v| v.as_str()).map(|s| s.to_string());
    let url = body.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let user_name = body.get("userName").and_then(|v| v.as_str()).map(|s| s.to_string());
    let token = body.get("token").and_then(|v| v.as_str()).map(|s| s.to_string());
    let engine_type = body.get("engineType").and_then(|v| v.as_str()).unwrap_or("alist");

    match db.execute(
        "UPDATE alist_list SET remark=?, url=?, userName=?, token=?, engineType=? WHERE id=?",
        rusqlite::params![remark, url, user_name, token, engine_type, id],
    ) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_updated"))),
        Err(e) => Json(ApiResponse::err(&format!("更新失败: {}", e))),
    }
}

/// DELETE /svr/alist - 删除引擎（id 在 body 中）
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
    match db.execute("DELETE FROM alist_list WHERE id=?", [id]) {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), &i18n::t("engine_deleted"))),
        Err(e) => Json(ApiResponse::err(&format!("删除失败: {}", e))),
    }
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
            let path = params.get("path").map(|s| s.as_str()).unwrap_or("/").to_string();
            let result = tokio::task::spawn_blocking(move || {
                let dir = std::fs::read_dir(&path);
                match dir {
                    Ok(entries) => {
                        let mut files: Vec<serde_json::Value> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let p = e.path();
                                let name = e.file_name().to_string_lossy().to_string();
                                let is_dir = p.is_dir();
                                serde_json::json!({
                                    "name": name,
                                    "path": p.to_string_lossy(),
                                    "isDir": is_dir,
                                })
                            })
                            .collect();
                        files.sort_by(|a, b| {
                            let a_dir = a["isDir"].as_bool().unwrap_or(false);
                            let b_dir = b["isDir"].as_bool().unwrap_or(false);
                            b_dir.cmp(&a_dir).then_with(|| {
                                a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
                            })
                        });
                        Ok(files)
                    }
                    Err(e) => Err(format!("读取目录失败: {}", e)),
                }
            }).await;
            match result {
                Ok(Ok(files)) => Json(ApiResponse::ok(serde_json::Value::Array(files))),
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
            let mut stmt = match db.prepare(
                "SELECT id, engineId, name, driverType, config, enabled, configVersion, authVersion, createTime
                 FROM storage_mount WHERE engineId=? ORDER BY id",
            ) {
                Ok(s) => s,
                Err(e) => return Json(ApiResponse::err(&format!("查询失败: {}", e))),
            };
            let rows = stmt.query_map([engine_id], |row| {
                Ok(StorageMount {
                    id: row.get(0)?,
                    engine_id: row.get(1)?,
                    name: row.get(2)?,
                    driver_type: row.get(3)?,
                    config: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
                    config_version: row.get::<_, i32>(6)?.max(1),
                    auth_version: row.get::<_, i32>(7)?.max(1),
                    create_time: row.get(8)?,
                })
            });
            match rows {
                Ok(iter) => {
                    let list: Vec<StorageMount> = iter.filter_map(|r| r.ok()).collect();
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
            // 添加挂载
            let engine_id = body.get("engineId").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let driver_type = body.get("driverType").and_then(|v| v.as_str()).unwrap_or("");
            let config = body.get("config").cloned().unwrap_or_default();
            let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            let config_str = serde_json::to_string(&config).unwrap_or_default();

            let db = state.db.lock().unwrap();
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