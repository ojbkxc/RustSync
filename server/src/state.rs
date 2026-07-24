use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use crate::config::Config;

/// 应用全局共享状态
pub struct AppState {
    pub config: Config,
    /// 数据库连接（用 RwLock 保护，因为 rusqlite 的 Connection 不是 Sync）
    pub db: RwLock<rusqlite::Connection>,
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
            db: RwLock::new(db),
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