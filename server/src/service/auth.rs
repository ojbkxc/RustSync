use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde_json::json;

use crate::data::models::{
    ChangePasswordRequest, Claims, LoginRequest, ResetPasswordRequest, User, UserInfo,
};
use crate::data::response::ApiResponse;
use crate::service::db;

pub fn create_jwt(user_id: i64, user_name: &str, expires_hours: u32, secret: &str) -> String {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        user_name: user_name.to_string(),
        iat: now.timestamp() as usize,
        exp: (now.timestamp() + expires_hours as i64 * 3600) as usize,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .expect("JWT encode failed")
}

pub fn validate_jwt(token: &str, secret: &str) -> Option<Claims> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .ok()
        .map(|d| d.claims)
}

pub async fn login(
    State(state): State<crate::state::SharedState>,
    Json(req): Json<LoginRequest>,
) -> axum::response::Response {
    let result = {
        let conn = state.db.get().unwrap();
        conn.query_row(
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
    };

    match result {
        Ok(user) => {
            if db::verify_password(&req.passwd, &user.passwd) {
                let token = create_jwt(user.id, &user.user_name, state.config.expires * 24, &state.config.jwt_secret);
                let user_return = json!({
                    "id": user.id,
                    "userName": user.user_name,
                    "createTime": user.create_time,
                    "token": token,
                });
                let response = Json(ApiResponse::ok(user_return));
                let cookie = format!(
                    "rust_sync={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                    token,
                    state.config.expires as i64 * 86400
                );
                (StatusCode::OK, [(header::SET_COOKIE, cookie)], response).into_response()
            } else {
                (StatusCode::OK, Json(ApiResponse::<()>::unauthorized("密码错误"))).into_response()
            }
        }
        Err(_) => (StatusCode::OK, Json(ApiResponse::<()>::not_found("用户不存在"))).into_response(),
    }
}

pub async fn reset_password(
    State(state): State<crate::state::SharedState>,
    Json(req): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    let conn = state.db.get().unwrap();
    let user = conn.query_row(
        "SELECT id, userName FROM user_list WHERE userName=?",
        [&req.user_name],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    );

    match user {
        Ok((user_id, _)) => {
            if req.key.trim() != state.config.jwt_secret {
                return Json(ApiResponse::<serde_json::Value>::forbidden("密钥错误"));
            }
            let new_passwd = req.passwd.as_deref().unwrap_or("");
            if new_passwd.is_empty() {
                let pwd = db::generate_random_password();
                let hash = db::hash_password(&pwd);
                let _ = conn.execute("UPDATE user_list SET passwd=? WHERE id=?", rusqlite::params![hash, user_id]);
                return Json(ApiResponse::ok(json!({"passwd": pwd})));
            }
            let hash = db::hash_password(new_passwd.trim());
            let _ = conn.execute("UPDATE user_list SET passwd=? WHERE id=?", rusqlite::params![hash, user_id]);
            Json(ApiResponse::ok_msg(json!({}), "密码重置成功"))
        }
        Err(_) => Json(ApiResponse::<serde_json::Value>::not_found("用户不存在")),
    }
}

pub async fn logout() -> impl IntoResponse {
    let cookie = "rust_sync=; Path=/; HttpOnly; Max-Age=0";
    (StatusCode::OK, [(header::SET_COOKIE, cookie)], Json(ApiResponse::ok_msg(json!({}), "已登出"))).into_response()
}

pub async fn get_user(
    State(state): State<crate::state::SharedState>,
    claims: Claims,
) -> impl IntoResponse {
    let conn = state.db.get().unwrap();
    let result = conn.query_row(
        "SELECT id, createTime FROM user_list WHERE id=?",
        [claims.sub],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    );
    match result {
        Ok((id, create_time)) => Json(ApiResponse::ok(UserInfo { id, user_name: claims.user_name, create_time })),
        Err(_) => Json(ApiResponse::<UserInfo>::not_found("用户不存在")),
    }
}

pub async fn change_password(
    State(state): State<crate::state::SharedState>,
    claims: Claims,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let conn = state.db.get().unwrap();
    let valid: bool = conn
        .query_row("SELECT passwd FROM user_list WHERE id=?", [claims.sub], |row| row.get::<_, String>(0))
        .map(|h| db::verify_password(&req.old_passwd, &h))
        .unwrap_or(false);

    if !valid {
        return Json(ApiResponse::<serde_json::Value>::bad_request("原密码错误"));
    }

    let new_hash = db::hash_password(&req.passwd);
    match conn.execute("UPDATE user_list SET passwd=? WHERE id=?", rusqlite::params![new_hash, claims.sub]) {
        Ok(_) => Json(ApiResponse::ok_msg(json!({}), "密码修改成功")),
        Err(e) => Json(ApiResponse::<serde_json::Value>::err(&format!("修改失败: {}", e))),
    }
}

fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(cookie) = headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie.to_str() {
            for part in cookie_str.split(';') {
                let part = part.trim();
                if part.starts_with("rust_sync=") {
                    return Some(part["rust_sync=".len()..].to_string());
                }
            }
        }
    }
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub async fn require_auth<B>(
    headers: axum::http::HeaderMap,
    mut request: axum::http::Request<B>,
    next: axum::middleware::Next<B>,
) -> axum::response::Response {
    let state = crate::state::get_global_state();
    if let Some(token) = extract_token(&headers) {
        if let Some(claims) = validate_jwt(&token, &state.config.jwt_secret) {
            request.extensions_mut().insert(claims);
            return next.run(request).await;
        }
    }
    (StatusCode::UNAUTHORIZED, Json(ApiResponse::<()>::unauthorized("未登录或会话已过期"))).into_response()
}

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<ApiResponse<()>>);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Claims>().cloned().ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(ApiResponse::<()>::unauthorized("未登录")))
        })
    }
}