use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::data::models::{ChangePasswordRequest, LoginRequest, ResetPasswordRequest, User, UserInfo};
use crate::data::response::ApiResponse;
use crate::service::db::hash_password;

/// 简单的内存会话管理
pub struct SessionManager {
    sessions: RwLock<std::collections::HashMap<String, SessionInfo>>,
}

#[derive(Clone)]
pub struct SessionInfo {
    pub user_id: i64,
    pub user_name: String,
    pub expires_at: i64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub async fn create_session(&self, user_id: i64, user_name: &str, expires_days: u32) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now().timestamp() + (expires_days as i64) * 86400;
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            token.clone(),
            SessionInfo {
                user_id,
                user_name: user_name.to_string(),
                expires_at,
            },
        );
        token
    }

    pub async fn validate_session(&self, token: &str) -> Option<SessionInfo> {
        let sessions = self.sessions.read().await;
        if let Some(info) = sessions.get(token) {
            if info.expires_at > chrono::Utc::now().timestamp() {
                return Some(info.clone());
            }
        }
        None
    }

    pub async fn remove_session(&self, token: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(token);
    }
}

pub type SharedSessionManager = Arc<SessionManager>;

// ==================== 登录 ====================

/// POST /svr/noAuth/login - 登录
pub async fn login(
    State(state): State<crate::state::SharedState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let result = {
        let db = state.db.lock().unwrap();
        db.query_row(
            "SELECT id, userName, passwd, sqlVersion, createTime FROM user_list WHERE userName=?",
            [&req.user_name],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    user_name: row.get(1)?,
                    passwd: row.get(2)?,
                    sql_version: row.get(3)?,
                    create_time: row.get(4)?,
                })
            },
        )
    }; // MutexGuard dropped here, safe to .await below

    match result {
        Ok(user) => {
            let hash = hash_password(&req.passwd);
            if hash == user.passwd {
                let session_mgr = get_session_manager();
                let token = session_mgr
                    .create_session(user.id, &user.user_name, state.config.expires)
                    .await;
                let user_return = json!({
                    "id": user.id,
                    "userName": user.user_name,
                    "createTime": user.create_time
                });
                let response = Json(ApiResponse::ok(user_return));
                // 设置 Cookie（与 Python set_signed_cookie 行为一致）
                let cookie = format!(
                    "tao_sync={}; Path=/; HttpOnly; Max-Age={}",
                    token,
                    state.config.expires as i64 * 86400
                );
                (
                    StatusCode::OK,
                    [(header::SET_COOKIE, cookie)],
                    response,
                )
                    .into_response()
            } else {
                (
                    StatusCode::OK,
                    Json(ApiResponse::<()>::err("密码错误")),
                )
                    .into_response()
            }
        }
        Err(_) => (
            StatusCode::OK,
            Json(ApiResponse::<()>::err("用户不存在")),
        )
            .into_response(),
    }
}

/// PUT /svr/noAuth/login - 重置密码（使用 secret.key 验证）
pub async fn reset_password(
    State(state): State<crate::state::SharedState>,
    Json(req): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    let db = state.db.lock().unwrap();
    let user = db.query_row(
        "SELECT id, userName FROM user_list WHERE userName=?",
        [&req.user_name],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    );

    match user {
        Ok((user_id, _)) => {
            // 验证 key 是否与 secret.key 一致
            if req.key.trim() != state.config.password_str {
                return Json(ApiResponse::<serde_json::Value>::err("密钥错误"));
            }
            let new_passwd = req.passwd.as_deref().unwrap_or("");
            if new_passwd.is_empty() {
                // 未提供新密码则生成随机密码
                let pwd = crate::service::db::generate_random_password();
                let hash = hash_password(&pwd);
                drop(db);
                let db = state.db.lock().unwrap();
                let _ = db.execute(
                    "UPDATE user_list SET passwd=? WHERE id=?",
                    rusqlite::params![hash, user_id],
                );
                return Json(ApiResponse::ok(json!({"passwd": pwd})));
            }
            let hash = hash_password(new_passwd.trim());
            drop(db);
            let db = state.db.lock().unwrap();
            let _ = db.execute(
                "UPDATE user_list SET passwd=? WHERE id=?",
                rusqlite::params![hash, user_id],
            );
            Json(ApiResponse::ok_msg(json!({}), "密码重置成功"))
        }
        Err(_) => Json(ApiResponse::<serde_json::Value>::err("用户不存在")),
    }
}

