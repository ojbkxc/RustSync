pub mod models;
pub mod response;

/// 兼容读取 JSON bool 值：支持 true/false 布尔值，也支持 1/0 数字
pub fn json_bool(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(b) => Some(*b),
        serde_json::Value::Number(n) => n.as_i64().map(|i| i != 0),
        _ => None,
    }
}