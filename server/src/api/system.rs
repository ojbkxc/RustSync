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
        Ok(content) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], content).into_response(),
        Err(_) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], include_str!("../../static/index.html").to_string()).into_response(),
    }
}

pub async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::ok(serde_json::json!({"status": "healthy", "version": env!("CARGO_PKG_VERSION")})))
}

pub async fn get_language() -> impl IntoResponse {
    Json(ApiResponse::ok(serde_json::json!({"language": i18n::get_current_lang(), "languages": i18n::get_supported_languages()})))
}

pub async fn set_language(Json(req): Json<LanguageRequest>) -> Json<ApiResponse<serde_json::Value>> {
    i18n::set_current_lang(&req.language);
    Json(ApiResponse::ok_msg(serde_json::json!({}), "语言设置成功"))
}

pub async fn spa_fallback(axum::extract::Path(path): axum::extract::Path<String>) -> axum::response::Response {
    use axum::http::header;
    let safe_path = path.replace("..", "").replace('\\', "/");
    let file_path = format!("static/{}", safe_path.trim_start_matches('/'));
    if let Ok(content) = tokio::fs::read(&file_path).await {
        let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.to_string())], content).into_response();
    }
    index().await
}

#[derive(Deserialize)]
pub struct LogQuery {
    #[serde(default)]
    pub lines: Option<usize>,
}

pub async fn log_list() -> impl IntoResponse {
    let log_dir = crate::config::get_config().log_dir;
    let dir = std::path::Path::new(&log_dir);
    let mut logs = Vec::new();
    if dir.exists() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let meta = entry.metadata().ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                logs.push(serde_json::json!({"name": name, "size": size, "modified": modified}));
            }
        }
    }
    logs.sort_by(|a, b| b["modified"].as_u64().cmp(&a["modified"].as_u64()));
    Json(ApiResponse::ok(logs))
}

pub async fn log_read(Query(q): Query<LogQuery>) -> impl IntoResponse {
    let log_dir = crate::config::get_config().log_dir;
    let log_path = std::path::Path::new(&log_dir).join("rustsync.log");
    if !log_path.exists() { return Json(ApiResponse::not_found("日志文件不存在")); }
    let max_lines = q.lines.unwrap_or(500);
    // Efficient tail reading: read only the last N lines instead of the whole file
    let content = match tail_file(&log_path, max_lines) {
        Ok(c) => c,
        Err(e) => return Json(ApiResponse::err(&format!("读取日志失败: {}", e))),
    };
    Json(ApiResponse::ok(serde_json::json!({"lines": max_lines, "content": content})))
}

/// Read the last N lines of a file efficiently by seeking from the end
fn tail_file(path: &std::path::Path, max_lines: usize) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();
    if file_size == 0 { return Ok(String::new()); }
    let mut file = file;
    let buf_size = 8192usize.min(file_size as usize);
    let mut buf = vec![0u8; buf_size];
    let mut all_lines: Vec<String> = Vec::new();
    let mut remaining = file_size;
    while all_lines.len() <= max_lines && remaining > 0 {
        let read_size = buf_size.min(remaining as usize);
        let seek_pos = remaining - read_size as u64;
        file.seek(SeekFrom::Start(seek_pos))?;
        buf.truncate(read_size);
        buf.resize(read_size, 0);
        file.read_exact(&mut buf)?;
        let chunk = String::from_utf8_lossy(&buf).to_string();
        let mut chunk_lines: Vec<String> = chunk.lines().map(|s| s.to_string()).collect();
        chunk_lines.extend(all_lines);
        all_lines = chunk_lines;
        remaining = seek_pos;
    }
    if all_lines.len() > max_lines {
        all_lines = all_lines[all_lines.len() - max_lines..].to_vec();
    }
    Ok(all_lines.join("\n"))
}

pub async fn log_clear() -> Json<ApiResponse<serde_json::Value>> {
    let log_dir = crate::config::get_config().log_dir;
    let log_path = std::path::Path::new(&log_dir).join("rustsync.log");
    match std::fs::write(&log_path, "") {
        Ok(_) => Json(ApiResponse::ok_msg(serde_json::json!({}), "日志已清空")),
        Err(e) => Json(ApiResponse::err(&format!("清空日志失败: {}", e))),
    }
}