/// DELETE /svr/noAuth/login - 登出
pub async fn logout(headers: axum::http::HeaderMap) -> impl IntoResponse {
    if let Some(session) = get_session_from_headers_raw(&headers).await {
        let session_mgr = get_session_manager();
        session_mgr.remove_session(&session).await;
    }
    let cookie = "tao_sync=; Path=/; HttpOnly; Max-Age=0";
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(ApiResponse::ok_msg(json!({}), "已登出")),
    )
        .into_response()
}

// ==================== 用户 ====================

/// GET /svr/user - 获取当前用户信息
pub async fn get_user(
    State(state): State<crate::state::SharedState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Some(session) = get_session_from_headers(&headers).await {
        let db = state.db.lock().unwrap();
        let result = db.query_row(
            "SELECT id, createTime FROM user_list WHERE id=?",
            [session.user_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        );
        match result {
            Ok((id, create_time)) => Json(ApiResponse::ok(UserInfo {
                id,
                user_name: session.user_name,
                create_time,
            })),
            Err(_) => Json(ApiResponse::<UserInfo>::err("用户不存在")),
        }
    } else {
        Json(ApiResponse::<UserInfo>::err_code(401, "未登录"))
    }
}

/// PUT /svr/user - 修改密码
pub async fn change_password(
    State(state): State<crate::state::SharedState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    if let Some(session) = get_session_from_headers(&headers).await {
        let db = state.db.lock().unwrap();
        let old_hash = hash_password(&req.old_passwd);
        let valid: bool = db
            .query_row(
                "SELECT passwd FROM user_list WHERE id=?",
                [session.user_id],
                |row| row.get::<_, String>(0),
            )
            .map(|p| p == old_hash)
            .unwrap_or(false);

        if !valid {
            return Json(ApiResponse::<serde_json::Value>::err("原密码错误"));
        }

        let new_hash = hash_password(&req.passwd);
        drop(db);
        let db = state.db.lock().unwrap();
        match db.execute(
            "UPDATE user_list SET passwd=? WHERE id=?",
            rusqlite::params![new_hash, session.user_id],
        ) {
            Ok(_) => Json(ApiResponse::ok_msg(json!({}), "密码修改成功")),
            Err(e) => Json(ApiResponse::<serde_json::Value>::err(&format!("修改失败: {}", e))),
        }
    } else {
        Json(ApiResponse::<serde_json::Value>::err_code(401, "未登录"))
    }
}

// ==================== 会话管理工具 ====================

use std::sync::OnceLock;

static SESSION_MANAGER: OnceLock<SharedSessionManager> = OnceLock::new();

pub fn get_session_manager() -> SharedSessionManager {
    SESSION_MANAGER
        .get_or_init(|| Arc::new(SessionManager::new()))
        .clone()
}

pub async fn get_session_from_headers(headers: &axum::http::HeaderMap) -> Option<SessionInfo> {
    let token = extract_token(headers)?;
    let session_mgr = get_session_manager();
    session_mgr.validate_session(&token).await
}

async fn get_session_from_headers_raw(headers: &axum::http::HeaderMap) -> Option<String> {
    extract_token(headers)
}

fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    // 先从 Cookie 中提取
    if let Some(cookie) = headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie.to_str() {
            for part in cookie_str.split(';') {
                let part = part.trim();
                if part.starts_with("tao_sync=") {
                    return Some(part["tao_sync=".len()..].to_string());
                }
            }
        }
    }
    // 再从 Authorization header 提取
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }
    None
}

/// 中间件：需要认证的请求
pub async fn require_auth<B>(
    headers: axum::http::HeaderMap,
    request: axum::http::Request<B>,
    next: axum::middleware::Next<B>,
) -> axum::response::Response {
    if get_session_from_headers(&headers).await.is_some() {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err_code(401, "未登录或会话已过期")),
        )
            .into_response()
    }
}