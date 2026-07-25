use serde::Serialize;

/// API 错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// 成功
    Ok = 200,
    /// 参数校验失败
    BadRequest = 400,
    /// 未认证
    Unauthorized = 401,
    /// 权限不足
    Forbidden = 403,
    /// 资源不存在
    NotFound = 404,
    /// 资源冲突（如作业正在运行）
    Conflict = 409,
    /// 服务器内部错误
    Internal = 500,
}

/// API 统一响应格式
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self { code: 200, data: Some(data), msg: None }
    }

    pub fn ok_msg(data: T, msg: &str) -> Self {
        Self { code: 200, data: Some(data), msg: Some(msg.to_string()) }
    }

    pub fn error(code: ErrorCode, msg: &str) -> Self {
        Self { code: code as i32, data: None, msg: Some(msg.to_string()) }
    }

    pub fn err(msg: &str) -> Self {
        Self::error(ErrorCode::Internal, msg)
    }

    pub fn bad_request(msg: &str) -> Self {
        Self::error(ErrorCode::BadRequest, msg)
    }

    pub fn unauthorized(msg: &str) -> Self {
        Self::error(ErrorCode::Unauthorized, msg)
    }

    pub fn forbidden(msg: &str) -> Self {
        Self::error(ErrorCode::Forbidden, msg)
    }

    pub fn not_found(msg: &str) -> Self {
        Self::error(ErrorCode::NotFound, msg)
    }

    pub fn conflict(msg: &str) -> Self {
        Self::error(ErrorCode::Conflict, msg)
    }
}