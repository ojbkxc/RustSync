use axum::{
    extract::Query,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use crate::data::models::LanguageRequest;
use crate::data::response::ApiResponse;
use crate::service::i18n;

pub async fn index() -> axum::response::Response {
    use axum::http::header;
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(content) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            content,
        )
            .into_response(),
        Err(_) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            include_str!("../../static/index.html").to_string(),
        )
            .into_response(),
    }
}

pub async fn get_language() -> impl IntoResponse {
    Json(ApiResponse::ok(serde_json::json!({
        "language": i18n::get_current_lang(),
        "languages": i18n::get_supported_languages()
    })))
}

pub async fn set_language(Json(req): Json<LanguageRequest>) -> impl IntoResponse {
    i18n::set_current_lang(&req.language);
    Json(ApiResponse::ok_msg(serde_json::json!({}), "语言设置成功"))
}

/// 提供前端静态文件 fallback - SPA 模式，所有非 API 路径返回 index.html
pub async fn spa_fallback(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::header;
    // 防止路径穿越
    let safe_path = path.replace("..", "").replace('\\', "/");
    let file_path = format!("static/{}", safe_path.trim_start_matches('/'));

    // 先尝试读取具体文件
    if let Ok(content) = tokio::fs::read(&file_path).await {
        let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
        return (
            [(header::CONTENT_TYPE, mime.to_string())],
            content,
        )
            .into_response();
    }

    // 回退到 index.html（SPA）
    index().await
}

#[derive(Deserialize)]
pub struct LogQuery {
    #[serde(default)]
    pub lines: Option<usize>,
}

/// 获取日志文件列表
pub async fn log_list() -> impl axum::response::IntoResponse {
    let log_dir = crate::config::get_config().log_dir;
    let dir = std::path::Path::new(&log_dir);
    
    let mut logs = Vec::new();
    if dir.exists() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let meta = entry.metadata().ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                logs.push(serde_json::json!({
                    "name": name,
                    "size": size,
                    "modified": modified,
                }));
            }
        }
    }
    
    logs.sort_by(|a, b| b["modified"].as_u64().cmp(&a["modified"].as_u64()));
    Json(ApiResponse::ok(logs))
}

/// 读取日志内容
pub async fn log_read(Query(q): Query<LogQuery>) -> impl axum::response::IntoResponse {
    let log_dir = crate::config::get_config().log_dir;
    let log_path = std::path::Path::new(&log_dir).join("rustsync.log");
    
    if !log_path.exists() {
        return Json(ApiResponse::err("日志文件不存在"));
    }
    
    let content = match std::fs::read_to_string(&log_path) {
        Ok(c) => c,
        Err(e) => return Json(ApiResponse::err(&format!("读取日志失败: {}", e))),
    };
    
    let lines: Vec<&str> = content.lines().collect();
    let max_lines = q.lines.unwrap_or(500);
    let start = if lines.len() > max_lines { lines.len() - max_lines } else { 0 };
    let recent: Vec<&str> = lines[start..].to_vec();
    
    Json(ApiResponse::ok(serde_json::json!({
        "total_lines": lines.len(),
        "content": recent.join("\n"),
    })))
}

/// 清空日志
pub async fn log_clear() -> impl axum::response::IntoResponse {
    let log_dir = crate::config::get_config().log_dir;
    let log_path = std::path::Path::new(&log_dir).join("rustsync.log");
    
    match std::fs::write(&log_path, "") {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), "日志已清空")),
        Err(e) => Json(ApiResponse::err(&format!("清空日志失败: {}", e))),
    }
}