use serde::{Deserialize, Serialize};

// ==================== 引擎 / 存储挂载 ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    pub id: i64,
    pub remark: Option<String>,
    pub url: String,
    pub user_name: Option<String>,
    pub token: Option<String>,
    #[serde(default = "default_engine_type")]
    pub engine_type: String,
    pub system_key: Option<String>,
    #[serde(default)]
    pub protected: bool,
    pub create_time: i64,
}

fn default_engine_type() -> String { "alist".to_string() }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRequest {
    pub remark: Option<String>,
    pub url: String,
    pub user_name: Option<String>,
    pub token: Option<String>,
    #[serde(default = "default_engine_type")]
    pub engine_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StorageMount {
    pub id: i64,
    pub engine_id: i64,
    pub name: String,
    pub driver_type: String,
    pub config: String,
    pub enabled: bool,
    pub config_version: i32,
    pub auth_version: i32,
    pub create_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMountRequest {
    pub engine_id: i64,
    pub name: String,
    pub driver_type: String,
    pub config: serde_json::Value,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }

// ==================== 作业 ====================

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: i64,
    pub enable: bool,
    pub remark: Option<String>,
    pub src_path: String,
    pub dst_path: String,
    pub alist_id: Option<i64>,
    pub use_cache_t: bool,
    pub scan_interval_t: i32,
    pub use_cache_s: bool,
    pub scan_interval_s: i32,
    pub method: i32,
    pub source_mode: bool,
    pub interval: Option<i32>,
    pub is_cron: i32,
    pub year: Option<String>,
    pub month: Option<String>,
    pub day: Option<String>,
    pub week: Option<String>,
    pub day_of_week: Option<String>,
    pub hour: Option<String>,
    pub minute: Option<String>,
    pub second: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub exclude: Option<String>,
    pub min_file_size: Option<i64>,
    pub max_file_size: Option<i64>,
    pub create_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub enable: Option<bool>,
    pub remark: Option<String>,
    pub src_path: String,
    pub dst_path: String,
    pub alist_id: Option<i64>,
    #[serde(default)] pub use_cache_t: bool,
    #[serde(default)] pub scan_interval_t: i32,
    #[serde(default)] pub use_cache_s: bool,
    #[serde(default)] pub scan_interval_s: i32,
    pub method: i32,
    #[serde(default)] pub source_mode: bool,
    #[serde(default)] pub interval: Option<i32>,
    #[serde(default)] pub is_cron: i32,
    pub year: Option<String>,
    pub month: Option<String>,
    pub day: Option<String>,
    pub week: Option<String>,
    pub day_of_week: Option<String>,
    pub hour: Option<String>,
    pub minute: Option<String>,
    pub second: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub exclude: Option<String>,
    pub min_file_size: Option<i64>,
    pub max_file_size: Option<i64>,
}

// ==================== 任务 ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobTask {
    pub id: i64,
    pub job_id: i64,
    pub status: i32,
    pub err_msg: Option<String>,
    pub run_time: Option<i64>,
    pub task_num: Option<String>,
    pub create_time: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobTaskItem {
    pub id: i64,
    pub task_id: i64,
    pub src_path: Option<String>,
    pub dst_path: Option<String>,
    pub is_path: bool,
    pub file_name: Option<String>,
    pub file_size: Option<i64>,
    #[serde(rename = "type")]
    pub item_type: i32,
    pub alist_task_id: Option<String>,
    pub status: i32,
    pub progress: Option<f64>,
    pub err_msg: Option<String>,
    pub create_time: i64,
}

// ==================== 通知 ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Notify {
    pub id: i64,
    pub enable: bool,
    pub method: i32,
    pub params: String,
    pub create_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyRequest {
    #[serde(default = "default_enabled")]
    pub enable: bool,
    pub method: i32,
    pub params: serde_json::Value,
}

// ==================== 用户 ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub user_name: String,
    pub passwd: String,
    pub sql_version: i32,
    pub create_time: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: i64,
    pub user_name: String,
    pub create_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub user_name: String,
    pub passwd: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub old_passwd: String,
    pub passwd: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordRequest {
    pub user_name: String,
    pub key: String,
    #[serde(default)]
    pub passwd: Option<String>,
}

// ==================== 分页 / 语言 ====================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    #[serde(default = "default_page_num")]
    pub page_num: i32,
    #[serde(default = "default_page_size")]
    pub page_size: i32,
}

fn default_page_num() -> i32 { 1 }
fn default_page_size() -> i32 { 10 }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult<T: Serialize> {
    pub list: Vec<T>,
    pub total: i64,
    pub page_num: i32,
    pub page_size: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageRequest {
    pub language: String,
}

// ==================== JWT Claims ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i64,
    pub user_name: String,
    pub exp: usize,
    pub iat: usize,
}
