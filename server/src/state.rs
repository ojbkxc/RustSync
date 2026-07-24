use std::sync::{Arc, Mutex, OnceLock};
use crate::config::Config;

/// 应用全局共享状态
/// 使用 std::sync::Mutex 而非 tokio::sync::RwLock
/// 因为 rusqlite::Connection 包含 RefCell 不实现 Sync，
/// 但 Mutex<T: Send> 是 Sync 的，符合 axum Router 要求。
pub struct AppState {
    pub config: Config,
    pub db: Mutex<rusqlite::Connection>,
}

pub type SharedState = Arc<AppState>;

/// 全局状态持有者
static GLOBAL_STATE: OnceLock<SharedState> = OnceLock::new();

impl AppState {
    pub fn new(config: Config) -> anyhow::Result<SharedState> {
        let db = rusqlite::Connection::open(&config.db_path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Arc::new(Self {
            config,
            db: Mutex::new(db),
        }))
    }

    pub fn new_shared(config: Config) -> anyhow::Result<SharedState> {
        let state = Self::new(config)?;
        let _ = GLOBAL_STATE.set(state.clone());
        Ok(state)
    }
}

/// 获取全局状态（供 sync_engine 等模块使用）
pub fn get_global_state() -> SharedState {
    GLOBAL_STATE.get().expect("全局状态未初始化").clone()
}