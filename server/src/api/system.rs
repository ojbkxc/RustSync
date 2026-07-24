use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
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
    let file_path = format!("static/{}", path);

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