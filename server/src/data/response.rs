use serde::Serialize;

/// API 响应格式 - 与 Python commonService.result_map 兼容
/// Python: {"code": 200, "data": ..., "msg": "..."}
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
        Self {
            code: 200,
            data: Some(data),
            msg: Some("success".to_string()),
        }
    }

    pub fn ok_msg(data: T, msg: &str) -> Self {
        Self {
            code: 200,
            data: Some(data),
            msg: Some(msg.to_string()),
        }
    }

    pub fn err(msg: &str) -> Self {
        Self {
            code: 500,
            data: None,
            msg: Some(msg.to_string()),
        }
    }

    pub fn err_code(code: i32, msg: &str) -> Self {
        Self {
            code,
            data: None,
            msg: Some(msg.to_string()),
        }
    }
}